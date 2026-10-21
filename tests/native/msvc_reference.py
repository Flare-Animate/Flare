"""Shared plumbing for the Rust-port differential harnesses in ``tests/native``.

Each port keeps a committed golden of what the *original* C++ produced, captured by
compiling that C++ with MSVC and printing its output. Regenerating a golden means the same
four steps every time:

1. find the newest commit whose tree still had the (now deleted) ``.cpp``;
2. extract that revision's sources;
3. compile them together with the harness under MSVC;
4. run it and capture stdout.

That was duplicated verbatim in ``gen_igs_color_blend_cases.py`` and
``gen_blurmatrix_cases.py``; it lives here now so the next generator is a dozen lines. The
subtleties it encodes, each of which cost a debugging cycle at least once:

- ``git log -1 -- <path>`` returns the commit that **deleted** the file once that commit
  lands, and ``git show`` on it fails. Walk the history and take the first revision where
  the path still exists.
- ``git show`` output is decoded as **bytes**: these sources carry non-ASCII comments that
  are not valid UTF-8, and a text-mode read dies on them.
- The compiler is ``cl`` from ``vcvars64.bat``, located through ``vswhere`` unless
  ``VCVARS64`` is set.
- ``/DNOMINMAX`` is required by any source that includes ``<windows.h>``: the ``min`` and
  ``max`` macros break ``std::max`` / ``std::abs``.
- stdout goes through the CRT in text mode, so a captured file is **CRLF** on Windows.
  ``normalize()`` undoes that, and callers should compare normalized text.
"""
from __future__ import annotations

import os
import shutil
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))


def git(*args: str) -> str:
    return subprocess.check_output(["git", *args], cwd=ROOT, text=True).strip()


def pre_deletion_revision(src: str) -> str:
    """The newest commit whose tree still contained `src` (repo-relative)."""
    for rev in git("log", "--format=%H", "--", src).splitlines():
        if not rev:
            continue
        if subprocess.run(
            ["git", "cat-file", "-e", f"{rev}:{src}"],
            cwd=ROOT,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        ).returncode == 0:
            return rev
    sys.exit(f"no revision of {src} exists to build a reference from")


def pre_deletion_tree(src: str) -> str:
    """The tree immediately before `src` was deleted (or HEAD while it still exists).

    Unlike `pre_deletion_revision` (the newest commit that *touched* `src`), this also
    reflects later edits to the headers that sat next to it, which is what a reference
    build of the original needs.
    """
    deleted = git("log", "--format=%H", "--diff-filter=D", "-1", "--", src)
    if deleted:
        return git("rev-parse", f"{deleted}^")
    return git("rev-parse", "HEAD")


def vcvars() -> str:
    for var in ("VCVARS64", "VCVARSALL"):
        if os.environ.get(var):
            return os.environ[var]
    program_files = os.environ.get("ProgramFiles(x86)", r"C:\Program Files (x86)")
    vswhere = os.path.join(program_files, "Microsoft Visual Studio", "Installer", "vswhere.exe")
    if os.path.exists(vswhere):
        out = subprocess.check_output(
            [vswhere, "-latest", "-property", "installationPath"], text=True
        ).strip()
        if out:
            cand = os.path.join(out, "VC", "Auxiliary", "Build", "vcvars64.bat")
            if os.path.exists(cand):
                return cand
    sys.exit("could not find vcvars64.bat; set VCVARS64")


def extract(rev: str, paths: list[str], workdir: str) -> None:
    """Write each repo-relative `paths` entry into `workdir`, flattened by basename."""
    for rel in paths:
        data = subprocess.check_output(["git", "show", f"{rev}:{rel}"], cwd=ROOT)
        with open(os.path.join(workdir, os.path.basename(rel)), "wb") as fh:
            fh.write(data)


def build_and_run(workdir: str, sources: list[str], harness: str, exe: str = "ref") -> str:
    """Compile `sources` + `harness` with MSVC in `workdir`, run it, return its output."""
    shutil.copyfile(harness, os.path.join(workdir, "ref.cpp"))
    joined = " ".join(sources)
    bat = os.path.join(workdir, "build.bat")
    with open(bat, "w") as fh:
        fh.write(
            "@echo off\n"
            f'call "{vcvars()}" >nul || exit /b 1\n'
            f"cl /nologo /DNOMINMAX /EHsc /std:c++17 /O2 ref.cpp {joined} /Fe:{exe}.exe || exit /b 1\n"
            f"{exe}.exe > {exe}.txt || exit /b 1\n"
        )
    subprocess.run(["cmd", "/c", bat], cwd=workdir, check=True)
    with open(os.path.join(workdir, f"{exe}.txt"), "r", newline="") as fh:
        return fh.read()


