#!/usr/bin/env python3
"""Fail if a CMakeLists.txt still lists a .cpp that was ported to Rust.

A stale entry is a path that is missing on disk while a same-name .rs sits beside
it (the porting convention in docs/PORTING.md).

Porting a .cpp to Rust deletes the file; a stale CMake entry then breaks
configure on every platform ("Cannot find source file"). Run from the repo root.
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SRC_RE = re.compile(r'(?<![\w$./-])((?:\.\./|\./)?[\w./+-]+\.(?:cpp|cc|c|mm))(?![\w])')
SKIP_DIRS = {"thirdparty", "target", ".git", "build", ".claude"}


def cmake_files():
    for p in (ROOT / "flare" / "sources").rglob("CMakeLists.txt"):
        if not SKIP_DIRS & set(p.relative_to(ROOT).parts):
            yield p


def main() -> int:
    missing = []
    for cm in cmake_files():
        for n, line in enumerate(cm.read_text(errors="replace").splitlines(), 1):
            code = line.split("#", 1)[0]
            if "${" in code or "$<" in code:
                continue
            for m in SRC_RE.finditer(code):
                target = cm.parent / m.group(1)
                if not target.exists() and target.with_suffix(".rs").exists():
                    missing.append(f"{cm.relative_to(ROOT)}:{n}: {m.group(1)}")
    for m in missing:
        print(f"missing source: {m}")
    return 1 if missing else 0


if __name__ == "__main__":
    sys.exit(main())
