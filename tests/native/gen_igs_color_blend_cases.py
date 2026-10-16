"""Regenerate flare/sources/stdfx/igs_color_blend_cases.txt.

That file is the golden output of the *original* C++ blend kernels, captured one
`name r g b a` line per (function, case) with the channels as raw IEEE-754 bit
patterns.  The Rust port (`stdfx/igs_color_blend.rs`) is checked against it
bit-for-bit, so a transcription slip -- a swapped branch, a dropped clamp --
fails the build rather than shipping silently.

Because the original `igs_color_blend.cpp` is deleted once the port lands, the
reference is compiled from git history: `git log -1 -- <cpp>` finds the last
commit that still had it.  That is also why this script is *not* part of the
test run: it needs MSVC and a git checkout, neither of which the Rust test suite
depends on.  The generated golden is committed.

The harness is `tests/native/igs_color_blend_ref.cpp`; its case table is mirrored
in the Rust test.  Change one, change the other.

Usage:
    python tests/native/gen_igs_color_blend_cases.py [--check]

`--check` does not write; it reports whether the committed golden still matches
the reference (exit 1 if it drifted).
"""
from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
SRC = "flare/sources/stdfx/igs_color_blend.cpp"
HDR = "flare/sources/stdfx/igs_color_blend.h"
GOLDEN = os.path.join(ROOT, "flare", "sources", "stdfx", "igs_color_blend_cases.txt")
HARNESS = os.path.join(HERE, "igs_color_blend_ref.cpp")


def git(*args: str) -> str:
    return subprocess.check_output(["git", *args], cwd=ROOT, text=True).strip()


def pre_deletion_revision() -> str:
    """The newest commit whose tree still contained the C++ implementation.

    `git log -1 -- <path>` alone is wrong once the deletion is committed: it returns the
    commit that *removed* the file, and `git show` on it fails. So walk the history and
    take the first revision where the path actually exists.
    """
    for rev in git("log", "--format=%H", "--", SRC).splitlines():
        if not rev:
            continue
        if subprocess.run(
            ["git", "cat-file", "-e", f"{rev}:{SRC}"], cwd=ROOT
        ).returncode == 0:
            return rev
    sys.exit(f"no revision of {SRC} to build the reference from")


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


def build_reference(rev: str, workdir: str) -> str:
    """Extract the original sources at `rev`, build the harness, return its output."""
    cpp = subprocess.check_output(["git", "show", f"{rev}:{SRC}"], cwd=ROOT)
    hdr = subprocess.check_output(["git", "show", f"{rev}:{HDR}"], cwd=ROOT)
    with open(os.path.join(workdir, "igs_color_blend.cpp"), "wb") as fh:
        fh.write(cpp)
    with open(os.path.join(workdir, "igs_color_blend.h"), "wb") as fh:
        fh.write(hdr)
    shutil.copyfile(HARNESS, os.path.join(workdir, "ref.cpp"))

    bat = os.path.join(workdir, "build.bat")
    with open(bat, "w") as fh:
        fh.write(
            "@echo off\n"
            f'call "{vcvars()}" >nul || exit /b 1\n'
            "cl /nologo /EHsc /std:c++17 /O2 ref.cpp igs_color_blend.cpp /Fe:ref.exe || exit /b 1\n"
            "ref.exe > ref.txt || exit /b 1\n"
        )
    subprocess.run(["cmd", "/c", bat], cwd=workdir, check=True)
    with open(os.path.join(workdir, "ref.txt"), "r", newline="") as fh:
        return fh.read()


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true", help="do not write; exit 1 on drift")
    args = ap.parse_args()

    rev = pre_deletion_revision()
    with tempfile.TemporaryDirectory(prefix="igsblend-") as workdir:
        fresh = build_reference(rev, workdir)

    old = ""
    if os.path.exists(GOLDEN):
        with open(GOLDEN, "r", newline="") as fh:
            old = fh.read()

    lines = len(fresh.splitlines())
    if fresh == old:
        print(f"unchanged: {GOLDEN} ({lines} lines)")
        return 0
    if args.check:
        print(f"DRIFT: {GOLDEN} does not match the reference ({lines} lines)", file=sys.stderr)
        return 1
    with open(GOLDEN, "w", newline="") as fh:
        fh.write(fresh)
    print(f"wrote {GOLDEN} ({lines} lines) from {rev[:12]}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
