"""Re-derive the recorded golden of the SWF shape / action / sprite / sound port (common/flash
FDTShapes, FAction, FDTSprite, FDTSounds -> *.rs) from the ORIGINAL C++.

The ten .cpp files (the six SWF stream writers ported earlier plus the four ported here, and the
headers as they were before either port) are rebuilt from git with the host g++ and driven by
``swfshapes_ref.cpp`` (colour transforms, shape records, DefineShape/2/3 and DefineMorphShape
from random object graphs, sprites, actions, conditions, clip actions, sounds). The output is
written to ``flare/sources/common/flash/swfshapes_cases.txt``, which the Rust tests of the four
modules replay.

Everything is plain integer/byte output and platform independent, with two documented
exceptions that the harness masks or pins (see ``swfshapes_ref.cpp``): the eight operand bytes
of ``FActionPush`` type 6 are derived from addresses in the C++ and printed as zeros, and the
members of ``FSndEnv`` (never set by the original constructor) come from pre-filled memory, which
needs ``-fno-lifetime-dse``.

Usage:
    python tests/native/gen_swfshapes_cases.py [--check] [--verify-chain]

``--check``         do not write; exit 1 when the committed golden differs from a rebuild.
``--verify-chain``  also build the harness against the NEW inline headers + the Rust static
                    library (no .cpp from the ported classes at all) and require byte-identical
                    output (proves the C++ shells marshal like the old members). Needs ``cargo``;
                    works offline.
Needs g++, zlib headers and git history. Not part of the test run; the golden is committed.
"""
from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import msvc_reference as ms  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
SRC = os.path.join(ms.ROOT, "flare", "sources")
FLASH = "flare/sources/common/flash"
GOLDEN = os.path.join(SRC, "common", "flash", "swfshapes_cases.txt")
# -fno-lifetime-dse: FSndEnv's constructor never sets its members; the harness pins the bytes of
# the pre-filled memory it is constructed over.
CXXFLAGS = ["-O2", "-std=c++17", "-ffp-contract=off", "-fno-lifetime-dse", "-DLINUX", "-DTNZ_LITTLE_ENDIAN", "-DNDEBUG", "-w"]
ORIGINAL = ["FSWFStream", "FPrimitive", "FObj", "FSound", "FCT", "FDTBitmaps", "FDTShapes", "FAction", "FDTSprite", "FDTSounds"]


def cxx() -> str:
    return os.environ.get("CXX") or "g++"


def extract_original(workdir: str) -> str:
    # the newest commit that still had FSWFStream.cpp also has the other nine (the SWF stream
    # writers were retired in one commit, the four classes ported here afterwards)
    rev = ms.pre_deletion_revision(f"{FLASH}/FSWFStream.cpp")
    for n in ORIGINAL:
        if subprocess.run(["git", "cat-file", "-e", f"{rev}:{FLASH}/{n}.cpp"], cwd=ms.ROOT).returncode != 0:
            sys.exit(f"{n}.cpp is missing at {rev[:12]}")
    names = ms.git("ls-tree", "--name-only", rev, FLASH + "/").splitlines()
    wanted = [p for p in names if p.endswith(".h") or (os.path.splitext(os.path.basename(p))[0] in ORIGINAL and p.endswith(".cpp"))]
    ms.extract(rev, wanted, workdir)
    return rev


def build(workdir: str, exe: str, sources: list[str], includes: list[str], extra: list[str], shim: bool = False) -> str:
    out = os.path.join(workdir, exe)
    argv = [cxx(), *CXXFLAGS]
    if shim:  # original only: 32-bit uLongf semantics, see swfwriter_zlib_shim.h
        argv += ["-include", os.path.join(HERE, "swfwriter_zlib_shim.h")]
    for i in includes:
        argv += ["-I", i]
    argv += ["-o", out, os.path.join(HERE, "swfshapes_ref.cpp"), *sources, *extra, "-lz"]
    subprocess.run(argv, check=True)
    return out


def run(exe: str) -> str:
    return subprocess.run([exe], check=True, capture_output=True, text=True).stdout


def rust_staticlib() -> str:
    subprocess.run(["cargo", "build", "-p", "flare_formats", "--offline"], cwd=SRC, check=True)
    meta = json.loads(subprocess.check_output(["cargo", "metadata", "--offline", "--format-version", "1", "--no-deps"], cwd=SRC))
    return os.path.join(meta["target_directory"], "debug", "libflare_formats.a")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true")
    ap.add_argument("--verify-chain", action="store_true")
    args = ap.parse_args()
    rc = 0
    with tempfile.TemporaryDirectory(prefix="swfshapes-") as t:
        rev = extract_original(t)
        orig = build(t, "ref_orig", [os.path.join(t, n + ".cpp") for n in ORIGINAL], [t, os.path.join(SRC, "include")], [], shim=True)
        out = run(orig)
        rc = ms.check_or_write(GOLDEN, out, rev, args.check, count="lines")
        if args.verify_chain:
            lib = rust_staticlib()
            flash = os.path.join(SRC, "common", "flash")
            new = build(t, "ref_new", [], [flash, os.path.join(SRC, "include")], [lib, "-lpthread", "-ldl", "-lm"])
            if run(new) != out:
                print("CHAIN MISMATCH: new headers + Rust differ from the original", file=sys.stderr)
                rc = 1
            else:
                print("chain ok: new headers + Rust print the same bytes as the original")
    return rc


if __name__ == "__main__":
    raise SystemExit(main())
