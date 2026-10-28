"""Fail if the Rust workspace resolves a UI-critical crate to more than one version.

Why this exists: the tree arrived with three egui front ends and, because each crate
pinned its own egui, `cargo` happily compiled *two* incompatible egui generations side by
side -- `ac-app` on 0.29 and `minnamate-editor`/`flare-studio` on 0.36. That is invisible in
a normal build (it succeeds) and only shows up as a widget that cannot be shared between
panels, which is exactly the wall UNIFICATION PLAN steps 8/14 hit. `cargo tree -d` finds it,
but only if someone remembers to run it; this makes the invariant a check.

It reads the committed `flare/sources/Cargo.lock` (no cargo, no network, so it runs in any
CI job) and reports every watched crate with more than one resolved version. Exit 1 on a
duplicate.

Usage:
    python scripts/dep_skew_check.py            # fail on any watched duplicate
    python scripts/dep_skew_check.py --all      # also list *all* duplicated crates, informatively
"""
from __future__ import annotations

import argparse
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
LOCK = os.path.join(ROOT, "flare", "sources", "Cargo.lock")

# The crates that must be one generation across the whole workspace, because they are the
# UI toolkit every panel is written against. Adding one here is the point of the check;
# `--all` covers everything else informationally.
WATCH = (
    "egui",
    "eframe",
    "epaint",
    "epaint_default_fonts",
    "egui-wgpu",
    "egui-winit",
    "egui_extras",
    "egui_kittest",
)

_PKG = re.compile(r"^\[\[package\]\]$")
_NAME = re.compile(r'^name = "([^"]+)"$')
_VERSION = re.compile(r'^version = "([^"]+)"$')


def resolved_versions(lock_text: str) -> dict[str, list[str]]:
    """name -> sorted versions, one entry per `[[package]]` block, honouring `version =`."""
    out: dict[str, set[str]] = {}
    name = None
    for line in lock_text.splitlines():
        line = line.strip()
        if _PKG.match(line):
            name = None
            continue
        m = _NAME.match(line)
        if m:
            name = m.group(1)
            continue
        m = _VERSION.match(line)
        if m and name:
            out.setdefault(name, set()).add(m.group(1))
    return {k: sorted(v) for k, v in out.items()}


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--all", action="store_true", help="also list every duplicated crate")
    args = ap.parse_args()

    if not os.path.exists(LOCK):
        print(f"dep_skew_check: no {LOCK}", file=sys.stderr)
        return 1

    with open(LOCK, "r", encoding="utf-8") as fh:
        versions = resolved_versions(fh.read())

    dup_watched = {n: v for n, v in versions.items() if n in WATCH and len(v) > 1}

    if args.all:
        dup_all = {n: v for n, v in versions.items() if len(v) > 1}
        print(f"dep_skew_check: {len(dup_all)} duplicated crate(s) in the lock")
        for n in sorted(dup_all):
            mark = "  WATCHED" if n in WATCH else ""
            print(f"  {n}: {', '.join(dup_all[n])}{mark}")

    if dup_watched:
        print("dep_skew_check: UI toolkit resolved to multiple versions", file=sys.stderr)
        for n in sorted(dup_watched):
            print(f"  {n}: {', '.join(dup_watched[n])}", file=sys.stderr)
        return 1

    egui = versions.get("egui", ["?"])
    print(
        "dep_skew_check: 0 watched duplicates "
        f"({len(WATCH)} crates); egui = {', '.join(egui)}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
