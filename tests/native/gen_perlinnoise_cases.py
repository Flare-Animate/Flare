"""Regenerate/check flare/sources/stdfx/perlinnoise_cases.txt from the original C++.

The noise functions are sliced verbatim out of the original perlinnoise.cpp (from
``PerlinNoise::LinearNoise`` up to the constructor) and compiled against a bare class layout.
"""
import os
import subprocess
import sys
import tempfile

import rw3_golden as g
import msvc_reference as ms

SRC = "flare/sources/stdfx/perlinnoise.cpp"


def main():
    # Slice from the original (the tree before the port trimmed the file).
    rev = ms.pre_deletion_tree(SRC)
    orig = subprocess.check_output(["git", "show", f"{rev}:{SRC}"], cwd=ms.ROOT)
    if b"double PerlinNoise::LinearNoise" not in orig:
        # HEAD already holds the trimmed file: walk back to the last full version
        for r in ms.git("log", "--format=%H", "--", SRC).splitlines():
            data = subprocess.check_output(["git", "show", f"{r}:{SRC}"], cwd=ms.ROOT)
            if b"double PerlinNoise::LinearNoise" in data:
                orig = data
                break
    a = orig.index(b"double PerlinNoise::LinearNoise")
    b = orig.index(b"PerlinNoise::PerlinNoise()")
    inc = orig[a:b]
    d = tempfile.mkdtemp(prefix="pnslice-")
    open(os.path.join(d, "pn_funcs.inc"), "wb").write(inc)
    return g.run("perlinnoise", "stdfx/perlinnoise_cases.txt", [SRC], includes=[],
                 extra=["-I" + d])


raise SystemExit(main())
