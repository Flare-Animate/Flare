"""Verify exported SVGs against an independent decode of the source XML.

Pairs each SVG with the exact <DOMShape> it came from using the exporter's
manifest, then checks:
  * the point list matches an independent Python decode, coordinate for
    coordinate;
  * the SVG's declared viewBox actually contains all the geometry, so no shape
    is silently clipped;
  * the file parses as XML.

The viewBox check is the one that catches a decoder computing bounds from the
wrong point set -- the failure where the art renders but part of it is missing.

usage: verify_svg.py <extracted-fla-dir> <svg-out-dir>
"""
import glob
import os
import re
import sys
import xml.etree.ElementTree as ET

NS = "{http://ns.adobe.com/xfl/2008/}"
SVG_NS = "{http://www.w3.org/2000/svg}"
NUM = r"(-?(?:\d+\.?\d*)|#(?:[0-9A-Fa-f]{1,6}\.[0-9A-Fa-f]{0,2}))"


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
            i += 1; continue
        if c in "!|":
            yield ("op", c); i += 1; continue
        if c in "[]":
            yield ("op", "["); i += 1; continue
        if c == "/":
            yield ("op", "|"); i += 1; continue
        if c == "S":
            i += 2; continue
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
        arity = 4 if v == "[" else 2
        for k in range(arity):
            if i + 1 + k >= len(toks) or toks[i + 1 + k][0] != "n":
                raise ValueError("truncated")
        nums = [toks[i + 1 + k][1] for k in range(arity)]
        if v == "!":
            dest = (nums[0] / 20.0, nums[1] / 20.0)
            restates = (at is not None and cur and
                        abs(at[0] - dest[0]) < 1e-9 and abs(at[1] - dest[1]) < 1e-9)
            if cur and not restates:
                contours.append(cur); cur = []
            at = dest
            if not restates:
                cur.append(dest)
        elif v == "|":
            at = (nums[0] / 20.0, nums[1] / 20.0); cur.append(at)
        else:
            cur.append((nums[0] / 20.0, nums[1] / 20.0))
            at = (nums[2] / 20.0, nums[3] / 20.0); cur.append(at)
        i += 1 + arity
    if cur:
        contours.append(cur)
    return contours


def svg_points(d):
    """(x, y) pairs from an M/Q path.

    An M takes one pair; a Q takes two (control, then endpoint), and the
    endpoint has no command letter in front of it -- so the numbers are read as
    a flat stream and grouped by how many each command consumes.
    """
    NUMTOK = re.compile(r"[-+]?[\d.]+(?:[eE][-+]?\d+)?")
    pts = []
    i = 0
    n = len(d)
    while i < n:
        c = d[i]
        if c not in "MQL":
            i += 1
            continue
        i += 1
        want = 1 if c in "ML" else 2
        for _ in range(want):
            m = NUMTOK.search(d, i)
            if not m:
                return pts
            x = float(m.group(0))
            m2 = NUMTOK.search(d, m.end())
            if not m2:
                return pts
            y = float(m2.group(0))
            pts.append((x, y))
            i = m2.end()
    return pts


