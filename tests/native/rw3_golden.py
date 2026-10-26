"""Shared g++ differential-golden driver for the rw3 ports.

``record(name, srcs, ...)`` extracts the *original* files from git (the newest tree that
still has them: HEAD while the .cpp exists, the parent of the deleting commit afterwards),
builds ``tests/native/<name>_ref.cpp`` against them with ``g++ -ffp-contract=off`` and writes
the output to the golden next to the Rust file. ``--check`` only compares; ``--verify-chain``
builds the same harness with ``-DCHAIN`` against the LIVE inline headers + the Rust library
and requires the identical output (kept lines selectable).

Layout of the temp tree: sources are flattened by basename into ``<tmp>``; ``stub`` files
(name -> text) are written into ``<tmp>/stub`` which is first on the include path (use it for
Qt-heavy headers the original merely includes). ``hdr_as`` maps a source path to the include
path it must be visible under (e.g. ``flare/namebuilder.h``).
"""
from __future__ import annotations

import argparse
import os
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import msvc_reference as ms  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
SRC_ROOT = os.path.join(ms.ROOT, "flare", "sources")


def run(name, golden_rel, srcs, hdr_as=None, stubs=None, includes=None, extra=None,
        keep=None, compile=None, chain_compile=None, orig_marker=None):
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true")
    ap.add_argument("--verify-chain", action="store_true")
    args = ap.parse_args()
    harness = os.path.join(HERE, f"{name}_ref.cpp")
    golden = os.path.join(SRC_ROOT, golden_rel)
    if orig_marker:
        # partial port: the .cpp still exists, so HEAD already holds the trimmed file. Take the
        # newest revision whose copy still contains `orig_marker` (text only the original has).
        import subprocess
        rev = None
        for r in ms.git("log", "--format=%H", "--", srcs[0]).splitlines():
            data = subprocess.run(["git", "show", f"{r}:{srcs[0]}"], cwd=ms.ROOT, capture_output=True).stdout
            if orig_marker.encode() in data:
                rev = r
                break
        if rev is None:
            sys.exit("no revision with the original " + srcs[0])
    else:
        rev = ms.pre_deletion_tree(srcs[0])
    incs = [os.path.join(SRC_ROOT, "include")] + [os.path.join(SRC_ROOT, i) for i in (includes or [])]
    with tempfile.TemporaryDirectory(prefix=name + "-") as wd:
        ms.extract(rev, srcs, wd)
        stubdir = os.path.join(wd, "stub")
        os.makedirs(stubdir)
        for rel, text in (stubs or {}).items():
            p = os.path.join(stubdir, rel)
            os.makedirs(os.path.dirname(p), exist_ok=True)
            open(p, "w").write(text)
        for src, inc in (hdr_as or {}).items():
            p = os.path.join(stubdir, inc)
            os.makedirs(os.path.dirname(p), exist_ok=True)
            os.replace(os.path.join(wd, os.path.basename(src)), p)
        fresh = ms.build_and_run_host(wd, list(compile or []), harness, include_dirs=[stubdir, wd] + incs,
                                      extra_args=["-DNDEBUG", "-DLINUX"] + (extra or []))
    rc = ms.check_or_write(golden, fresh, rev, args.check, count="lines")
    if rc == 0 and args.verify_chain:
        # the stubs (not the original headers) also stand in for heavy includes in the chain build
        with tempfile.TemporaryDirectory(prefix=name + "-chain-") as cd:
            for rel, text in (stubs or {}).items():
                p = os.path.join(cd, rel)
                os.makedirs(os.path.dirname(p), exist_ok=True)
                open(p, "w").write(text)
            rc = _verify_chain(harness, golden, [cd] + incs, keep, ["-DNDEBUG", "-DLINUX"] + (extra or []),
                               [os.path.join(ms.ROOT, c) for c in (chain_compile or [])])
    return rc


def _verify_chain(harness, golden, include_dirs, keep, extra_args, sources):
    """`ms.verify_chain` plus live C++ sources linked next to the Rust archive (partial ports)."""
    archive = ms.rust_archive()
    with open(golden, "r", newline="") as fh:
        want = ms.normalize(fh.read()).splitlines()
    if keep:
        want = [l for l in want if keep(l)]
    with tempfile.TemporaryDirectory(prefix="chain-") as wd:
        got = ms.normalize(
            ms.build_and_run_host(wd, sources, harness, include_dirs=include_dirs,
                                  link_archives=[archive, "-lpthread", "-ldl", "-lm"],
                                  extra_args=["-DCHAIN", *extra_args])
        ).splitlines()
    if keep:
        got = [l for l in got if keep(l)]
    if got != want:
        n = next((i for i, (a, b) in enumerate(zip(got, want)) if a != b), min(len(got), len(want)))
        print(f"CHAIN MISMATCH at line {n}: got {got[n:n+1]} want {want[n:n+1]}", file=sys.stderr)
        return 1
    print(f"chain ok: new headers + Rust reproduce {len(want)} golden lines")
    return 0
