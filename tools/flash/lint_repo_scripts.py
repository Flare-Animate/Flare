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

    for root, _, files in os.walk(ROOT):
        # Skip virtualenvs, build directories and .git
        if any(p in root for p in (".git", "build", "dist", "venv", ".venv")):
            continue
        for fn in files:
            if fn.lower().endswith(EXTS):
                full = os.path.join(root, fn)
                rel = os.path.relpath(full, ROOT)
                scanned += 1
                probs = lint_file(full)
                if probs:
                    problems[rel] = probs
                    print(f"Problems in {rel}: {probs}", file=sys.stderr)

    out = os.path.join(ROOT, "repo_script_problems.json")
    with open(out, "w", encoding="utf-8") as f:
        json.dump(problems, f, indent=2)

    print(f"Scanned {scanned} script files, found problems in {len(problems)} files")

    return 0


if __name__ == "__main__":
    sys.exit(main())
