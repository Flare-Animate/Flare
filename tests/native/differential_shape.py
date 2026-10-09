"""Differential test: the C++ XFL shape decoder against an independent Python one.

Two implementations written from the format description, run over the same real
FLA, compared field by field. They must agree exactly on counts and to within a
rounding step on coordinates -- if they do not, one of them is wrong, and a
self-consistent C++ test cannot tell you which.

usage: differential_shape.py <extracted-fla-dir> [census-exe]
"""
import glob
import os
import re
import subprocess
import sys
import xml.etree.ElementTree as ET

HERE = os.path.dirname(os.path.abspath(__file__))
NS = "{http://ns.adobe.com/xfl/2008/}"
NUM = r"(-?(?:\d+\.?\d*)|#(?:[0-9A-Fa-f]{1,6}\.[0-9A-Fa-f]{0,2}))"

# The same tolerance the C++ decoder uses, for the same two tests:
# a restated moveTo, and a contour that returns to its start. It has
# to match exactly, or the two implementations answer different
# questions and a disagreement here says nothing about either one.
# Five coordinate quanta (1/256 twip is 1.95e-4 px).
EPSILON = 1e-3


# Note: no `assert` anywhere below. Python strips asserts under `python -O`, and
# these are structural guards the differential comparison depends on -- with them
# gone, a malformed edge is silently skipped on one side and the run still
# reports agreement.


def decode_number(tok):
    if not tok.startswith("#"):
        return float(tok)
    body = tok[1:]
    d = body.find(".")
    ih = (body if d < 0 else body[:d]).rjust(6, "0")
    fh = ("" if d < 0 else body[d + 1:]).ljust(2, "0")
    raw = int(ih + fh, 16)
    if raw >= 2 ** 31:
        raw -= 2 ** 32
    return raw / 256.0


def tokenize(s):
    i, n = 0, len(s)
    while i < n:
        c = s[i]
        if c in " \t\n\r":
            i += 1
            continue
        if c in "!|":
            yield ("op", c); i += 1; continue
        if c in "[]":
            yield ("op", "["); i += 1; continue
        if c == "/":
            yield ("op", "|"); i += 1; continue   # a lineTo, like '|'
        if c == "S":
            if i + 1 >= n or not s[i + 1].isdigit():
                raise ValueError("S without a style digit")
            i += 2
            continue
        m = re.match(NUM, s[i:])
        if not m:
            raise ValueError(f"cannot lex at {i}: {s[i:i+20]!r}")
        yield ("n", decode_number(m.group(1)))
        i += m.end()


def decode(s):
    toks = list(tokenize(s))
    contours, cur = [], []
    at = None
    i = 0
    while i < len(toks):
        kind, v = toks[i]
        if kind != "op":
            # A number where an opcode belongs. The C++ decoder rejects this too;
            # letting it through here would make one side fail and the other
            # succeed on the same input, which is the whole thing this file
            # exists to detect.
            raise ValueError(f"number where an opcode belongs, at token {i}")
        arity = 4 if v == "[" else 2
        for k in range(arity):
            if i + 1 + k >= len(toks) or toks[i + 1 + k][0] != "n":
                raise ValueError("truncated coordinate list")
        nums = [toks[i + 1 + k][1] for k in range(arity)]
        if v == "!":
            dest = (nums[0] / 20.0, nums[1] / 20.0)
            restates = (at is not None and cur and
                        abs(at[0] - dest[0]) < EPSILON and abs(at[1] - dest[1]) < EPSILON)
            if cur and not restates:
                contours.append(cur); cur = []
            at = dest
            # A restatement adds no geometry, so it must add no point either.
            # Recording the duplicate desynchronises the point list from the
            # segment list, shifting every control point after it -- which is
            # the very misreading this file is written to catch, and which it
            # was itself making.
            if not restates:
                cur.append(dest)
        elif v == "|":
            at = (nums[0] / 20.0, nums[1] / 20.0)
            cur.append(at)
        else:
            cur.append((nums[0] / 20.0, nums[1] / 20.0))
            at = (nums[2] / 20.0, nums[3] / 20.0)
            cur.append(at)
        i += 1 + arity
    if cur:
        contours.append(cur)
    return contours


def python_census(root):
    edges = parsed = failed = contours = closed = 0
    xs, ys = [], []
    for path in glob.glob(os.path.join(root, "**", "*.xml"), recursive=True):
        try:
            tree = ET.parse(path)
        except ET.ParseError:
            continue
        for el in tree.iter(NS + "Edge"):
            d = el.get("edges")
            if not d:
                continue
            edges += 1
            try:
                cs = decode(d)
            except ValueError:
                failed += 1
                continue
            parsed += 1
            for c in cs:
                if len(c) < 2:
                    continue
                contours += 1
                if (abs(c[0][0] - c[-1][0]) < EPSILON
                        and abs(c[0][1] - c[-1][1]) < EPSILON):
                    closed += 1
                for x, y in c:
                    xs.append(x); ys.append(y)
    # `failed` counted in the loop, so both sides measure the same thing. It was
    # derived as edges - parsed here, which agrees only because every edge is
    # either parsed or failed -- a distinction that stops holding as soon as one
    # side skips an edge for a different reason.
    return dict(edges=edges, parsed=parsed, failed=failed,
                contours=contours, closed=closed,
                minx=min(xs) if xs else 0, maxx=max(xs) if xs else 0,
                miny=min(ys) if ys else 0, maxy=max(ys) if ys else 0)


def cpp_census(root, exe):
    out = subprocess.run([exe, root], capture_output=True, text=True)
    if out.returncode:
        raise SystemExit(f"shape_census failed ({out.returncode}): {out.stderr[:400]}")
    d = {}
    for line in out.stdout.splitlines():
        if "=" in line:
            k, v = line.split("=", 1)
            try:
                d[k] = int(v) if "." not in v else float(v)
            except ValueError:
                pass
    return d


def main():
    if len(sys.argv) < 2:
        raise SystemExit("usage: differential_shape.py <extracted-fla-dir> [exe]")
    root = sys.argv[1]
    exe = sys.argv[2] if len(sys.argv) > 2 else os.path.join(
        HERE, "build", "RelWithDebInfo", "shape_census.exe")
    if not os.path.isfile(exe):
        raise SystemExit(f"shape_census not built: {exe}")

    py = python_census(root)
    cpp = cpp_census(root, exe)

    print("C++ decoder vs an independent Python decoder, same document\n")
    print(f"{'field':<10} {'C++':>12} {'Python':>12}   agree")
    fails = 0
    for k in ("edges", "parsed", "failed", "contours", "closed"):
        a, b = cpp.get(k), py.get(k)
        ok = a == b
        if not ok:
            fails += 1
        print(f"{k:<10} {a if a is not None else '?':>12} {b:>12}   "
              f"{'yes' if ok else 'NO'}")
    for k in ("minx", "maxx", "miny", "maxy"):
        a, b = cpp.get(k), py.get(k)
        # Both print two decimals, so they must match to the cent.
        ok = a is not None and abs(a - b) <= 0.01
        if not ok:
            fails += 1
        print(f"{k:<10} {a if a is not None else '?':>12} {b:>12.2f}   "
              f"{'yes' if ok else 'NO'}")

    print()
    if fails:
        print(f"FAILED: {fails} field(s) disagree")
        return 1
    print("PASSED: the two decoders agree on every field")
    return 0


if __name__ == "__main__":
    sys.exit(main())
