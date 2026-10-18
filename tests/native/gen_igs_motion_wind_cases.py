"""Regenerate flare/sources/stdfx/igs_motion_wind_cases.txt: the port's golden output.

Records what the *original* `igs_motion_wind_pixel.cpp` + `igs_motion_wind_table.cpp`
(recovered from git history, since the port deletes both) produced for three fixed
configurations driven over a deterministic 400-pixel sequence.

The original RNG already routes through `flare_formats` (`igs_math_random.h`), so the
LCG stream is shared by construction and only the table/pixel arithmetic is under test.

The Rust test `igs_motion_wind_pixel::tests::matches_the_cpp_reference` replays it:
`inside` must match exactly; channel values within 1e-12 for the gamma (`pow`) config
and exactly for the arithmetic-only ones.

Two things to know about how the reference is built:
- Quoted includes resolve relative to the *including* file first, so the archived
  `igs_motion_wind_pixel.cpp` picks up the archived `igs_motion_wind_pixel.h` from the
  temp workdir even though `-I <tree>/stdfx` comes first. If `HDRS` ever loses an entry,
  the live FFI-forwarder header wins instead and the class is defined twice (a compile
  error, not a silent pass) -- keep SRCS and HDRS in step.
- The archived sources still include the **live** `igs_ifx_common.h` (for
  `igs::image::rgba` / `igs::color::ref_value`). A future edit there changes what the
  golden records, which is exactly how the Toonz `b,g,r` order bit us -- re-run this
  generator and re-check the port if that header changes.

Usage:
    python tests/native/gen_igs_motion_wind_cases.py [--check]
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
    "flare/sources/stdfx/igs_motion_wind_pixel.cpp",
    "flare/sources/stdfx/igs_motion_wind_table.cpp",
]
HDRS = [
    "flare/sources/stdfx/igs_motion_wind_pixel.h",
    "flare/sources/stdfx/igs_motion_wind_table.h",
]
GOLDEN = os.path.join(ms.ROOT, "flare", "sources", "stdfx", "igs_motion_wind_cases.txt")
HARNESS = os.path.join(HERE, "igs_motion_wind_ref.cpp")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true", help="do not write; exit 1 on drift")
    args = ap.parse_args()

    rev = ms.pre_deletion_revision(SRCS[0])
    stdfx = os.path.join(ms.ROOT, "flare", "sources", "stdfx")
    archive = os.path.join(ms.ROOT, "build", "libflare_formats.a")
    if not os.path.exists(archive):
        sys.exit(
            f"{archive} is missing: the reference harness needs the Rust LCG because "
            "igs_math_random.h forwards to flare_igs_random_next*. Build it first:\n"
            "  cargo build --release --manifest-path flare/sources/Cargo.toml --lib"
        )
    with tempfile.TemporaryDirectory(prefix="igswind-") as workdir:
        ms.extract(rev, SRCS + HDRS, workdir)
        fresh = ms.build_and_run_host(
            workdir,
            [os.path.basename(s) for s in SRCS],
            HARNESS,
            include_dirs=[stdfx, workdir],
            link_archives=[archive],
        )

    return ms.check_or_write(GOLDEN, fresh, rev, args.check, count="lines")


if __name__ == "__main__":
    raise SystemExit(main())
