"""Regenerate the golden for the `igs_motion_blur.cpp` port.

Records what the *original* `igs_motion_blur.cpp` (recovered from git history, since the
port deletes it) produced for 16 kernel configurations: the sparse kernel itself
(weights as raw IEEE-754 hex, pixel offsets) and 12 blurred images each (8/16-bit x
4/3/1 channels x alpha-blur on/off), hashed with FNV-1a 64.

The kernel goes through the host libm (`atan`/`sin`/`cos`/`pow`), so the Rust test
compares weights within a few ULP and only requires the image hashes to match
exactly when fed the recorded weights (see `igs_motion_blur::tests`). The reference is
built with g++/clang++ `-ffp-contract=off`.

Usage:
    python tests/native/gen_igs_motion_blur_cases.py [--check]
"""

from __future__ import annotations

import argparse
import os
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import msvc_reference as ms  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
SRCS = [
    "flare/sources/stdfx/igs_motion_blur.cpp",
    "flare/sources/stdfx/igs_motion_blur.h",
]
GOLDEN = os.path.join(ms.ROOT, "flare", "sources", "stdfx", "igs_motion_blur_cases.txt")
HARNESS = os.path.join(HERE, "igs_motion_blur_ref.cpp")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true", help="do not write; exit 1 on drift")
    ap.add_argument(
        "--verify-chain",
        action="store_true",
        help="also build the harness against the LIVE inline header + libflare_formats.a "
        "and require the golden's output (needs cargo, works offline)",
    )
    args = ap.parse_args()

    rev = ms.pre_deletion_tree(SRCS[0])
    stdfx = os.path.join(ms.ROOT, "flare", "sources", "stdfx")
    with tempfile.TemporaryDirectory(prefix="igsmotblur-") as workdir:
        ms.extract(rev, SRCS, workdir)
        # igs_motion_blur.cpp is #included by the harness; nothing else to compile.
        fresh = ms.build_and_run_host(workdir, [], HARNESS, include_dirs=[workdir, stdfx])
    rc = ms.check_or_write(GOLDEN, fresh, rev, args.check, count="lines")
    if rc == 0 and args.verify_chain:
        # only the image lines: the kernel arrays live in the original's anonymous namespace
        rc = ms.verify_chain(HARNESS, GOLDEN, [stdfx], keep=lambda l: l.startswith("img "))
    return rc


if __name__ == "__main__":
    raise SystemExit(main())
