"""Regenerate the golden for the `igs_gaussian_blur.cpp` port.

Records what the *original* `igs_gaussian_blur.cpp` produced for 14 float images
(4/3/1 channels, with and without a reference, equal-channel reuse, non-default sigma,
the `real_radius <= 0` no-op). The original `#include`d `igs_gauss_distribution.cpp`
as source; that file is itself retired (Rust), so both come from git history and a
generated `igs_gauss_distribution.h` stub points the former at the latter.

The brush goes through the host libm `exp`, so the Rust test accepts 4 float ULP where
the bits differ. Built with `-ffp-contract=off`.

Usage:
    python tests/native/gen_igs_gaussian_blur_cases.py [--check]
"""

from __future__ import annotations

import argparse
import os
import shutil
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import msvc_reference as ms  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
SRCS = [
    "flare/sources/stdfx/igs_gaussian_blur.cpp",
    "flare/sources/stdfx/igs_gaussian_blur.h",
]
GAUSS = "flare/sources/stdfx/igs_gauss_distribution.cpp"
GOLDEN = os.path.join(ms.ROOT, "flare", "sources", "stdfx", "igs_gaussian_blur_cases.txt")
HARNESS = os.path.join(HERE, "igs_gaussian_blur_ref.cpp")


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
    gauss_rev = ms.pre_deletion_tree(GAUSS)
    stdfx = os.path.join(ms.ROOT, "flare", "sources", "stdfx")
    with tempfile.TemporaryDirectory(prefix="igsgauss-") as workdir:
        ms.extract(rev, SRCS, workdir)
        ms.extract(gauss_rev, [GAUSS], workdir)
        os.replace(
            os.path.join(workdir, "igs_gauss_distribution.cpp"),
            os.path.join(workdir, "igs_gauss_distribution_orig.cpp"),
        )
        with open(os.path.join(workdir, "igs_gauss_distribution.h"), "w", encoding="utf-8") as fh:
            fh.write('#pragma once\n#include "igs_gauss_distribution_orig.cpp"\n')
        # the harness #includes nothing but the header; compile the original cpp alongside
        fresh = ms.build_and_run_host(
            workdir, ["igs_gaussian_blur.cpp"], HARNESS, include_dirs=[workdir, stdfx]
        )
    rc = ms.check_or_write(GOLDEN, fresh, f"{rev}+{gauss_rev}", args.check, count="lines")
    if rc == 0 and args.verify_chain:
        rc = ms.verify_chain(HARNESS, GOLDEN, [stdfx])
    return rc


if __name__ == "__main__":
    raise SystemExit(main())