def build_and_run_host(
    workdir: str,
    sources: list[str],
    harness: str,
    include_dirs: list[str] | None = None,
    link_archives: list[str] | None = None,
    exe: str = "ref",
    extra_args: list[str] | None = None,
) -> str:
    """Same as `build_and_run` but with the host C++ compiler (clang++/g++), for POSIX.

    Goldens captured this way depend on the host libm for `sqrt`/`pow`/`cos`/`sin`;
    callers comparing transcendental output must allow a few ULP rather than raw bits
    (see the generator docstrings).
    """
    shutil.copyfile(harness, os.path.join(workdir, "ref.cpp"))
    cxx = os.environ.get("CXX") or shutil.which("g++") or shutil.which("clang++")
    if not cxx:
        sys.exit("no host C++ compiler found (set CXX)")
    argv = [
        cxx,
        "-O2",
        "-std=c++17",
        # No FMA contraction: the C++ source expresses these as separate mul+add, and
        # Rust does not fuse them either. Without this the reference binary records a
        # 1-ULP artifact of the host compiler, not a property of the original logic.
        "-ffp-contract=off",
        "-o",
        exe,
        "ref.cpp",
        *sources,
    ]
    # clang++ on some Linux distros cannot find the libstdc++ headers by default;
    # tell it to use the GCC standard library, which build-essential always provides.
    if "clang" in os.path.basename(cxx).lower():
        argv += ["-stdlib=libstdc++"]
    for d in include_dirs or []:
        argv += ["-I", d]
    argv += [*(extra_args or [])]
    argv += [*(link_archives or [])]
    subprocess.run(argv, cwd=workdir, check=True)
    out = subprocess.run(
        [os.path.join(workdir, exe)], cwd=workdir, check=True, capture_output=True, text=True
    ).stdout
    return out


def normalize(text: str) -> str:
    """CRLF -> LF, so golden comparison and blank-line splitting behave the same."""
    return text.replace("\r\n", "\n")


def blocks(text: str) -> list[str]:
    """Non-empty blocks of a harness dump, which separates cases with a blank line."""
    return [b for b in normalize(text).split("\n\n") if b.strip()]


def check_or_write(golden: str, fresh: str, rev: str, check: bool, count: str = "cases") -> int:
    """Compare against the committed golden; write it unless `--check`. Returns an exit code.

    `count` is the noun in the progress message: "cases" counts blank-line-separated
    blocks (the block-dump harnesses), "lines" counts lines (the line-dump harness).
    """
    fresh = normalize(fresh)
    n = len(blocks(fresh)) if count == "cases" else len(fresh.splitlines())
    old = ""
    if os.path.exists(golden):
        with open(golden, "r", newline="") as fh:
            old = normalize(fh.read())
    if fresh == old:
        print(f"unchanged: {golden} ({n} {count})")
        return 0
    if check:
        print(f"DRIFT: {golden} does not match the reference ({n} {count})", file=sys.stderr)
        return 1
    with open(golden, "w", newline="") as fh:
        fh.write(fresh)
    print(f"wrote {golden} ({n} {count}) from {rev[:12]}")
    return 0


def rust_archive() -> str:
    """Build `libflare_formats.a` (offline, debug) and return its path."""
    src = os.path.join(ROOT, "flare", "sources")
    subprocess.run(["cargo", "build", "-p", "flare_formats", "--offline", "-q"], cwd=src, check=True)
    return os.path.join(src, "target", "debug", "libflare_formats.a")


def verify_chain(
    harness: str,
    golden: str,
    include_dirs: list[str],
    keep=None,
    extra_args: list[str] | None = None,
) -> int:
    """Build `harness -DCHAIN` against the LIVE inline headers + the Rust static library
    and require the (optionally `keep`-filtered) output to equal the committed golden.

    This proves the C++ forwarders marshal exactly like the original functions did. The
    harnesses guard the parts that need the original sources (`#ifndef CHAIN`).
    """
    archive = rust_archive()
    with open(golden, "r", newline="") as fh:
        want = normalize(fh.read()).splitlines()
    if keep:
        want = [l for l in want if keep(l)]
    with tempfile.TemporaryDirectory(prefix="chain-") as workdir:
        got = normalize(
            build_and_run_host(
                workdir,
                [],
                harness,
                include_dirs=include_dirs,
                link_archives=[archive, "-lpthread", "-ldl", "-lm"],
                extra_args=["-DCHAIN", *(extra_args or [])],
            )
        ).splitlines()
    if keep:
        got = [l for l in got if keep(l)]
    if got != want:
        n = next((i for i, (a, b) in enumerate(zip(got, want)) if a != b), min(len(got), len(want)))
        print(f"CHAIN MISMATCH at line {n}: got {got[n:n+1]} want {want[n:n+1]}", file=sys.stderr)
        return 1
    print(f"chain ok: new headers + Rust reproduce {len(want)} golden lines")
    return 0
