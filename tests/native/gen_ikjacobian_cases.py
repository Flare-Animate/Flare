"""Re-derive the recorded golden in flarelib/ikjacobian.rs (the port of ikjacobian.cpp and the
angle arithmetic of ikengine.cpp).

The original ``ikjacobian.cpp`` (+ ``ikjacobian.h``, ``ikengine.*``, ``ikskeleton.*``, and
``iknode.*`` from before its own retirement) is rebuilt from git history with the host g++,
driven by ``ikjacobian_ref.cpp`` (matrix / SVD / Jacobian / engine-angle cases from a fixed
LCG, inputs and outputs printed as IEEE bit patterns), and the output is spliced between the
``// BEGIN GOLDEN`` / ``// END GOLDEN`` markers of the Rust test module.

Platform independence: everything recorded is ``+ - * /`` and ``sqrt`` (correctly rounded
by IEEE 754), compiled with ``-ffp-contract=off``; Rust does not fuse either. The only libm
dependent lines are ``ang`` (``atan2``); the Rust test allows 2 ULP there. NaN results are
compared as "NaN in both" because the sign/payload of a generated NaN is hardware defined.

Usage:
    python tests/native/gen_ikjacobian_cases.py [--check] [--verify-chain]

``--check``         do not write; exit 1 when the committed golden differs from a rebuild.
``--verify-chain``  also build the harness against the NEW inline headers + the Rust static
                    library and require byte-identical output (proves the C++ forwarders
                    marshal like the old members; also compares a trig-based engine drag,
                    which is not part of the golden). Needs ``cargo`` and works offline.
Needs g++ and git history. Not part of the test run; the golden is committed.
"""
from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import msvc_reference as ms  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
SRC = os.path.join(ms.ROOT, "flare", "sources")
FL = "flare/sources/flarelib"
INC = "flare/sources/include/flare"
TARGET_RS = os.path.join(SRC, "flarelib", "ikjacobian.rs")
CXXFLAGS = ["-O2", "-std=c++17", "-ffp-contract=off", "-DLINUX", "-DNDEBUG", "-w"]


def cxx() -> str:
    return os.environ.get("CXX") or "g++"


def extract_original(workdir: str) -> list[str]:
    rev = ms.pre_deletion_revision(f"{FL}/ikjacobian.cpp")
    rev_node = ms.pre_deletion_revision(f"{FL}/iknode.cpp")
    os.makedirs(os.path.join(workdir, "flare"))
    flat = lambda r, rels, d: ms.extract(r, rels, d)  # noqa: E731
    flat(rev, [f"{INC}/ikjacobian.h", f"{INC}/ikengine.h", f"{INC}/ikskeleton.h"], os.path.join(workdir, "flare"))
    flat(rev_node, [f"{INC}/iknode.h"], os.path.join(workdir, "flare"))
    flat(rev, [f"{FL}/ikjacobian.cpp", f"{FL}/ikengine.cpp", f"{FL}/ikskeleton.cpp"], workdir)
    flat(rev_node, [f"{FL}/iknode.cpp"], workdir)
    return [rev, rev_node]


def build_original(workdir: str) -> str:
    exe = os.path.join(workdir, "ref_orig")
    subprocess.run(
        [cxx(), *CXXFLAGS, "-I", workdir, "-I", os.path.join(SRC, "include"), "-o", exe,
         os.path.join(HERE, "ikjacobian_ref.cpp"), *(os.path.join(workdir, f) for f in
         ("ikjacobian.cpp", "ikengine.cpp", "ikskeleton.cpp", "iknode.cpp"))],
        check=True,
    )
    return exe


def rust_staticlib() -> str:
    subprocess.run(["cargo", "build", "-p", "flare_formats", "--offline"], cwd=SRC, check=True)
    meta = json.loads(subprocess.check_output(["cargo", "metadata", "--offline", "--format-version", "1", "--no-deps"], cwd=SRC))
    return os.path.join(meta["target_directory"], "debug", "libflare_formats.a")


def build_new(workdir: str) -> str:
    lib = rust_staticlib()
    exe = os.path.join(workdir, "ref_new")
    subprocess.run(
        [cxx(), *CXXFLAGS, "-I", os.path.join(SRC, "include"), "-o", exe,
         os.path.join(HERE, "ikjacobian_ref.cpp"), os.path.join(SRC, "flarelib", "ikengine.cpp"),
         os.path.join(SRC, "flarelib", "ikskeleton.cpp"), lib, "-lpthread", "-ldl", "-lm"],
        check=True,
    )
    return exe


def run(exe: str, *args: str) -> str:
    return subprocess.run([exe, *args], check=True, capture_output=True, text=True).stdout


def splice(path: str, body: str, check: bool) -> bool:
    s = open(path).read()
    pat = re.compile(r"(// BEGIN GOLDEN\n).*?(    // END GOLDEN)", re.S)
    new = pat.sub(lambda m: m.group(1) + body + m.group(2), s)
    if check:
        return new == s
    open(path, "w").write(new)
    return True


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true")
    ap.add_argument("--verify-chain", action="store_true")
    args = ap.parse_args()
    rc = 0
    with tempfile.TemporaryDirectory(prefix="ikjac-") as t:
        revs = extract_original(t)
        orig = build_original(t)
        out = run(orig)
        lines = out.splitlines()
        body = '    const GOLDEN: &str = r#"\n' + "\n".join(lines) + '\n"#;\n'
        if not splice(TARGET_RS, body, args.check):
            print("DRIFT: golden in ikjacobian.rs does not match the rebuilt original", file=sys.stderr)
            rc = 1
        else:
            print(f"golden ok: {len(lines)} cases from {revs[0][:12]} / {revs[1][:12]}")
        if args.verify_chain:
            new = build_new(t)
            if run(new) != out:
                print("CHAIN MISMATCH: new headers + Rust differ from the original on the golden cases", file=sys.stderr)
                rc = 1
            if run(new, "drag") != run(orig, "drag"):
                print("CHAIN MISMATCH: engine drag differs", file=sys.stderr)
                rc = 1
            if rc == 0:
                print("chain ok: golden cases and engine drag identical to the original")
    return rc


if __name__ == "__main__":
    raise SystemExit(main())
