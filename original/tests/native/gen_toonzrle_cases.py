"""Re-derive the recorded golden in toonzrle.rs from the ORIGINAL decode kernels.

The cm16/cm24 decoders of image/tzp/toonzrle.cpp are `static` inside a file that
needs libtiff's private headers, so they are cut out of git (REF, the commit
before the port) and compiled with g++ next to toonzrle_ref.cpp. The streams are
made here (an LCG): biased random opcode streams plus terminators, so most of
them run to an end marker. For every stream that decodes, extra rows pin the
extent handling of the port (output capacity one short, input one byte short).
The output lines are spliced between `// BEGIN GOLDEN` / `// END GOLDEN` in the
Rust test module.

Only platform-independent cases are recorded (no libc); cases whose output would
exceed 300 pixels, or that crash the original on its guard page, are dropped.

Usage: python tests/native/gen_toonzrle_cases.py [--check]
Needs g++. Not part of the test run; the golden is committed.
"""
import os, re, subprocess, sys, tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", ".."))
SRC = os.path.join(REPO, "flare", "sources")
REF = os.environ.get("PORT_C_REF", "95dd5e2")
RS = os.path.join(SRC, "image/tzp/toonzrle.rs")
MAXDEC = 300


class Lcg:
    def __init__(self, s): self.s = s
    def __call__(self, n=1 << 31):
        self.s = (self.s * 6364136223846793005 + 1442695040888963407) & (2**64 - 1)
        return (self.s >> 33) % n


PARAMS = [  # tone_bits color_offs color_bits pencil_offs pencil_bits offset_mask
    (4, 4, 7, 11, 5, 0), (4, 4, 5, 9, 2, 0x0800), (3, 2, 6, 9, 5, 0x10),
    (2, 0, 8, 8, 8, 0), (4, 4, 8, 12, 4, 0x0100), (1, 1, 1, 2, 1, 0xFFFF),
]


TERMS = [b"\xfe", b"\xff\x00", b"\xff\x1a\x30", b"\xfe\x00", b"\xfe\x01", b"\xfe\x02\x01\x02\x03\x04", b"\xfe\x02\x01\x02\x03"]


def stream(r, cm16):
    n = r(24)
    b = bytearray()
    for _ in range(n):
        t = r(10)
        if t < 4: b.append(r(256))
        elif t < 7: b.append(0xF0 | r(16) if cm16 else 0xF6 + r(10))
        elif t < 9: b.append(r(16) * 16 + 0xF if cm16 else 0xFC + r(4))
        else: b.append(r(3))
    if r(4):  # mostly terminate
        b += TERMS[r(len(TERMS))]
    return bytes(b)


def hx(b): return bytes(b).hex() or "-"


def run(exe, lines):
    out = subprocess.run([exe], input=("\n".join(lines) + "\n").encode(), check=True, stdout=subprocess.PIPE).stdout.decode()
    return out.splitlines()


def build_ref(t):
    txt = subprocess.check_output(["git", "-C", REPO, "show", f"{REF}:flare/sources/image/tzp/toonzrle.cpp"]).decode()
    i = txt.index("//-------------------- DECODE CM16")
    j = txt.index("//=====", i)
    region = txt[i:j].rstrip()
    assert region.endswith("}")
    region = region[:-1]  # drop the closing brace of the original `extern "C" {`
    open(os.path.join(t, "orig_kernels.inc"), "w").write(
        "static int tif_toonz1_decode_cm16(UCHAR* buf_in, int* buf_in_len, USHORT* buf_out, int tone_bits,\n"
        "  int color_offs, int color_bits, int pencil_offs, int pencil_bits, USHORT offset_mask);\n"
        "static int tif_toonz1_decode_cm24(UCHAR* buf_in, int* buf_in_len, TUINT32* buf_out);\n" + region)
    exe = os.path.join(t, "ref")
    subprocess.check_call(["g++", "-std=c++17", "-O1", "-fpermissive", "-w", f"-I{t}", os.path.join(HERE, "toonzrle_ref.cpp"), "-o", exe])
    return exe


def make_cases(exe):
    r = Lcg(777)
    base = []
    for n in range(1800):
        p = PARAMS[n % len(PARAMS)]
        s = stream(r, True)
        base.append(("A", p, s))
    for n in range(1400):
        base.append(("B", None, stream(r, False)))
    def line(k, p, s, incap, outcap):
        if k == "A": return "A %d %d %d %d %d %d %d %d %s" % (*p, incap, outcap, hx(s))
        return "B %d %d %s" % (incap, outcap, hx(s))
    # pass 1: find (dec, enc) with a huge capacity
    first = run(exe, [line(k, p, s, len(s), 1 << 20) for k, p, s in base])
    rows = []
    for (k, p, s), res in zip(base, first):
        tail = res.split("=> ")[1]
        if tail == "SKIP": continue
        dec, enc = int(tail.split()[0]), int(tail.split()[1])
        if dec > MAXDEC: continue
        rows.append((k, p, s, len(s), dec))
        if dec > 0:
            rows.append((k, p, s, len(s), dec - 1))      # output one pixel short
            rows.append((k, p, s, len(s), dec + 3))      # output with room to spare
            if enc >= 1:
                rows.append((k, p, s[:enc - 1], enc - 1, dec))  # input one byte short
    res = run(exe, [line(k, p, s, ic, oc) for k, p, s, ic, oc in rows])
    return [l for l in res if not l.endswith("SKIP")]


def splice(body, check):
    s = open(RS).read()
    pat = re.compile(r"(    // BEGIN GOLDEN\n).*?(    // END GOLDEN)", re.S)
    new = pat.sub(lambda m: m.group(1) + body + m.group(2), s)
    if check: return new == s
    open(RS, "w").write(new)
    return True


def main():
    check = "--check" in sys.argv
    with tempfile.TemporaryDirectory() as t:
        exe = build_ref(t)
        out = make_cases(exe)
    body = "".join("    " + l + "\n" for l in out)
    if not splice(body, check):
        print("golden drifted"); sys.exit(1)
    print(f"{len(out)} cases")


if __name__ == "__main__":
    main()
