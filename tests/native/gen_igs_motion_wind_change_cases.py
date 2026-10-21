"""Regenerate the golden for the `igs_motion_wind.cpp` port (the scanline walk).

Records what the *original* `igs_motion_wind.cpp` (+ `igs_motion_wind_pixel.cpp` and
`igs_motion_wind_table.cpp`, recovered from git history because the ports delete them)
produced over 576 cases: 2 configs x 8/16-bit x 4/3/1 channels x 4 directions x
blow-dark x blow-alpha x {no ref, 8-bit ref, 16-bit ref}, hashed with FNV-1a 64.

Every config is arithmetic only (bias 1, force 1, so `pow(x, 1.0)`), hence the hashes
are exact on any libm. As in `gen_igs_motion_wind_cases.py`, two goldens exist because
the original LCG is as wide as `unsigned long` (64-bit on LP64, 32-bit on LLP64):

    igs_motion_wind_change_cases.txt       --rng-bits 64
    igs_motion_wind_change_cases_32.txt    --rng-bits 32

Three revisions are involved: `igs_motion_wind.cpp` and its header come from the newest
commit that still had the cpp; the pixel/table cpp + headers from the (older) commit
before *they* were retired; `igs_ifx_common.h` is the live one.

Usage:
    python tests/native/gen_igs_motion_wind_change_cases.py [--rng-bits {32,64}] [--check]
"""

from __future__ import annotations

import argparse
import os
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import msvc_reference as ms  # noqa: E402
import gen_igs_motion_wind_cases as pix  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
WALK_SRCS = [
    "flare/sources/stdfx/igs_motion_wind.cpp",
    "flare/sources/stdfx/igs_motion_wind.h",
]
GOLDENS = {
    64: os.path.join(ms.ROOT, "flare", "sources", "stdfx", "igs_motion_wind_change_cases.txt"),
    32: os.path.join(ms.ROOT, "flare", "sources", "stdfx", "igs_motion_wind_change_cases_32.txt"),
}
HARNESS = os.path.join(HERE, "igs_motion_wind_change_ref.cpp")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--rng-bits", type=int, choices=(32, 64), default=64)
    ap.add_argument("--check", action="store_true", help="do not write; exit 1 on drift")
    args = ap.parse_args()

    walk_rev = ms.pre_deletion_revision(WALK_SRCS[0])
    pix_rev = ms.pre_deletion_revision(pix.SRCS[0])
    stdfx = os.path.join(ms.ROOT, "flare", "sources", "stdfx")
    with tempfile.TemporaryDirectory(prefix="igswindchg-") as workdir:
        ms.extract(pix_rev, pix.SRCS + pix.HDRS, workdir)
        ms.extract(walk_rev, WALK_SRCS, workdir)
        with open(os.path.join(workdir, "igs_math_random.h"), "w", encoding="utf-8") as fh:
            fh.write(pix.rng_stub(args.rng_bits))
        fresh = ms.build_and_run_host(
            workdir,
            ["igs_motion_wind.cpp"] + [os.path.basename(s) for s in pix.SRCS],
            HARNESS,
            include_dirs=[stdfx, workdir],
        )
    return ms.check_or_write(
        GOLDENS[args.rng_bits], fresh, f"{walk_rev}+{pix_rev}", args.check, count="lines"
    )


if __name__ == "__main__":
    raise SystemExit(main())
