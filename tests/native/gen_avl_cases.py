"""Re-derive the recorded golden in avl.rs from the ORIGINAL avl.c.

avl.c is deleted, so it is rebuilt from git (REF, the commit before the
retirement) with gcc, linked to avl_ref.cpp, and run on a script made here (an
LCG): for each of the 20 tree kinds a few random sessions of inserts, the five
locate relations, removes, scans, start/next/prev walks, link chains, dumps,
copies, release and reset, plus `cdb_decode_all` on generated colour-name tables.
The script carries its own inputs; the output lines (`<op> => <result>`) are
spliced between `// BEGIN GOLDEN` / `// END GOLDEN` in the Rust test module.

Platform independent by construction: ids instead of pointers, keys inside
0..2^31, chars 0..127, and `atoi` only on plain digit strings.

Usage: python tests/native/gen_avl_cases.py [--check]
Needs gcc and g++. Not part of the test run; the golden is committed.
"""
import os, re, subprocess, sys, tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", ".."))
SRC = os.path.join(REPO, "flare", "sources")
REF = os.environ.get("PORT_C_REF", "95dd5e2")
RS = os.path.join(SRC, "image/tzp/avl.rs")

KINDS = ["mbr", "ptr", "str", "long", "int", "short", "ulong", "uint", "ushort", "char"]
NITEMS = 60


class Lcg:
    def __init__(self, s): self.s = s
    def __call__(self, n=1 << 31):
        self.s = (self.s * 6364136223846793005 + 1442695040888963407) & (2**64 - 1)
        return (self.s >> 33) % n


def word(r): return "".join("abc"[r(3)] for _ in range(1 + r(3)))


def item(r, i):
    big = r(12) == 0
    return "M %d %d %d %d %d %d %d %d %s %d" % (
        i, r(31) - 15, r(31) - 15, r(31), 2147483647 if big else r(31), 2147483647 if big else r(31),
        r(31) - 15, r(41), word(r), r(26))


def key(r, kind):
    if kind == "str": return word(r)
    if kind in ("ulong", "uint", "ushort"): return str(2147483647 if r(14) == 0 else r(34))
    if kind == "char": return str(r(44))
    if kind in ("mbr", "ptr"): return str(r(28))
    return str(r(35) - 17)


def session(r, kind, dup):
    out = ["T %s_%s" % ("dup" if dup else "nodup", kind)]
    out += [item(r, i) for i in range(NITEMS)]
    pool = list(range(NITEMS))
    for i in range(len(pool) - 1, 0, -1):
        j = r(i + 1); pool[i], pool[j] = pool[j], pool[i]
    for _ in range(230):
        t = r(100)
        if t < 38 and pool: out.append("I %d" % pool.pop())
        elif t < 58: out.append("L %s %s" % (["eq", "ge", "gt", "le", "lt"][r(5)], key(r, kind)))
        elif t < 68: out.append("R %s" % key(r, kind))
        elif t < 73: out.append("W %s %d" % (key(r, kind), r(2)))
        elif t < 77: out.append("G %d" % r(2))
        elif t < 79: out.append("S" if r(2) else "B")
        elif t < 81: out.append("F" if r(2) else "Z")
        elif t < 83: out.append("K %d" % r(2))
        elif t < 86: out.append("D")
        elif t < 88: out.append("C")
        elif t < 91: out.append("N")
        elif t < 92: out += ["E", "N"]; pool = list(range(NITEMS))
        elif t < 93: out += ["Q", "N"]; pool = list(range(NITEMS))
        elif pool:  # walk, then change the tree under the iteration path
            out.append("G %d" % r(2)); out.append("I %d" % pool.pop()); out.append("G 0")
    out.append("S")
    return out


def cdb_cases(r, n):
    cases = []
    for _ in range(n):
        recs = []
        for _ in range(1 + r(4)):
            p = ""
            if r(3): p += ("*" if r(4) == 0 else word(r)) + "\t"
            if r(4) == 0: p += "<!A%s!>" % word(r)
            if r(8) == 0: p += "<!Z%s!>" % word(r)
            p += "*" if r(6) == 0 else word(r)
            if r(2):
                fx = []
                for _ in range(1 + r(2)):
                    np_ = r(3)
                    s = "\t" if not fx else ""
                    s = word(r) + "#%d#" % np_
                    s += " : ".join("%s#%s%s" % (word(r), word(r), "  " if r(5) == 0 else "") for _ in range(np_))
                    fx.append(s)
                p += "\t" + " ! ".join(fx)
            recs.append(p + "|")
        text = "".join(recs)
        n_items = len(recs) + r(2)
        cases.append("P %d %d %s" % (n_items, r(2), text.encode().hex()))
    return cases


def build_ref(t):
    for n in ("avl.c", "avl.h", "tcm.h", "tmacro.h"):
        d = subprocess.check_output(["git", "-C", REPO, "show", f"{REF}:flare/sources/image/tzp/{n}"])
        open(os.path.join(t, n), "wb").write(d)
    subprocess.check_call(["gcc", "-std=gnu11", "-O1", "-w", "-c", f"-I{t}", os.path.join(t, "avl.c"), "-o", os.path.join(t, "avl.o")])
    exe = os.path.join(t, "ref")
    subprocess.check_call(["g++", "-std=c++17", "-O1", "-w", f"-I{t}", os.path.join(HERE, "avl_ref.cpp"), os.path.join(t, "avl.o"), "-o", exe])
    return exe


def make_script():
    r = Lcg(4242)
    lines = []
    for dup in (False, True):
        for kind in KINDS:
            for _ in range(3): lines += session(r, kind, dup)
    lines += cdb_cases(r, 160)
    return lines


def splice(body, check):
    s = open(RS).read()
    pat = re.compile(r"(    // BEGIN GOLDEN\n).*?(    // END GOLDEN)", re.S)
    new = pat.sub(lambda m: m.group(1) + body + m.group(2), s)
    if check: return new == s
    open(RS, "w").write(new)
    return True


def main():
    check = "--check" in sys.argv
    script = make_script()
    with tempfile.TemporaryDirectory() as t:
        exe = build_ref(t)
        out = subprocess.run([exe], input=("\n".join(script) + "\n").encode(), check=True, stdout=subprocess.PIPE).stdout.decode()
    lines = [l for l in out.splitlines() if " => " in l]
    assert len(lines) == len(script), (len(lines), len(script))
    lines = [l for l in lines if not l.endswith(" => SKIP")]  # `P` rows on which the C read garbage (UB)
    body = "".join("    " + l + "\n" for l in lines)
    if not splice(body, check):
        print("golden drifted"); sys.exit(1)
    print(f"{len(lines)} lines")


if __name__ == "__main__":
    main()
