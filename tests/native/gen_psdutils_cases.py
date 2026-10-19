"""Re-derive the recorded golden in psdutils.rs from the ORIGINAL psdutils.cpp.

psdutils.cpp is deleted, so it is rebuilt from git (REF, the commit before the
retirement) with g++ against the system zlib (-lz) and run on inputs made here
(an LCG, plus a tiny deflate encoder using only stored and fixed-Huffman
blocks, so the streams do not depend on the zlib version that made them). The
output lines are spliced between `// BEGIN GOLDEN` / `// END GOLDEN` in the
Rust test module.

Only platform-independent cases are recorded: no libc locale/atoi, all lengths
small enough for 32-bit `long`, and the over-read of `unpackrow` (UB in C++) is
fed zero padding, which the Rust port defines to read as zero.

Usage: python tests/native/gen_psdutils_cases.py [--check]
Needs g++ and zlib headers. Not part of the test run; the golden is committed.
"""
import os, re, struct, subprocess, sys, tempfile, zlib

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", ".."))
SRC = os.path.join(REPO, "flare", "sources")
REF = os.environ.get("PORT_C_REF", "13d1d28")


class Lcg:
    def __init__(self, s): self.s = s
    def __call__(self, n=1 << 31):
        self.s = (self.s * 6364136223846793005 + 1442695040888963407) & (2**64 - 1)
        return (self.s >> 33) % n


class Bits:
    def __init__(self): self.b = bytearray(); self.n = 0
    def put(self, v, k):  # LSB first
        for i in range(k):
            if self.n % 8 == 0: self.b.append(0)
            self.b[-1] |= ((v >> i) & 1) << (self.n % 8); self.n += 1
    def huff(self, code, k):  # MSB first
        for i in range(k - 1, -1, -1): self.put((code >> i) & 1, 1)


LBASE = [3,4,5,6,7,8,9,10,11,13,15,17,19,23,27,31,35,43,51,59,67,83,99,115,131,163,195,227,258]
LEXT = [0]*8 + [1]*4 + [2]*4 + [3]*4 + [4]*4 + [5]*4 + [0]
DBASE = [1,2,3,4,5,7,9,13,17,25,33,49,65,97,129,193,257,385,513,769,1025,1537,2049,3073,4097,6145,8193,12289,16385,24577]
DEXT = [0,0,0,0,1,1,2,2,3,3,4,4,5,5,6,6,7,7,8,8,9,9,10,10,11,11,12,12,13,13]


def sym(bits, s):
    if s < 144: bits.huff(0x30 + s, 8)
    elif s < 256: bits.huff(0x190 + s - 144, 9)
    elif s < 280: bits.huff(s - 256, 7)
    else: bits.huff(0xC0 + s - 280, 8)


def fixed_block(data, final=True, matches=True):
    b = Bits(); b.put(1 if final else 0, 1); b.put(1, 2)
    i = 0
    while i < len(data):
        best = (0, 0)
        if matches:
            for d in range(1, min(i, 40) + 1):
                l = 0
                while l < 258 and i + l < len(data) and data[i + l - d] == data[i + l]: l += 1
                if l > best[0]: best = (l, d)
        if best[0] >= 3:
            l, d = best
            li = max(k for k in range(29) if LBASE[k] <= l)
            sym(b, 257 + li); b.put(l - LBASE[li], LEXT[li])
            di = max(k for k in range(30) if DBASE[k] <= d)
            b.huff(di, 5); b.put(d - DBASE[di], DEXT[di])
            i += l
        else:
            sym(b, data[i]); i += 1
    sym(b, 256)
    return bytes(b.b)


def stored(data, final=True):
    return bytes([1 if final else 0]) + struct.pack("<HH", len(data), len(data) ^ 0xFFFF) + data


def adler(d): return struct.pack(">I", zlib.adler32(bytes(d)) & 0xFFFFFFFF)


def zstream(data, kind):
    data = bytes(data)
    if kind == 0: body = stored(data)
    elif kind == 1: body = fixed_block(data)
    elif kind == 2: body = fixed_block(data, matches=False)
    else:
        h = len(data) // 2
        body = stored(data[:h], False) + fixed_block(data[h:])
    return b"\x78\x9c" + body + adler(data)


def hx(b): return bytes(b).hex() or "-"


