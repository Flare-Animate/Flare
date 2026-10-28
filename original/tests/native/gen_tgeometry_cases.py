"""Regenerate/check flare/sources/common/tgeometry/tgeometry_cases.txt from the original C++."""
import rw3_golden as g

raise SystemExit(g.run(
    "tgeometry", "common/tgeometry/tgeometry_cases.txt",
    ["flare/sources/common/tgeometry/tgeometry.cpp", "flare/sources/include/tgeometry.h"],
    orig_marker="TAffine TAffine::inv() const {",
    compile=["tgeometry.cpp"],
    chain_compile=["flare/sources/common/tgeometry/tgeometry.cpp"],
))
