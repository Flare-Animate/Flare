#!/usr/bin/env python3
"""Replace 'OpenToonz' -> 'Flare' in .ts translation files (user-visible strings).
This script only replaces the exact capitalization 'OpenToonz' to avoid touching URLs or other lowercase occurrences.
Run locally to update translation files before committing.
"""
from __future__ import annotations

import os
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
TS_DIR = os.path.join(ROOT, "flare", "sources", "translations")

replacements = [("OpenToonz", "Flare")]

changed_files = []
for dirpath, dirnames, filenames in os.walk(TS_DIR):
    for fn in filenames:
        if fn.endswith('.ts'):
            fp = os.path.join(dirpath, fn)
            with open(fp, 'r', encoding='utf-8') as f:
                data = f.read()
            new = data
            for old, newv in replacements:
                new = new.replace(old, newv)
            if new != data:
                with open(fp, 'w', encoding='utf-8') as f:
                    f.write(new)
                changed_files.append(fp)

print(f"Updated {len(changed_files)} translation files:")
for p in changed_files:
    print(' -', os.path.relpath(p, ROOT))

if not changed_files:
    print('No translation files needed changes.')

if __name__ == '__main__':
    sys.exit(0)
