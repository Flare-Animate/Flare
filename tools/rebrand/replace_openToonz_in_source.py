#!/usr/bin/env python3
"""Conservative rebrand: replace Flare -> Flare in source code and user-facing strings.

Rules:
- Replace capitalized 'OpenToonz' -> 'Flare' and lowercase 'opentoonz' -> 'flare' except when part of a URL (http(s) or github.com/opentoonz or opentoonz.readthedocs.io) or when inside 'opentoonz.github.io', to avoid breaking upstream links.
- Replace '/flare/' -> '/flare/' and 'flare/flare' -> 'flare/flare'.
- Do NOT replace bare 'toonz' within identifiers or module names (e.g., 'toonzqt').
- Skip files under exclusions like .git, .github, thirdparty, doc, stuff/doc, and tools/rebrand.

This script is conservative and focuses on user-visible strings and path segments.
"""

from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[2]
EXCLUSIONS = ['.git', '.github', 'thirdparty', 'doc', 'stuff/doc', 'tools/rebrand']
INCLUDE_GLOBS = ['**/*.cpp', '**/*.h', '**/*.c', '**/*.py', '**/*.qml', '**/*.json', '**/*.xml', '**/*.yml', '**/*.desktop', '**/*.in', '**/*.txt', '**/*.sh', '**/*.bat', '**/*.cmake', '**/*.rc', '**/*.qrc']
SKIP_FILES = ['README.md']

url_indicators = ['http://', 'https://', 'github.com/opentoonz', 'opentoonz.readthedocs', 'opentoonz.github.io']

replacements = [
    (re.compile(r'/flare/'), '/flare/'),
    (re.compile(r'flare/flare'), 'flare/flare'),
]

# Capitalization aware replacements
# Will replace 'Flare' -> 'Flare' and standalone 'flare' -> 'flare'

def should_skip_line(line: str) -> bool:
    lower = line.lower()
    for u in url_indicators:
        if u in lower:
            return True
    return False

def process_file(path: Path) -> int:
    text = path.read_text(encoding='utf-8')
    orig = text
    lines = text.splitlines(True)
    changed = False
    for i, line in enumerate(lines):
        # skip lines with URLs to Flare docs or repo
        if should_skip_line(line):
            continue
        new_line = line
        # apply path-based replacements
        for pat, repl in replacements:
            new_line = pat.sub(repl, new_line)
        # replace Flare (capitalized)
        if 'Flare' in new_line:
            new_line = new_line.replace('Flare', 'Flare')
        # replace lowercase flare when not in URLs
        if 'flare' in new_line:
            # avoid changing 'flare' as part of identifiers? We'll replace it conservatively
            new_line = new_line.replace('flare', 'flare')
        if new_line != line:
            lines[i] = new_line
            changed = True
    if changed:
        new_text = ''.join(lines)
        # Quick sanity: do not change files to have too many changes; we log and write
        path.write_text(new_text, encoding='utf-8')
        return 1
    return 0

if __name__ == '__main__':
    total = 0
    files = []
    for glob in INCLUDE_GLOBS:
        files.extend(ROOT.glob(glob))
    for f in files:
        if any(part in EXCLUSIONS for part in f.parts):
            continue
        if f.name in SKIP_FILES:
            continue
        try:
            if process_file(f):
                print(f'Updated: {f}')
                total += 1
        except Exception as e:
            print(f'Error processing {f}: {e}')
    print(f'Rebrand script completed. Files changed: {total}')