def main():
    src, outdir = sys.argv[1], sys.argv[2]

    # The exporter's manifest pairs each SVG with its source file and index.
    manifest = os.path.join(outdir, "manifest.tsv")
    if not os.path.isfile(manifest):
        print(f"   no manifest at {manifest}; rerun shape_export")
        return 1
    rows = []
    with open(manifest, encoding="utf-8") as f:
        header = f.readline()
        for line in f:
            if line.strip():
                rows.append(line.rstrip("\n").split("\t"))
    print(f"manifest rows        : {len(rows)}")

    # Decode every source document once, keeping DOMShape elements in document
    # order, which is the order the exporter walked them.
    # Key on the path relative to `src`, not the basename: an FLA has many
    # same-named symbols in different folders, so a basename key collapses them
    # and most manifest rows then find no document at all.
    docs = {}
    for path in glob.glob(os.path.join(src, "**", "*.xml"), recursive=True):
        rel = os.path.relpath(path, src).replace("\\", "/")
        try:
            # Only the shapes that carry an `edges` attribute, which is what the
            # exporter numbered: shapes with no geometry are skipped there, so
            # including them here would shift every index.
            keep = []
            for el in ET.parse(path).iter(NS + "DOMShape"):
                edges = el.find(NS + "edges")
                if edges is not None and any(e.get("edges")
                                             for e in edges):
                    keep.append(el)
            docs[rel] = keep
        except (ET.ParseError, OSError):
            pass

    checked = mismatch = clipped = unparsable = missing = 0
    count_disagree = 0
    bad_rows = 0
    problems = []
    for row in rows:
        if len(row) != 5:
            # A truncated or stray line. Reported, not raised: a manifest this
            # script cannot read is a failure of the thing being verified, and
            # aborting with a traceback hides every other check.
            bad_rows += 1
            if len(problems) < 5:
                problems.append(("<manifest>", f"malformed row: {row!r}"))
            continue
        name, source, index, contours, points = row
        svg_path = os.path.join(outdir, name)
        if not os.path.isfile(svg_path):
            missing += 1
            continue
        shapes = docs.get(source)
        if not shapes:
            missing += 1
            continue
        idx = int(index) - 1
        if idx >= len(shapes):
            missing += 1
            continue
        el = shapes[idx]
        edges = el.find(NS + "edges")
        want = []
        if edges is not None:
            for e in edges:
                d = e.get("edges")
                if not d:
                    continue
                try:
                    for c in decode(d):
                        if len(c) >= 2:
                            want.extend(c)
                except ValueError:
                    pass

        try:
            root = ET.parse(svg_path).getroot()
        except ET.ParseError:
            unparsable += 1
            continue
        path_el = root.find(SVG_NS + "path")
        if path_el is None:
            problems.append((name, "no <path>"))
            mismatch += 1
            continue

        checked += 1
        got = svg_points(path_el.get("d"))
        # The exporter's own counts, cross-checked against this decode. They were
        # in the manifest and unused, which was the cheapest available
        # independent signal that the two sides agreed on which shape a row
        # refers to.
        if len(got) != int(points):
            count_disagree += 1
            if len(problems) < 5:
                problems.append((name, f"manifest says {points} points, "
                                       f"SVG has {len(got)}"))
        if len(got) != len(want):
            mismatch += 1
            if len(problems) < 5:
                problems.append((name, f"{len(got)} points in SVG vs {len(want)}"))
            continue
        for (gx, gy), (wx, wy) in zip(got, want):
            if abs(gx - wx) > 0.002 or abs(gy - wy) > 0.002:
                mismatch += 1
                if len(problems) < 5:
                    problems.append((name, f"point ({gx},{gy}) vs ({wx},{wy})"))
                break

        vb = [float(v) for v in root.get("viewBox").split()]
        vx, vy, vw, vh = vb
        for (x, y) in want:
            if not (vx - 0.01 <= x <= vx + vw + 0.01 and
                    vy - 0.01 <= y <= vy + vh + 0.01):
                clipped += 1
                if len(problems) < 5:
                    problems.append((name, f"point ({x},{y}) outside viewBox {vb}"))
                break

    print(f"checked              : {checked}")
    print(f"missing / unpaired   : {missing}")
    print(f"malformed rows       : {bad_rows}")
    print(f"unparsable SVG       : {unparsable}")
    print(f"manifest count differs: {count_disagree}")
    print(f"point mismatches     : {mismatch}")
    print(f"shapes with clipped art: {clipped}")
    for name, why in problems:
        print(f"   {name}: {why}")
    ok = (mismatch == 0 and clipped == 0 and unparsable == 0 and missing == 0
          and count_disagree == 0 and bad_rows == 0
          and checked == len(rows))
    print()
    print("PASSED" if ok else "FAILED")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
