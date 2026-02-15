#!/usr/bin/env python3
"""Conservative rebrand: replace Flare -> Flare in source code and user-facing strings.

Rules:
- Replace 'Flare' -> 'Flare' and 'flare' -> 'flare' in non-URL contexts.
- Replace '/flare/' -> '/flare/' and 'flare/flare' -> 'flare/flare'.
- Do NOT replace shortened 'toonz' inside identifiers like 'toonzqt' to avoid breaking code.
- Skip files/directories in EXCLUSIONS and skip lines containing URLs (http(s) or common Flare docs/links).

This script is conservative and focuses on user-visible strings and path segments.
"""

from pathlib import Path
import re

print('SCRIPT START')

ROOT = Path(__file__).resolve().parents[2]
EXCLUSIONS = {'.git', '.github', 'thirdparty', 'doc', 'stuff/doc', 'tools/rebrand'}
INCLUDE_GLOBS = ['**/*.cpp', '**/*.h', '**/*.c', '**/*.py', '**/*.qml', '**/*.json', '**/*.xml', '**/*.yml', '**/*.desktop', '**/*.in', '**/*.txt', '**/*.sh', '**/*.bat', '**/*.cmake', '**/*.rc', '**/*.qrc']
SKIP_FILES = {'README.md'}

# lines containing any of these substrings will be skipped from replacement
URL_INDICATORS = ['http://', 'https://', 'github.com/opentoonz', 'opentoonz.readthedocs', 'opentoonz.github.io']

# regex for word-boundary matches
RE_OPEN = re.compile(r"\bOpenToonz\b")
RE_LOWER = re.compile(r"\bopentoonz\b")
RE_TR_OPEN = re.compile(r'(tr\(\s*")([^"]*OpenToonz[^"]*)("\s*\))')
RE_QT_TR_OPEN = re.compile(r'(QObject::tr\(\s*")([^"]*OpenToonz[^"]*)("\s*\))')

PATH_REPLACEMENTS = [
    (re.compile(r"/flare/"), "/flare/"),
    (re.compile(r"flare/flare"), "flare/flare"),
]

SPECIAL_REPLACEMENTS = [
    # some specific user-visible strings
    (re.compile(r"About Flare"), "About Flare"),
    (re.compile(r"Flare Startup"), "Flare Startup"),
    (re.compile(r"Show Startup Window when Flare Starts"), "Show Startup Window when Flare Starts"),
    (re.compile(r"Flare crashed"), "Flare crashed"),
    (re.compile(r"My Documents/Flare\*"), "My Documents/Flare*"),
    (re.compile(r"Desktop/Flare\*"), "Desktop/Flare*"),
]


def line_has_url(line: str) -> bool:
    low = line.lower()
    return any(ind in low for ind in URL_INDICATORS)


def process_file(path: Path) -> int:
    text = path.read_text(encoding='utf-8', errors='replace')
    orig = text
    lines = text.splitlines(True)
    changed = False

    for i, line in enumerate(lines):
        if line_has_url(line):
            continue
        new_line = line
        for pat, repl in PATH_REPLACEMENTS:
            new_line = pat.sub(repl, new_line)
        for pat, repl in SPECIAL_REPLACEMENTS:
            new_line = pat.sub(repl, new_line)
        # word-boundary replacements
        if 'Flare' in new_line:
            new_line = RE_OPEN.sub('Flare', new_line)
        # lowercase
        if 'flare' in new_line.lower():
            new_line = RE_LOWER.sub('flare', new_line)

        if new_line != line:
            lines[i] = new_line
            changed = True

    if changed:
        new_text = ''.join(lines)
        path.write_text(new_text, encoding='utf-8')
        return 1
    return 0


def replace_in_tr_strings(text):
    # Replace OpenToonz inside tr() strings and QObject::tr() occurrences
    new_text = text
    count = 0

    def _repl_tr(m):
        nonlocal count
        inner = m.group(2)
        if any(ind in inner.lower() for ind in URL_INDICATORS):
            return m.group(0)
        new_inner = inner.replace('Flare', 'Flare')
        count += 1
        return m.group(1) + new_inner + m.group(3)

    new_text = RE_TR_OPEN.sub(_repl_tr, new_text)
    new_text = RE_QT_TR_OPEN.sub(_repl_tr, new_text)

    return new_text, count


if __name__ == '__main__':
    files = []
    for glob in INCLUDE_GLOBS:
        files.extend(ROOT.glob(glob))

    print(f'Files matched: {len(files)}')
    total = 0
    for f in files:
        if any(part in EXCLUSIONS for part in f.parts):
            continue
        if f.name in SKIP_FILES:
            continue
        try:
            text = f.read_text(encoding='utf-8', errors='replace')
        except Exception as e:
            print(f'Error reading {f}: {e}')
            continue

        changed = False
        # first, replace specifically inside tr() strings
        new_text, replaced_tr = replace_in_tr_strings(text)
        if replaced_tr > 0:
            print(f'Updated {replaced_tr} tr() occurrences in: {f}')
            changed = True

        # then run the general process for other replacements on non-URL lines
        if not changed:
            if 'OpenToonz' in text or 'opentoonz' in text.lower():
                print(f'Processing: {f}')
                try:
                    if process_file(f):
                        print(f'Updated: {f}')
                        total += 1
                except Exception as e:
                    print(f'Error processing {f}: {e}')
        else:
            # write the tr()-modified text back and count it as an update
            f.write_text(new_text, encoding='utf-8')
            total += 1

    print(f'Rebrand script completed. Files changed: {total}')
