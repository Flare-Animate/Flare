"""Regenerate the golden for the `igs_maxmin*.cpp` ports.

Records what the *original* `igs_maxmin.cpp`, `igs_maxmin_lens_matrix.cpp` and
`igs_maxmin_slrender.cpp` (with `igs_maxmin_getput.h` / `igs_maxmin_multithread.h`,
recovered from git history because the port deletes them) produced for 14 lens
configurations: the lens matrices (fresh and reshaped) and 126 end-to-end image
hashes. The threading uses the real `igs_resource_multithread.cpp` on pthreads.

Weights of polygon lenses go through the host libm (`cos`/`sin`/`atan2`); the Rust
test compares them within 4 ULP and requires the image hashes exactly for circles
(sqrt only) and for polygons whenever its lenses are bit-identical. Reference images
are 8/16-bit only (a float reference is undefined behaviour in the original). RGBA /
reference cases are single threaded because the original races on its shared lens.
Built with `-ffp-contract=off -pthread`.

Usage:
    python tests/native/gen_igs_maxmin_cases.py [--check]
"""

from __future__ import annotations

import argparse
import os
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import msvc_reference as ms  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
STDFX = "flare/sources/stdfx/"
SRCS = [STDFX + n for n in (
    "igs_maxmin.cpp",
    "igs_maxmin_lens_matrix.cpp",
    "igs_maxmin_slrender.cpp",
)]
HDRS = [STDFX + n for n in (
    "igs_maxmin.h",
    "igs_maxmin_getput.h",
    "igs_maxmin_lens_matrix.h",
    "igs_maxmin_multithread.h",
    "igs_maxmin_slrender.h",
)]
# live, unported threading helpers
LIVE = [
    "igs_resource_multithread.cpp",
    "igs_resource_thread.cpp",
    "igs_resource_msg_from_err.cpp",
]
GOLDEN = os.path.join(ms.ROOT, "flare", "sources", "stdfx", "igs_maxmin_cases.txt")
HARNESS = os.path.join(HERE, "igs_maxmin_ref.cpp")


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
    with tempfile.TemporaryDirectory(prefix="igsmaxmin-") as workdir:
        ms.extract(rev, SRCS + HDRS, workdir)
        fresh = ms.build_and_run_host(
            workdir,
            [os.path.basename(s) for s in SRCS] + [os.path.join(stdfx, n) for n in LIVE],
            HARNESS,
            include_dirs=[workdir, stdfx],
            extra_args=["-pthread"],
        )
    rc = ms.check_or_write(GOLDEN, fresh, rev, args.check, count="lines")
    if rc == 0 and args.verify_chain:
        # only the end-to-end lines: the lens API no longer exists in C++
        rc = ms.verify_chain(HARNESS, GOLDEN, [stdfx], keep=lambda l: l.startswith("e2e "))
    return rc


if __name__ == "__main__":
    raise SystemExit(main())
