#!/usr/bin/env python3
"""Guard the Theme / Layout split.

Flare has two independent axes in Preferences > Interface:

  Theme  - colour and shape of the widgets, one stylesheet per directory under
           stuff/config/qss/<Name>/<Name>.qss
  Layout - where the panels sit, one room directory under
           stuff/profiles/layouts/rooms/<Name>/

Any Theme can be combined with any Layout. They must not share a name, because
the two lists are shown together and "Animate" meaning two different things is
exactly the confusion this check exists to catch -- both namespaces carried an
"Animate" at one point.

Also checks that a theme really covers the widget set, directly or via @import.
LiquidGlass shipped as a 33-line fragment while every other theme was ~3356
lines, so selecting it left most of the UI unstyled.

    python scripts/check_theme_layout_separation.py
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
THEMES = REPO / "stuff" / "config" / "qss"
LAYOUTS = REPO / "stuff" / "profiles" / "layouts" / "rooms"

# The reference theme defines what "fully covered" means. Uses the most common
# selector count rather than the maximum, so one outlier cannot make every other
# theme look incomplete.
COMMON_FLOOR = 500


def dirs(path: Path) -> list[str]:
    return sorted(p.name for p in path.iterdir() if p.is_dir()) if path.is_dir() else []


def stylesheet(theme: str) -> Path:
    return THEMES / theme / f"{theme}.qss"


def selectors(text: str) -> set[str]:
    out = set()
    for line in text.splitlines():
        m = re.match(r"^([^{}@][^{}]*?)\s*\{", line)
        if m:
            out.add(m.group(1).strip())
    return out


def imports(text: str) -> list[str]:
    # Two capture groups, so findall yields (quote, specifier) pairs; take the
    # specifier. Taking the whole match instead would keep the quote character.
    return [
        spec
        for _, spec in re.findall(
            r"""@import\s+(['"])(?!https?:|/|data:)([^'"]+)\1""", text
        )
    ]


def theme_dir(spec: str) -> str:
    return spec.replace("\\", "/").rstrip("/").split("/")[-1].removesuffix(".qss")


def effective(theme: str, seen: set[str] | None = None) -> set[str]:
    seen = seen or set()
    path = stylesheet(theme)
    if not path.is_file() or theme in seen:
        return set()
    seen.add(theme)
    text = path.read_text(encoding="utf-8", errors="replace")
    out = selectors(text)
    for spec in imports(text):
        out |= effective(theme_dir(spec), seen)
    return out


def main() -> int:
    problems: list[str] = []

    if not THEMES.is_dir():
        print(f"FAIL: no themes directory at {THEMES}")
        return 1
    if not LAYOUTS.is_dir():
        print(f"FAIL: no layouts directory at {LAYOUTS}")
        return 1

    themes = dirs(THEMES)
    layouts = dirs(LAYOUTS)
    print(f"themes : {', '.join(themes) or '(none)'}")
    print(f"layouts: {', '.join(layouts) or '(none)'}\n")

    # 1. disjoint namespaces
    clash = sorted(set(themes) & set(layouts))
    if clash:
        problems.append(
            "name(s) used as BOTH a Theme and a Layout: "
            + ", ".join(clash)
            + "\n       one is the widget appearance, the other is the panel "
            "layout; they must be distinguishable"
        )
    else:
        print("PASS  Theme and Layout namespaces are disjoint")

    # 2. every theme covers the widget set
    covered = {t: len(effective(t)) for t in themes}
    full = max(covered.values()) if covered else 0
    for theme, n in sorted(covered.items()):
        flag = "PASS" if n >= COMMON_FLOOR else "FAIL"
        print(f"{flag}  theme {theme:<16} {n} selectors")
        if n < COMMON_FLOOR:
            problems.append(
                f"theme {theme!r} covers only {n} selectors; "
                f"a theme must cover ~{full} or it leaves the UI unstyled"
            )

    # 3. each theme's file exists and is non-empty
    for theme in themes:
        p = stylesheet(theme)
        if not p.is_file():
            problems.append(f"theme {theme!r} has no {theme}.qss")
        elif p.stat().st_size == 0:
            problems.append(f"theme {theme!r} has an empty {theme}.qss")

    if problems:
        print()
        for p in problems:
            print("FAIL: " + p)
        print(f"\n{len(problems)} problem(s)")
        return 1

    print(f"\nOK: {len(themes)} themes, {len(layouts)} layouts, no clashes")
    return 0


if __name__ == "__main__":
    sys.exit(main())