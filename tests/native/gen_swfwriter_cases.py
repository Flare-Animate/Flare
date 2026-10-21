"""Re-derive the recorded golden of the SWF writer port (common/flash FSWFStream, FPrimitive,
FObj, FSound, FCT, FDTBitmaps -> *.rs) from the ORIGINAL C++.

The six deleted .cpp files (and the headers as they were before the port) are rebuilt from git
with the host g++ together with the three unported ones the harness needs (FDTShapes.cpp for
FCXForm, FAction.cpp for FClipAction, FDTSounds.cpp for FSoundInfo), driven by
``swfwriter_ref.cpp`` (random byte-stream op sequences, primitives, collections, ADPCM, tags,
bitmaps). The output is written to ``flare/sources/common/flash/swfwriter_cases.txt``, which the
Rust tests of the six modules replay.

zlib streams are normalised by the harness (inflated before printing): the Rust side deflates
with miniz, so only the inflated bytes (and the "compressed or not" decision) are comparable.
Everything else is plain integer/byte output and is platform independent.

Usage:
    python tests/native/gen_swfwriter_cases.py [--check] [--verify-chain]

``--check``         do not write; exit 1 when the committed golden differs from a rebuild.
``--verify-chain``  also build the harness against the NEW inline headers + the Rust static
                    library and require byte-identical output (proves the C++ shells marshal
                    like the old members). Needs ``cargo``; works offline.
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
GOLDEN = os.path.join(SRC, "common", "flash", "swfwriter_cases.txt")
CXXFLAGS = ["-O2", "-std=c++17", "-ffp-contract=off", "-DLINUX", "-DTNZ_LITTLE_ENDIAN", "-DNDEBUG", "-w"]
PORTED = ["FSWFStream", "FPrimitive", "FObj", "FSound", "FCT", "FDTBitmaps"]
KEPT = ["FDTShapes", "FAction", "FDTSounds"]


def cxx() -> str:
    return os.environ.get("CXX") or "g++"


def extract_original(workdir: str) -> str:
    rev = ms.pre_deletion_revision(f"{FLASH}/FSWFStream.cpp")
    for n in PORTED:
        if subprocess.run(["git", "cat-file", "-e", f"{rev}:{FLASH}/{n}.cpp"], cwd=ms.ROOT).returncode != 0:
            sys.exit(f"{n}.cpp is missing at {rev[:12]}")
    names = ms.git("ls-tree", "--name-only", rev, FLASH + "/").splitlines()
    wanted = [p for p in names if p.endswith(".h") or os.path.splitext(os.path.basename(p))[0] in PORTED + KEPT and p.endswith(".cpp")]
    ms.extract(rev, wanted, workdir)
    # Patch FCT.h: the archived version includes <string.h> (C header) but uses
    # std::string, which requires <string>. Fixed in the current tree; patch here
    # so the golden reference compiles.
    fct_path = os.path.join(workdir, "FCT.h")
    if os.path.exists(fct_path):
        with open(fct_path) as f:
            content = f.read()
        content = content.replace("#include <string.h>", "#include <string>")
        with open(fct_path, "w") as f:
            f.write(content)
    return rev


def build(workdir: str, exe: str, sources: list[str], includes: list[str], extra: list[str], shim: bool = False) -> str:
    out = os.path.join(workdir, exe)
    argv = [cxx(), *CXXFLAGS]
    if shim:  # original only: 32-bit uLongf semantics, see swfwriter_zlib_shim.h
        argv += ["-include", os.path.join(HERE, "swfwriter_zlib_shim.h")]
    for i in includes:
        argv += ["-I", i]
    argv += ["-o", out, os.path.join(HERE, "swfwriter_ref.cpp"), *sources, *extra, "-lz"]
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
    with tempfile.TemporaryDirectory(prefix="swfw-") as t:
        rev = extract_original(t)
        orig = build(t, "ref_orig", [os.path.join(t, n + ".cpp") for n in PORTED + KEPT], [t, os.path.join(SRC, "include")], [], shim=True)
        out = run(orig)
        rc = ms.check_or_write(GOLDEN, out, rev, args.check, count="lines")
        if args.verify_chain:
            lib = rust_staticlib()
            flash = os.path.join(SRC, "common", "flash")
            new = build(t, "ref_new", [os.path.join(flash, n + ".cpp") for n in KEPT], [flash, os.path.join(SRC, "include")], [lib, "-lpthread", "-ldl", "-lm"])
            if run(new) != out:
                print("CHAIN MISMATCH: new headers + Rust differ from the original", file=sys.stderr)
                rc = 1
            else:
                print("chain ok: new headers + Rust print the same bytes as the original")
    return rc


if __name__ == "__main__":
    raise SystemExit(main())
