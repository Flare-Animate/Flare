"""Regenerate/check flare/sources/flarelib/namebuilder_cases.txt from the original C++."""
import rw3_golden as g

raise SystemExit(g.run(
    "namebuilder", "flarelib/namebuilder_cases.txt",
    ["flare/sources/flarelib/namebuilder.cpp", "flare/sources/include/flare/namebuilder.h"],
    hdr_as={"flare/sources/include/flare/namebuilder.h": "flare/namebuilder.h"},
    stubs={"tconvert.h": "#pragma once\n"},
))
