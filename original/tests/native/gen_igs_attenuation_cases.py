"""Regenerate flare/sources/stdfx/igs_attenuation_cases.txt: the port's golden output.

The golden is what the *original* C++ `igs_attenuation_distribution.cpp` (recovered
from git history, since the port deletes it) produced for a grid of
(radius, curve, polygon, degree) inputs: the odd diameter, the per-row offset/size,
where `lens_starts[y]` points relative to the row, and every matrix value as raw
IEEE-754 hex.

The Rust port (`stdfx/igs_attenuation_distribution.rs`) is checked against it in
`tests::matches_the_cpp_reference`: structural fields exactly, matrix values within
a few ULP, because `sqrt`/`pow`/`cos`/`sin` are allowed to differ between the
recording compiler's libm and the one running the test.

Run this on the machine whose golden you intend to commit (the harness uses the host
C++ compiler through `msvc_reference.build_and_run_host`).

Usage:
    python tests/native/gen_igs_attenuation_cases.py [--check]
"""

from __future__ import annotations

import argparse
import os
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import msvc_reference as ms  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
SRC = "flare/sources/stdfx/igs_attenuation_distribution.cpp"
GOLDEN = os.path.join(ms.ROOT, "flare", "sources", "stdfx", "igs_attenuation_cases.txt")
HARNESS = os.path.join(HERE, "igs_attenuation_ref.cpp")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true", help="do not write; exit 1 on drift")
    args = ap.parse_args()

    rev = ms.pre_deletion_revision(SRC)
    with tempfile.TemporaryDirectory(prefix="igsatt-") as workdir:
        ms.extract(rev, [SRC], workdir)
        # The harness #includes the archived .cpp itself (its helpers are in an
        # anonymous namespace, so this cannot collide with flare_formats).
        fresh = ms.build_and_run_host(workdir, [], HARNESS)

    return ms.check_or_write(GOLDEN, fresh, rev, args.check, count="cases")


if __name__ == "__main__":
    raise SystemExit(main())
