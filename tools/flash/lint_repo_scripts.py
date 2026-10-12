#!/usr/bin/env python3
"""Scan repository for .jsfl/.js/.as files and run a syntax check on each.

Writes 'repo_script_problems.json' with a mapping from relative path -> list of problems.
"""
from __future__ import annotations

import json
import os
import sys
from typing import Dict, List

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
EXTS = (".jsfl", ".js", ".as")

# Directories that hold other people's code. Linting them reports problems in
# files Flare does not maintain and cannot fix, which made this suite fail on
# thirdparty/libjpeg-turbo's bundled jQuery/ search.js rather than on anything
# in Flare's own tree.
SKIP_DIRS = (
    ".git", "build", "dist", "venv", ".venv", "node_modules",
    "thirdparty", "vcpkg", ".github",
)


def lint_file(path: str) -> List[Dict[str, str]]:
    problems = []
    try:
        import esprima  # type: ignore
        with open(path, "r", encoding="utf-8") as f:
            src = f.read()
        try:
            esprima.parseScript(src)
        except Exception as e:
            problems.append({"type": "syntax", "message": str(e)})
    except Exception:
        with open(path, "r", encoding="utf-8") as f:
            src = f.read()
        if src.count("{") != src.count("}") or src.count("(") != src.count(")") or src.count("[") != src.count("]"):
            problems.append({"type": "syntax", "message": "Mismatched parentheses/braces/brackets (esprima not installed)"})
    return problems


def main() -> int:
    problems: Dict[str, List[Dict[str, str]]] = {}
    scanned = 0

    for root, dirs, files in os.walk(ROOT):
        # Prune in place so the walk never descends into vendored trees.
        dirs[:] = [d for d in dirs if d not in SKIP_DIRS]
        for fn in files:
            if fn.lower().endswith(EXTS):
                full = os.path.join(root, fn)
                rel = os.path.relpath(full, ROOT)
                scanned += 1
                probs = lint_file(full)
                if probs:
                    problems[rel] = probs
                    print(f"Problems in {rel}: {probs}", file=sys.stderr)

    # Keep the report out of the working tree -- it is a build artifact, and
    # writing it at the repo root dirtied `git status` on every run.
    out = os.path.join(ROOT, "build", "repo_script_problems.json")
    os.makedirs(os.path.dirname(out), exist_ok=True)
    with open(out, "w", encoding="utf-8") as f:
        json.dump(problems, f, indent=2)

    print(f"Scanned {scanned} script files, found problems in {len(problems)} files")

    return 0


if __name__ == "__main__":
    sys.exit(main())
