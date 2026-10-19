"""Regenerate the goldens for the PatternPosition / SDirection / PatternMapParam ports.

    flare/sources/flarelib/sandor_fxs/PatternPosition_cases.txt
    flare/sources/flarelib/sandor_fxs/SDirection_cases.txt
    flare/sources/flarelib/sandor_fxs/PatternMapParam_cases.txt

Each is the output of the *original* C++ (``CPatternPosition``, ``CSDirection``,
``CPatternMapParam``), rebuilt from git history with the host C++ compiler. The Rust ports
(``PatternPosition.rs`` etc.) replay the cases and must match byte for byte, so the
goldens carry their own inputs (masks, argument lists) instead of mirroring a case table.

Platform independence: ``PatternPosition.cpp`` calls ``rand()``. The reference TU
(``pp_orig.cpp``) redirects it to a fixed LCG (``ref_rand``, see patternposition_ref.cpp)
which the Rust test replays, and ``RAND_MAX`` is pinned to glibc's 2147483647 on the Rust
side. ``PatternMapParam`` uses the host ``atof`` / ``atoi`` on both sides, so its golden is
only exact against the C runtime it was recorded on (glibc here); hex-float / ``inf`` /
``nan`` tokens are the ones a different libc may parse differently.

Mocks written next to the extracted sources (the real headers pull in the raster stack):
``STColSelPic.h``, ``CIL.h``, ``Pattern.h``.

Usage:
    python tests/native/gen_sandor_fxs_cases.py [--check]
"""
from __future__ import annotations

import argparse
import os
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import msvc_reference as ms  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
SANDOR = "flare/sources/flarelib/sandor_fxs"
OUT = os.path.join(ms.ROOT, *SANDOR.split("/"))

MOCK_STCOLSELPIC = """#pragma once
#include <memory>
#include "SDef.h"
template <class P>
struct CSTColSelPic {
  int m_lX = 0, m_lY = 0;
  std::shared_ptr<UCHAR> m_sel;
  P *m_pic = nullptr;
};
"""
MOCK_CIL = """#pragma once
#include <string.h>
#include <algorithm>
class CCIL {
public:
  int m_nb;
  int m_ci[4096];
  CCIL() : m_nb(0) {}
  void set(const char *s, const int) { m_nb = s ? (int)strlen(s) : 0; }
};
"""
MOCK_PATTERN = "#pragma once\n#include <algorithm>\n"
PP_ORIG = """#include <stdlib.h>
#include <cstdlib>
extern "C" int ref_rand(void);
#define rand ref_rand
#include "PatternPosition.cpp"
"""

# (golden, original sources, harness, extra files to write, sources to compile)
JOBS = [
    (
        "PatternPosition_cases.txt",
        ["PatternPosition.cpp", "PatternPosition.h", "SDef.h", "SError.h"],
        "patternposition_ref.cpp",
        {"STColSelPic.h": MOCK_STCOLSELPIC, "pp_orig.cpp": PP_ORIG},
        ["pp_orig.cpp"],
    ),
    (
        "SDirection_cases.txt",
        ["SDirection.cpp", "SDirection.h", "SDef.h", "SError.h"],
        "sdirection_ref.cpp",
        {},
        ["SDirection.cpp"],
    ),
    (
        "PatternMapParam_cases.txt",
        ["PatternMapParam.cpp", "PatternMapParam.h", "SDef.h", "SError.h"],
        "patternmapparam_ref.cpp",
        {"CIL.h": MOCK_CIL, "Pattern.h": MOCK_PATTERN},
        ["PatternMapParam.cpp"],
    ),
]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true", help="do not write; exit 1 on drift")
    args = ap.parse_args()
    rc = 0
    for golden, srcs, harness, extra, compile_ in JOBS:
        cpp = f"{SANDOR}/{srcs[0]}"
        rev = ms.pre_deletion_revision(cpp)
        with tempfile.TemporaryDirectory(prefix="sandor-") as workdir:
            ms.extract(rev, [f"{SANDOR}/{s}" for s in srcs], workdir)
            for name, body in extra.items():
                with open(os.path.join(workdir, name), "w") as fh:
                    fh.write(body)
            fresh = ms.build_and_run_host(
                workdir, compile_, os.path.join(HERE, harness), include_dirs=[workdir]
            )
        rc |= ms.check_or_write(os.path.join(OUT, golden), fresh, rev, args.check)
    return rc


if __name__ == "__main__":
    raise SystemExit(main())