def make_cases():
    r = Lcg(12345)
    cases = []
    # --- unpackrow: PackBits-shaped and pure random streams
    for n in range(500):
        outlen = r(26)
        if n % 5 == 4:
            s = bytes(r(256) for _ in range(r(30)))
        else:
            s = bytearray()
            for _ in range(r(8)):
                t = r(10)
                if t == 0: s.append(128)
                elif t < 5: k = r(10); s.append(k); s += bytes(r(256) for _ in range(k + 1))
                else: s.append(257 - (r(8) + 2) if t < 9 else 257 - (r(40) + 2)); s.append(r(256))
            if r(3) == 0: s = s[:r(len(s) + 1)]
        cases.append(f"U {outlen} {hx(s)}")
    # --- zlib: valid streams, dst size variants, damaged streams
    payloads = [b"", b"a", b"abcabcabcabcabc", bytes(range(40)), b"\x00" * 33,
                bytes(r(256) for _ in range(30)), bytes(r(4) for _ in range(60)), b"xyxyxyxy" * 5]
    for p in payloads:
        for kind in range(4):
            z = zstream(p, kind)
            for dl in sorted({len(p), len(p) + 7, max(len(p) - 1, 0), len(p) // 2, 0, 1}):
                cases.append(f"Z {dl} {hx(z)}")
            for k in sorted({0, 1, 2, 3, len(z) // 2, len(z) - 5, len(z) - 4, len(z) - 1}):
                if 0 <= k < len(z):
                    cases.append(f"Z {len(p)} {hx(z[:k])}")
                    cases.append(f"Z {len(p) + 4} {hx(z[:k])}")
            for _ in range(6):
                m = bytearray(z)
                if m: m[r(len(m))] ^= 1 << r(8)
                dl = len(p) + r(3)
                # zlib stops at an empty output buffer before it validates the next
                # back-reference (see "Deviations" in psdutils.rs): not recorded
                if dl > 0: cases.append(f"Z {dl} {hx(m)}")
            bad = bytearray(z); bad[-1] ^= 0x55
            cases.append(f"Z {len(p)} {hx(bad)}")
            cases.append(f"Z {len(p) + 3} {hx(bad)}")
            if len(p) > 2: cases.append(f"Z {len(p) - 2} {hx(bad)}")
            cases.append(f"Z {len(p)} {hx(z + b'garbage')}")
    for hdr in (b"\x78\x00", b"\x00\x00", b"\x78\xbb", b"\x78\x20", b"\x58\x85"):
        cases.append(f"Z 8 {hx(hdr + stored(b'12345678') + adler(b'12345678'))}")
    cases += ["Z 8 -", "Z 0 -", "Z 5 78", "Z 0 78", "Z 0 789c", "Z 0 789c0300", "Z 4 789c"]
    # --- with prediction: valid streams of whole rows (the C++ overruns otherwise)
    for n in range(120):
        depth = 16 if n % 2 else 8
        row = r(6) + 1
        rows = r(4) + 1
        size = row * rows * (2 if depth == 16 else 1)
        data = bytes(r(256) if n % 3 else r(3) for _ in range(size))
        cases.append(f"P {size} {row} {depth} {hx(zstream(data, n % 4))}")
    cases.append(f"P 8 4 16 {hx(zstream(bytes(8), 0)[:-3])}")
    cases.append("P 4 4 8 -")
    return cases


def build_ref(t):
    for n in ("psdutils.cpp", "psdutils.h"):
        d = subprocess.check_output(["git", "-C", REPO, "show", f"{REF}:flare/sources/common/psdlib/{n}"])
        open(os.path.join(t, "orig_psdutils.cpp" if n.endswith("cpp") else n), "wb").write(d)
    exe = os.path.join(t, "ref")
    subprocess.check_call(["g++", "-std=c++17", "-O1", "-D_GLIBCXX_ASSERTIONS", f"-I{t}",
                           os.path.join(HERE, "psdutils_ref.cpp"), "-lz", "-o", exe])
    return exe


def splice(rs, body, check):
    s = open(rs).read()
    pat = re.compile(r"(    // BEGIN GOLDEN\n).*?(    // END GOLDEN)", re.S)
    new = pat.sub(lambda m: m.group(1) + body + m.group(2), s)
    if check: return new == s
    open(rs, "w").write(new)
    return True


def main():
    check = "--check" in sys.argv
    with tempfile.TemporaryDirectory() as t:
        exe = build_ref(t)
        inp = "\n".join(make_cases()) + "\n"
        out = subprocess.run([exe], input=inp.encode(), check=True, stdout=subprocess.PIPE).stdout.decode()
    body = "".join("    " + l + "\n" for l in out.splitlines())
    if not splice(os.path.join(SRC, "common/psdlib/psdutils.rs"), body, check):
        print("golden drifted"); sys.exit(1)
    print(f"{len(out.splitlines())} cases")


if __name__ == "__main__":
    main()
