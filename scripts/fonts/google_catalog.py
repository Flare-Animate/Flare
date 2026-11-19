#!/usr/bin/env python3
"""Writes the Google Fonts catalog Flare offers in its font lists.

The catalog names each family, its category and the font files that set it, as they are in the
google/fonts repository at one commit. Flare downloads a family's files from that commit only
when someone sets text in it, so the addresses never go stale. Run it to move to a newer
commit:

    python3 scripts/fonts/google_catalog.py [commit]

It reads the repository's file list from the GitHub API and the families' names and categories
from fonts.google.com, and writes crates/foundation/flare-text/data/google-fonts.tsv.
"""

import collections
import json
import pathlib
import re
import sys
import urllib.request

REPO = "google/fonts"
# Larger files (whole Chinese type families, the colour emoji test font) are left out: a font
# list should not offer what takes minutes to download. flare-text holds the same limit.
MAX_FILE_BYTES = 32 * 1024 * 1024
# The longest family name flare-document accepts.
MAX_FAMILY_CHARS = 256
OUT = pathlib.Path(__file__).resolve().parents[2] / "crates/foundation/flare-text/data/google-fonts.tsv"


def get_json(url):
    request = urllib.request.Request(url, headers={"User-Agent": "flare-catalog-script"})
    with urllib.request.urlopen(request, timeout=120) as response:
        text = response.read().decode("utf-8")
    # fonts.google.com prefixes its JSON with a line that stops it being run as a script.
    return json.loads(text[text.index("{"):])


def key(name):
    return re.sub("[^a-z0-9]", "", name.lower())


def pick(files):
    """The files a family is set with, as (style, name): r, b, i and bi for regular, bold,
    italic and bold italic. A variable file covers every weight, so it stands alone."""
    upright = [f for f in files if "italic" not in f.lower()]
    italic = [f for f in files if "italic" in f.lower()]
    chosen = []
    for files, base, bold in ((upright, "r", "b"), (italic, "i", "bi")):
        variable = [f for f in files if "[" in f]
        if variable:
            chosen.append((base, min(variable, key=len)))
            continue
        regular = [f for f in files if re.search(r"-(Regular|Italic)\.ttf$", f)]
        bolds = [f for f in files if re.search(r"-Bold(Italic)?\.ttf$", f)]
        if regular:
            chosen.append((base, regular[0]))
        elif files:
            chosen.append((base, sorted(files)[0]))
        if bolds:
            chosen.append((bold, bolds[0]))
    return chosen


def main():
    commit = sys.argv[1] if len(sys.argv) > 1 else get_json(
        f"https://api.github.com/repos/{REPO}/commits/main")["sha"]
    tree = get_json(f"https://api.github.com/repos/{REPO}/git/trees/{commit}?recursive=1")
    if tree.get("truncated"):
        sys.exit("the repository's file list came back cut short")
    folders = collections.defaultdict(dict)
    for entry in tree["tree"]:
        parts = entry["path"].split("/")
        if len(parts) == 3 and parts[0] in ("ofl", "apache", "ufl") and parts[2].endswith(".ttf"):
            if entry["size"] <= MAX_FILE_BYTES:
                folders[(parts[0], parts[1])][parts[2]] = entry["size"]
    by_key = {folder: place for place in folders for folder in [place[1]]}
    # A few folders are not the family's name squeezed together; their files' names are.
    by_stem = {}
    for place, files in folders.items():
        for name in files:
            by_stem.setdefault(key(re.split(r"[-\[.]", name)[0]), place)

    families = get_json("https://fonts.google.com/metadata/fonts")["familyMetadataList"]
    lines, missing = [], []
    for family in sorted(families, key=lambda f: f["family"].lower()):
        name = family["family"]
        place = by_key.get(key(name)) or by_stem.get(key(name))
        if not place or "\t" in name or "|" in name or len(name) > MAX_FAMILY_CHARS:
            missing.append(name)
            continue
        chosen = pick(sorted(folders[place]))
        if not chosen:
            missing.append(name)
            continue
        files = "|".join(f"{style}:{file}" for style, file in chosen)
        lines.append(f"{name}\t{family['category']}\t{place[0]}/{place[1]}\t{files}")

    OUT.parent.mkdir(parents=True, exist_ok=True)
    with OUT.open("w", encoding="utf-8", newline="\n") as out:
        out.write(f"# Google Fonts at github.com/{REPO}, written by scripts/fonts/google_catalog.py.\n")
        out.write("# Each family: name, category, folder, then style:file for its files.\n")
        out.write(f"commit\t{commit}\n")
        for line in lines:
            out.write(line + "\n")
    print(f"{len(lines)} families at {commit}; left out: {', '.join(missing) or 'none'}")


if __name__ == "__main__":
    main()
