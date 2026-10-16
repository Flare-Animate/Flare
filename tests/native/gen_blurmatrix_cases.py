"""Regenerate flare/sources/flarelib/sandor_fxs/BlurMatrix_cases.txt.

That file is the golden output of the *original* C++ ``CBlurMatrix`` (``createEqual``,
``addPath`` and ``createRandom``), one block per case with the sample distances as raw
IEEE-754 bit patterns. The Rust port (``flarelib/sandor_fxs/BlurMatrix.rs``) is checked
against it byte for byte, so a transcription slip -- a swapped rounding mode, a missed
mirror quadrant -- fails the build instead of shipping silently.

Because the original ``BlurMatrix.cpp`` is deleted once the port lands, the reference is
compiled from git history: ``git log -1 -- <cpp>`` finds the last commit that still had
it. That is also why this script is *not* part of the test run: it needs MSVC and a git
checkout, neither of which the Rust test suite depends on. The generated golden is
committed.

The harness is ``tests/native/blurmatrix_ref.cpp``; the case table it iterates and the
Rust test must agree. Change one, change the other.

Usage:
    python tests/native/gen_blurmatrix_cases.py [--check]

``--check`` does not write; it reports whether the committed golden still matches the
reference (exit 1 if it drifted).
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
SRC = "flare/sources/flarelib/sandor_fxs/BlurMatrix.cpp"
HDR = "flare/sources/flarelib/sandor_fxs/BlurMatrix.h"
SUPPORT = [
    "flare/sources/flarelib/sandor_fxs/SDef.h",
    "flare/sources/flarelib/sandor_fxs/SError.h",
    "flare/sources/flarelib/sandor_fxs/SError.cpp",
]
GOLDEN = os.path.join(ROOT, "flare", "sources", "flarelib", "sandor_fxs", "BlurMatrix_cases.txt")
HARNESS = os.path.join(HERE, "blurmatrix_ref.cpp")


def git(*args: str) -> str:
    return subprocess.check_output(["git", *args], cwd=ROOT, text=True).strip()


def pre_deletion_revision() -> str:
    """The last commit whose tree still contained the C++ implementation."""
    rev = git("log", "-1", "--format=%H", "--", SRC)
    if not rev:
        sys.exit(f"no history for {SRC}; cannot locate the original")
    return rev


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
    for rel in [SRC, HDR, *SUPPORT]:
        data = subprocess.check_output(["git", "show", f"{rev}:{rel}"], cwd=ROOT)
        out = os.path.join(workdir, os.path.basename(rel))
        with open(out, "wb") as fh:
            fh.write(data)
    shutil.copyfile(HARNESS, os.path.join(workdir, "ref.cpp"))

    bat = os.path.join(workdir, "build.bat")
    with open(bat, "w") as fh:
        fh.write(
            "@echo off\n"
            f'call "{vcvars()}" >nul || exit /b 1\n'
            # NOMINMAX: BlurMatrix.cpp includes <windows.h> on _WIN32, whose min/max
            # macros break its std::max/std::abs call.
            "cl /nologo /DNOMINMAX /EHsc /std:c++17 /O2 ref.cpp BlurMatrix.cpp SError.cpp "
            "/Fe:ref.exe || exit /b 1\n"
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
    with tempfile.TemporaryDirectory(prefix="blurmatrix-") as workdir:
        fresh = build_reference(rev, workdir)

    old = ""
    if os.path.exists(GOLDEN):
        with open(GOLDEN, "r", newline="") as fh:
            old = fh.read()

    blocks = len([b for b in fresh.split("\n\n") if b.strip()])
    if fresh == old:
        print(f"unchanged: {GOLDEN} ({blocks} cases)")
        return 0
    if args.check:
        print(f"DRIFT: {GOLDEN} does not match the reference ({blocks} cases)", file=sys.stderr)
        return 1
    with open(GOLDEN, "w", newline="") as fh:
        fh.write(fresh)
    print(f"wrote {GOLDEN} ({blocks} cases) from {rev[:12]}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())