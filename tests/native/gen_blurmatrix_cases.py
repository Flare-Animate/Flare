"""Regenerate flare/sources/flarelib/sandor_fxs/BlurMatrix_cases.txt.

That file is the golden output of the *original* C++ ``CBlurMatrix`` (``createEqual``,
``addPath`` and ``createRandom``), one block per case with the sample distances as raw
IEEE-754 bit patterns. The Rust port (``flarelib/sandor_fxs/BlurMatrix.rs``) is checked
against it byte for byte, so a transcription slip -- a swapped rounding mode, a missed
mirror quadrant -- fails the build instead of shipping silently.

The ``.cpp`` is deleted, so the reference is rebuilt from git history. That needs MSVC and
a checkout, so this is *not* part of the test run; the generated golden is committed. See
``msvc_reference`` for the shared plumbing.

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
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import msvc_reference as ms  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
SRC = "flare/sources/flarelib/sandor_fxs/BlurMatrix.cpp"
GOLDEN = os.path.join(
    ms.ROOT, "flare", "sources", "flarelib", "sandor_fxs", "BlurMatrix_cases.txt"
)
HARNESS = os.path.join(HERE, "blurmatrix_ref.cpp")
# BlurMatrix.cpp throws SBlurMatrixError, so SError.cpp has to link in; SDef.h supplies
# SXYD and I_ROUND.
SUPPORT = [
    "flare/sources/flarelib/sandor_fxs/BlurMatrix.h",
    "flare/sources/flarelib/sandor_fxs/SDef.h",
    "flare/sources/flarelib/sandor_fxs/SError.h",
    "flare/sources/flarelib/sandor_fxs/SError.cpp",
]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true", help="do not write; exit 1 on drift")
    args = ap.parse_args()

    rev = ms.pre_deletion_revision(SRC)
    with tempfile.TemporaryDirectory(prefix="blurmatrix-") as workdir:
        ms.extract(rev, [SRC, *SUPPORT], workdir)
        fresh = ms.build_and_run(workdir, ["BlurMatrix.cpp", "SError.cpp"], HARNESS)

    return ms.check_or_write(GOLDEN, fresh, rev, args.check)


if __name__ == "__main__":
    raise SystemExit(main())