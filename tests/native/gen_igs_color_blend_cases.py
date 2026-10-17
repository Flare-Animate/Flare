"""Regenerate tests/native/../stdfx/igs_color_blend_cases.txt: the port's golden output.

That file is the golden output of the *original* C++ blend kernels, captured one
`name r g b a` line per (function, case) with the channels as raw IEEE-754 bit
patterns.  The Rust port (`stdfx/igs_color_blend.rs`) is checked against it
bit-for-bit, so a transcription slip -- a swapped branch, a dropped clamp --
fails the build rather than shipping silently.

Because the original `igs_color_blend.cpp` is deleted once the port lands, the
reference is compiled from git history.  That is also why this script is *not* part of
the test run: it needs MSVC and a git checkout, neither of which the Rust test suite
depends on.  The generated golden is committed.  See ``msvc_reference`` for the shared
plumbing.

The harness is `tests/native/igs_color_blend_ref.cpp`; its case table is mirrored in that
Rust test.  Change one, change the other.

Usage:
    python tests/native/gen_igs_color_blend_cases.py [--check]

`--check` does not write; it reports whether the committed golden still matches
the reference (exit 1 if it drifted).
"""
from __future__ import annotations

import argparse
import os
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import msvc_reference as ms  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
SRC = "flare/sources/stdfx/igs_color_blend.cpp"
HDR = "flare/sources/stdfx/igs_color_blend.h"
GOLDEN = os.path.join(ms.ROOT, "flare", "sources", "stdfx", "igs_color_blend_cases.txt")
HARNESS = os.path.join(HERE, "igs_color_blend_ref.cpp")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true", help="do not write; exit 1 on drift")
    args = ap.parse_args()

    rev = ms.pre_deletion_revision(SRC)
    with tempfile.TemporaryDirectory(prefix="igsblend-") as workdir:
        ms.extract(rev, [SRC, HDR], workdir)
        fresh = ms.build_and_run(workdir, ["igs_color_blend.cpp"], HARNESS)

    return ms.check_or_write(GOLDEN, fresh, rev, args.check, count="lines")


if __name__ == "__main__":
    raise SystemExit(main())