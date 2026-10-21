#!/usr/bin/env python3
"""Fail if a header defines a qualified function (Ns::f / Class::f) out of class
without `inline`.

Porting a .cpp to Rust sometimes moves the C++ glue into the header. An out-of-class
`void TRop::f(...) {` at namespace scope is rejected by gcc (explicit qualification)
and, if accepted, duplicated in every translation unit (clang/MSVC/ld: multiple
definition / LNK2005). Define it `inline` inside the namespace instead.
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
HDR = ROOT / "flare" / "sources"
DEF = re.compile(r"^(?!\s)(?!inline\b|static\b|template\b|extern\b|typedef\b|return\b|class\b|struct\b|namespace\b|using\b|#|//|\})[\w:<>,*&~ ]+?\b(\w+::~?\w+)\s*\([^;]*$")


# Pre-existing header-only class used from a single translation unit.
ALLOW = {"flare/sources/include/tversion.h"}


def main() -> int:
    bad = []
    for p in sorted(HDR.rglob("*.h")):
        if "thirdparty" in p.parts or "target" in p.parts:
            continue
        if p.relative_to(ROOT).as_posix() in ALLOW:
            continue
        lines = p.read_text(errors="replace").splitlines()
        in_comment = False
        for i, line in enumerate(lines, 1):
            if "/*" in line and "*/" not in line:
                in_comment = True
            if in_comment:
                if "*/" in line:
                    in_comment = False
                continue
            if not DEF.match(line):
                continue
            # a definition has `{` before the next `;` within a few lines
            tail = " ".join(lines[i - 1 : i + 6])
            if re.search(r"\)\s*(const\s*)?(noexcept\s*)?\{", tail.split(";")[0] + ";"):
                prev = lines[i - 2].strip() if i >= 2 else ""
                if prev.startswith("inline") or prev.startswith("template"):
                    continue
                bad.append(f"{p.relative_to(ROOT)}:{i}: {line.strip()}")
    for b in bad:
        print(f"non-inline out-of-class definition in header: {b}")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
