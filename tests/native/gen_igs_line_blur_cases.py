"""Regenerate the golden for the `igs_line_blur.cpp` port.

Records what the *original* `igs_line_blur.cpp` (recovered from git history, since the
port deletes it) produced for:

* 45 end-to-end `igs::line_blur::convert` runs (8/16-bit, 1-4 channels, every reference
  channel, padded strides, curve blur and smudge brush, blur lengths / powers / sub-pixel
  counts / near-line counts and lengths, smoothing retries, a no-op brush action, tiny
  images and the "thinning loop counter over 100" exception), each as the FNV-1a 64 of the
  whole output buffer (padding included) plus a structure summary of the vector stage
  (line count, point count, bounding box, hash of every point);
* the thinning class on its own, including the 2x/3x scale-up convert never uses;
* the libm-dependent helpers (`get_d_radian`, `get_dd_rotate`, the `pow` ratio table, the
  `sin` line weight) as raw IEEE hex;
* the verbose output (`mv_sw/pv_sw/cv_sw`, timestamps and addresses normalised) and the ten
  `debug_save_sw` files of both brushes.

The vector stage goes through the host libm (`atan`, `sin`, `cos`, `pow`), so the Rust test
compares those values within 4 ULP and requires the image hashes and the structure summary
exactly only where its own libm values are bit-identical (always on linux-gnu). Built with
`-ffp-contract=off -DNDEBUG` (the original's `assert`s are compiled out as in a Release build).

Usage:
    python tests/native/gen_igs_line_blur_cases.py [--check] [--verify-chain]
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
    "flare/sources/stdfx/igs_line_blur.cpp",
    "flare/sources/stdfx/igs_line_blur.h",
]
GOLDEN = os.path.join(ms.ROOT, "flare", "sources", "stdfx", "igs_line_blur_cases.txt")
HARNESS = os.path.join(HERE, "igs_line_blur_ref.cpp")


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
    with tempfile.TemporaryDirectory(prefix="igslineblur-") as workdir:
        ms.extract(rev, SRCS, workdir)
        # igs_line_blur.cpp is #included by the harness; nothing else to compile.
        fresh = ms.build_and_run_host(
            workdir, [], HARNESS, include_dirs=[workdir, stdfx], extra_args=["-DNDEBUG"]
        )
    rc = ms.check_or_write(GOLDEN, fresh, rev, args.check, count="lines")
    if rc == 0 and args.verify_chain:
        # the class-level records only exist in the original
        rc = ms.verify_chain(
            HARNESS,
            GOLDEN,
            [stdfx],
            keep=lambda l: l.split(" ", 1)[0]
            in ("case", "img", "throw", "vcase", "log", "err", "vimg", "file"),
            extra_args=["-DNDEBUG"],
        )
    return rc


if __name__ == "__main__":
    raise SystemExit(main())
