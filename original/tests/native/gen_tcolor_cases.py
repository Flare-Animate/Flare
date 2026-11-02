"""Re-derive the recorded golden of the tcolor port (common/tcolor tpixel / tcolorfunctions /
tpixelutils -> *.rs) from the ORIGINAL C++.

The pre-port headers (flare/sources/include) and the three .cpp are extracted from git (the
newest revision that still has tpixelutils.cpp), built with g++ -ffp-contract=off together with
``tcolor_ref.cpp`` and run. Output: ``flare/sources/common/tcolor/tcolor_cases.txt``, replayed
by the Rust tests. Integer and arithmetic lines are platform independent; the ``lin*`` lines
depend on libm ``pow`` (Rust tests hard-assert them only on linux-gnu, otherwise a few ULP).

    python tests/native/gen_tcolor_cases.py [--check] [--verify-chain]

``--verify-chain`` also builds the harness against the NEW headers + libflare_formats.a and
requires identical output.
"""
import argparse, os, subprocess, sys, tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import msvc_reference as ms  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
SRC = os.path.join(ms.ROOT, "flare", "sources")
GOLDEN = os.path.join(SRC, "common", "tcolor", "tcolor_cases.txt")
CXXFLAGS = ["-O2", "-std=c++17", "-ffp-contract=off", "-DLINUX", "-DTNZ_LITTLE_ENDIAN", "-DNDEBUG", "-w"]
CPPS = ["tpixel", "tcolorfunctions", "tpixelutils"]


def build(out, sources, incs, extra=()):
    cxx = os.environ.get("CXX") or "g++"
    argv = [cxx, *CXXFLAGS]
    for i in incs:
        argv += ["-I", i]
    subprocess.run([*argv, "-o", out, os.path.join(HERE, "tcolor_ref.cpp"), *sources, *extra], check=True)
    return subprocess.run([out], check=True, capture_output=True, text=True).stdout


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true")
    ap.add_argument("--verify-chain", action="store_true")
    a = ap.parse_args()
    rev = ms.pre_deletion_revision("flare/sources/common/tcolor/tpixelutils.cpp")
    with tempfile.TemporaryDirectory(prefix="tcolor-") as t:
        inc = os.path.join(t, "include")
        os.makedirs(inc)
        tar = subprocess.Popen(["git", "archive", rev, "flare/sources/include"], cwd=ms.ROOT, stdout=subprocess.PIPE)
        subprocess.run(["tar", "-x", "-C", t, "--strip-components=2"], stdin=tar.stdout, check=True)
        tar.wait()
        srcs = []
        for n in CPPS:
            p = os.path.join(t, n + ".cpp")
            open(p, "wb").write(subprocess.check_output(["git", "show", f"{rev}:flare/sources/common/tcolor/{n}.cpp"], cwd=ms.ROOT))
            srcs.append(p)
        out = build(os.path.join(t, "ref_orig"), srcs, [inc])
        rc = ms.check_or_write(GOLDEN, out, rev, a.check, count="lines")
        if a.verify_chain:
            lib = ms.rust_archive()
            new = build(os.path.join(t, "ref_new"), [os.path.join(SRC, "common/tcolor/tpixel.cpp")], [os.path.join(SRC, "include")], [lib, "-lpthread", "-ldl", "-lm"])
            if new != out:
                print("CHAIN MISMATCH", file=sys.stderr)
                rc = 1
            else:
                print("chain ok: new headers + Rust print the same lines as the original")
    return rc


if __name__ == "__main__":
    raise SystemExit(main())
