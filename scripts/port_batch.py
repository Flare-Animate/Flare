#!/usr/bin/env python3
"""Port-batch planner and guard for the C++ -> Rust rewrite.

The rewrite replaces each C++ file with an identically named `.rs` beside it
(`flare/sources/<dir>/<name>.cpp` -> `flare/sources/<dir>/<name>.rs`); the C++
is deleted once the Rust is authoritative (see docs/RUST_REWRITE.md and
docs/PORTING.md).

This script automates the mechanical parts of that workflow:

* ``--batch N``      list the N smallest not-yet-ported, Qt/TRaster-free leaf
                     candidates to start a new port batch from;
* ``--check FILES``  for each path say ``PORT`` (no sibling ``.rs``), ``SKIP``
                     (already ported - never rewrite it), or ``MISSING``;
* ``--register``     print the ``#[path]`` lines to add to flare_formats.rs and
                     report whether the module is already registered (an
                     unregistered module compiles but ships as dead code);
* ``--draft FILES``  if ``crust`` or ``cpp2rust`` is on PATH, run it as a
                     *drafting* aid (docs/CPP2RUST_TRIAL.md: never trust the
                     output); always refuse files that already have a ``.rs``.

Usage::

    python scripts/port_batch.py --batch 5
    python scripts/port_batch.py --check stdfx/naru_maxflow.cpp
    python scripts/port_batch.py --register stdfx/naru_maxflow.cpp
    python scripts/port_batch.py --draft stdfx/naru_maxflow.cpp

Only bare stdlib is used so it runs anywhere the rest of the tooling does.
"""

import argparse
import os
import shutil
import subprocess
import sys

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SRC = os.path.join(REPO, "flare", "sources")
FORMATS_RS = os.path.join(SRC, "flare_formats.rs")

CPP_EXTS = (".cpp", ".cc", ".cxx")
# Directories that are never part of the port (fixtures, vendored code, build).
SKIP_DIRS = ("thirdparty", "tests", "test", "fixtures", "vcpkg", "build", "target")
# Heuristic risk markers from docs/PORT_CANDIDATES_BATCH4.md.
RISKY_INCLUDES = ("TRaster", "TRasterFx", "QWidget", "QObject", "Qt", "QString")


def is_skipped_dir(path):
    parts = path.replace("\\", "/").split("/")
    return any(p in SKIP_DIRS or p.lower().startswith("build") for p in parts)


def rel(path):
    return os.path.relpath(path, SRC).replace("\\", "/")


def has_sibling_rs(cpp_path):
    base = os.path.splitext(cpp_path)[0]
    return os.path.exists(base + ".rs")


def iter_cpp():
    for root, dirs, files in os.walk(SRC):
        if is_skipped_dir(rel(root) if root != SRC else ""):
            dirs[:] = []
            continue
        for f in sorted(files):
            if f.endswith(CPP_EXTS):
                full = os.path.join(root, f)
                if not is_skipped_dir(rel(full)):
                    yield full


def line_count(path):
    try:
        with open(path, "r", encoding="utf-8", errors="ignore") as fh:
            return sum(1 for _ in fh)
    except OSError:
        return 0


def looks_risky(path):
    try:
        with open(path, "r", encoding="utf-8", errors="ignore") as fh:
            head = fh.read(4000)
    except OSError:
        return False
    return any(m in head for m in RISKY_INCLUDES)


def candidates():
    """(path, lines, risky) for every C++ file with no sibling .rs."""
    out = []
    for full in iter_cpp():
        if has_sibling_rs(full):
            continue
        out.append((full, line_count(full), looks_risky(full)))
    return out


def cmd_batch(n):
    rows = [(l, p, r) for (p, l, r) in candidates() if not r]
    rows.sort()
    if not rows:
        print("no unported C++ candidates found")
        return 0
    print("next %d port candidates (smallest, Qt/TRaster-free first):" % n)
    print("%6s  %-8s %s" % ("lines", "risk", "path"))
    for lines, path, risky in rows[:n]:
        print("%6d  %-8s %s" % (lines, "low" if not risky else "med", rel(path)))
    total = len(candidates())
    print("\n%d unported C++ files remain (of %d total)" % (total, sum(1 for _ in iter_cpp())))
    return 0


def cmd_check(files):
    rc = 0
    for f in files:
        full = f if os.path.isabs(f) else os.path.join(SRC, f)
        if not os.path.exists(full):
            print("MISSING  %s" % f)
            rc = 1
        elif has_sibling_rs(full):
            print("SKIP     %s  (already ported: sibling .rs exists)" % f)
        else:
            print("PORT     %s  (%d lines)" % (f, line_count(full)))
    return rc


def cmd_register(files):
    with open(FORMATS_RS, "r", encoding="utf-8") as fh:
        formats = fh.read()
    for f in files:
        base = os.path.splitext(f.replace("\\", "/"))[0]
        modname = os.path.basename(base)
        if "/%s.rs" % modname in formats or '"%s.rs"' % base in formats:
            print("# already registered: %s" % modname)
            continue
        print("add to flare_formats.rs:\n")
        print('#[path = "%s.rs"]' % base)
        print("pub mod %s;" % modname)
    return 0


def cmd_draft(files):
    tool = None
    for name in ("cpp2rust", "crust"):
        found = shutil.which(name)
        if found:
            tool = found
            break
    if not tool:
        print("neither cpp2rust nor crust is on PATH; install one to draft "
              "(see docs/CPP2RUST_TRIAL.md). Output is a drafting aid only.",
              file=sys.stderr)
        return 2
    rc = 0
    for f in files:
        full = f if os.path.isabs(f) else os.path.join(SRC, f)
        if has_sibling_rs(full):
            print("refusing to draft %s: sibling .rs already exists" % f)
            rc = 1
            continue
        out = os.path.splitext(full)[0] + ".draft.rs"
        print("%s -> %s" % (os.path.basename(tool), out))
        rc |= subprocess.call([tool, full] if "crust" in os.path.basename(tool) else [tool, "--file=%s" % full, "-o=%s" % out])
    return rc


def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    g = p.add_mutually_exclusive_group(required=True)
    g.add_argument("--batch", type=int, metavar="N", help="list the N smallest candidates")
    g.add_argument("--check", nargs="+", metavar="FILE", help="classify files as PORT/SKIP/MISSING")
    g.add_argument("--register", nargs="+", metavar="FILE", help="print #[path] registration lines")
    g.add_argument("--draft", nargs="+", metavar="FILE", help="run crust/cpp2rust on unported files")
    args = p.parse_args(argv)
    if args.batch is not None:
        return cmd_batch(args.batch)
    if args.check:
        return cmd_check(args.check)
    if args.register:
        return cmd_register(args.register)
    if args.draft:
        return cmd_draft(args.draft)
    return 2


if __name__ == "__main__":
    sys.exit(main())
