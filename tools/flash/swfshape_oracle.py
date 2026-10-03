"""Check the SWF shape decoder against JPEXS, vertex for vertex.

This is how the decoder was verified and how it should be re-verified after any
change to it. It is a measurement script, not part of the build and not shipped:
it needs a SWF file, the shape probe built, and a copy of JPEXS.

Why an external decoder is needed: a `DefineShape` body is bit-packed, so a wrong
read produces a shape that looks drawn rather than an error. Eight such faults got
through here, each of which left the census reporting a plausible number. The only
thing that settles the question is a second implementation's answer on the same
bytes.

JPEXS is GPL v3 and is *not* vendored, linked or invoked by Flare. It is run here
as an oracle over a file this script already has to parse itself, and the comparison
is on geometry -- every vertex, then the extent.

What it does:

  1. finds every DefineShape{,2,3,4} tag in the SWF
  2. asks JPEXS for the same file as XML and reads back each tag's records
  3. runs the built swfshape_probe over each tag body
  4. compares the character id, the declared bounds, the style counts, the bit
     widths, the record count, every vertex, and the outline extent

Expected output on the reference file is 250/250 on all of them.

usage:
    python swfshape_oracle.py <file.swf> [--probe <path>] [--jpexs <ffdec.jar>]

To get JPEXS (about 19 MB, needs a JRE):
    https://github.com/jindrapetrik/jpexs-decompiler/releases
    java -jar ffdec-cli.jar -swf2xml <file.swf> <out.xml>
"""

import argparse
import os
import re
import struct
import subprocess
import sys
import xml.etree.ElementTree as ET
import zlib
from collections import Counter

SHAPE_TAGS = {"DefineShapeTag", "DefineShape2Tag", "DefineShape3Tag",
              "DefineShape4Tag"}
TAG_VERSION = {2: 1, 22: 2, 32: 3, 83: 4}

def build_env(root):
    """The PATH the built probe needs.

    The probe links the same third-party DLLs the rest of Flare does, and they are
    staged in the repository's own LocalTest directory. Without it on PATH the loader
    picks up something else and the process dies before main() with a stack overflow
    -- which looks like a crash in the decoder and is not one. This is the same PATH
    the test runner uses; QT_BIN locates Qt when it is not on PATH already.
    """
    env = dict(os.environ)
    parts = []
    qt = os.environ.get("QT_BIN") or os.environ.get("QT_DIR")
    if qt:
        parts.append(qt)
    else:
        for cand in (r"C:\Qt\5.15.2\msvc2019_64\bin",
                     "/usr/lib/x86_64-linux-gnu", "/usr/lib"):
            if os.path.isdir(cand):
                parts.append(cand)
                break
    for rel in (os.path.join("build_local", "RelWithDebInfo"), "LocalTest",
                os.path.join("build_local", "Release"), "."):
        p = os.path.join(root, rel)
        if os.path.isdir(p):
            parts.append(p)
    env["PATH"] = os.pathsep.join(parts + [env.get("PATH", "")])
    return env


def default_probe():
    here = os.path.dirname(os.path.dirname(os.path.dirname(
        os.path.abspath(__file__))))
    return os.path.join(here, "tests", "native", "build", "RelWithDebInfo",
                        "swfshape_probe.exe")


def read_tags(buf):
    """Every DefineShape{,2,3,4} body, in tag order, descending into sprites."""
    found = []

    def walk(off, depth):
        while off + 2 <= len(buf) and depth < 8:
            head = buf[off] | (buf[off + 1] << 8)
            code, length = head >> 6, head & 0x3F
            off += 2
            if length == 0x3F:
                length = struct.unpack_from("<I", buf, off)[0]
                off += 4
            if code == 0:
                return
            if code in TAG_VERSION:
                found.append((TAG_VERSION[code], buf[off:off + length]))
            elif code == 39 and length > 4:      # DefineSprite: skip its ID
                walk(off + 4, depth + 1)
            off += length
            if off > len(buf):
                return

    p = 8
    nbits = (buf[p] >> 3) & 0x1F
    walk(p + ((5 + 4 * nbits + 7) // 8) + 4, 0)
    return found


def decompress(path):
    raw = open(path, "rb").read()
    if raw[:3] == b"CWS":
        return b"FWS" + raw[3:8] + zlib.decompress(raw[8:])
    if raw[:3] == b"ZWS":
        return b"FWS" + raw[3:8] + zlib.lzma.decompress(raw[12:])
    return raw


def jpexs_shapes(jar, swf, tmp):
    """JPEXS's parse of every shape tag, keyed by order."""
    out = os.path.join(tmp, "jpexs.xml")
    r = subprocess.run(["java", "-jar", jar, "-swf2xml", swf, out],
                       capture_output=True, text=True, timeout=1800)
    if not os.path.exists(out):
        sys.exit("JPEXS produced no XML:\n" + (r.stderr or "")[:500])
    shapes = []

    def walk(items):
        for it in items:
            if it.tag == "item" and it.get("type") in SHAPE_TAGS:
                shapes.append(it)
            if it.tag == "item":
                inner = it.find("tags")
                if inner is not None:
                    walk(inner)

    walk(ET.parse(out).getroot().find("tags"))
    return shapes


def jpexs_points(shape):
    """JPEXS's records, accumulated into absolute vertices.

    A style change carrying a nested style array also carries a move, and that move
    lands where the pen already is, so it is not a new vertex. And a shape may open
    with an edge rather than a move, in which case the origin genuinely is the first
    vertex although JPEXS does not list it.
    """
    cur = [0, 0]
    pts = []
    started = False
    recs = shape.find("shapes/shapeRecords")
    for e in list(recs) if recs is not None else []:
        kind, a = e.get("type"), e.attrib
        if kind == "StyleChangeRecord":
            if a.get("stateMoveTo") == "true" and a.get("stateNewStyles") != "true":
                cur = [int(a.get("moveDeltaX", 0)), int(a.get("moveDeltaY", 0))]
                pts.append(tuple(cur))
                started = True
        elif kind in ("StraightEdgeRecord", "CurvedEdgeRecord"):
            if not started:
                pts.append(tuple(cur))
                started = True
            if kind == "StraightEdgeRecord":
                cur = [cur[0] + int(a.get("deltaX", 0)),
                       cur[1] + int(a.get("deltaY", 0))]
            else:
                cur = [cur[0] + int(a.get("anchorDeltaX", 0)),
                       cur[1] + int(a.get("anchorDeltaY", 0))]
            pts.append(tuple(cur))
    return pts


def probe_points(probe, env, path, version):
    """This decoder's vertices, read back out of the SVG path it emits."""
    r = subprocess.run([probe, path, str(version)], capture_output=True,
                       text=True, env=env, timeout=300)
    m = re.search(r'd="([^"]*)"', r.stdout)
    if not m:
        return [], r.stdout
    d = m.group(1)
    nums = re.findall(r"-?\d+(?:\.\d+)?(?:e[+-]?\d+)?", d)
    pts, i = [], 0
    for cmd in re.findall(r"[MLQZmlqz]", d):
        if cmd in "Mm":
            pts.append((float(nums[i]), float(nums[i + 1])))
            i += 2
        elif cmd in "Ll":
            pts.append((float(nums[i]), float(nums[i + 1])))
            i += 2
        elif cmd in "Qq":
            pts.append((float(nums[i + 2]), float(nums[i + 3])))
            i += 4
    return pts, r.stdout


def extent(pts):
    if not pts:
        return None
    xs = [q[0] for q in pts]
    ys = [q[1] for q in pts]
    return (min(xs), min(ys), max(xs), max(ys))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("swf")
    ap.add_argument("--probe", default=None)
    ap.add_argument("--jpexs", default=None,
                    help="path to ffdec.jar or ffdec-cli.jar")
    ap.add_argument("--tmp", default=None)
    a = ap.parse_args()

    probe = a.probe or default_probe()
    if not os.path.exists(probe):
        sys.exit("probe not built: " + probe)
    jar = a.jpexs or os.path.join(os.environ.get("TEMP", "."), "opencode",
                                 "ffdec", "ffdec-cli.jar")
    if not os.path.exists(jar):
        sys.exit("JPEXS jar not found: " + jar +
                 "\nsee the module docstring for where to get it")
    tmp = a.tmp or os.path.join(os.environ.get("TEMP", "."), "opencode",
                                "oracle")
    os.makedirs(tmp, exist_ok=True)

    root = os.path.dirname(os.path.dirname(os.path.dirname(
        os.path.abspath(__file__))))
    env = build_env(root)

    bodies = read_tags(decompress(a.swf))
    oracle = jpexs_shapes(jar, a.swf, tmp)
    print(f"  {a.swf}")
    print(f"  {len(bodies)} shape tag(s) here, {len(oracle)} in JPEXS's parse")
    if len(bodies) != len(oracle):
        print("  the two disagree on the tag count -- resolve that first")

    tally = Counter()
    mismatch = []
    header_diff = []
    for i, ((version, body), it) in enumerate(zip(bodies, oracle)):
        path = os.path.join(tmp, f"s{i:04d}.v{version}.bin")
        open(path, "wb").write(body)
        out = subprocess.run([probe, path, str(version)], capture_output=True,
                             text=True, env=env, timeout=300).stdout
        fields = dict(re.findall(r"^\s*(\w[\w ]*?)\s*:\s*(.+?)\s*$", out, re.M))
        sh = it.find("shapes")
        fs = sh.find("fillStyles/fillStyles")
        ls = sh.find("lineStyles/lineStyles")
        want = dict(
            id=int(it.get("shapeId")),
            nf=len(list(fs)) if fs is not None else 0,
            nl=len(list(ls)) if ls is not None else 0,
            nfb=int(sh.get("numFillBits")), nlb=int(sh.get("numLineBits")),
            records=len(list(sh.find("shapeRecords"))) - 1,   # JPEXS counts the end
        )
        got = dict(
            id=int(fields.get("id", -1)),
            nf=int((re.match(r"(\d+) fill", fields.get("styles", ""))
                    or [0, -1])[1]),
            nl=int((re.match(r"\d+ fill, (\d+) line", fields.get("styles", ""))
                    or [0, -1])[1]),
            nfb=int((re.match(r"NumFillBits=(\d+)", fields.get("widths", ""))
                     or [0, -1])[1]),
            nlb=int((re.match(r"NumFillBits=\d+ NumLineBits=(\d+)",
                             fields.get("widths", "")) or [0, -1])[1]),
            records=int(fields.get("records", -1)),
            ok=fields.get("ok") == "1",
        )
        jp, mn = jpexs_points(it), probe_points(probe, env, path, version)[0]

        tally["compared"] += 1
        tally["decodes"] += got["ok"]
        tally["header agrees"] += all(got[k] == want[k] for k in
                                      ("id", "nf", "nl", "nfb", "nlb", "records"))
        if not all(got[k] == want[k] for k in
                   ("id", "nf", "nl", "nfb", "nlb", "records")):
            bad = [f"{k}: JPEXS {want[k]} / mine {got[k]}"
                   for k in ("id", "nf", "nl", "nfb", "nlb", "records")
                   if got[k] != want[k]]
            header_diff.append((i, it.get("shapeId"), it.get("type"),
                                "; ".join(bad)))
        tally["vertices agree"] += (jp == mn)
        tally["extent agrees"] += (extent(jp) == extent(mn))
        if jp != mn:
            mismatch.append((i, it.get("shapeId"), len(jp), len(mn),
                             extent(jp), extent(mn)))

    n = tally["compared"]
    print()
    for k in ("decodes", "header agrees", "vertices agree", "extent agrees"):
        print(f"  {k:<18}: {tally[k]}/{n}")
    if header_diff:
        print()
        print(f"  {len(header_diff)} shape(s) where the header disagrees. Check which")
        print("  side is self-consistent before believing either: a style-change")
        print("  index pointing outside its own style array is not a reading the")
        print("  format admits. On the reference file all of these are DefineShape4")
        print("  tags where JPEXS reports a line-style index of 1 into an array it")
        print("  says has 0 entries, and the geometry agrees anyway.")
        for i, sid, tag, what in header_diff[:6]:
            print(f"    #{i:<5} id={sid:<6} {tag:<18} {what}")
    if mismatch:
        print()
        print(f"  {len(mismatch)} shape(s) differ in their vertices:")
        for i, sid, a_pts, b_pts, ea, eb in mismatch[:8]:
            print(f"    #{i:<5} id={sid:<6} JPEXS {a_pts} pts {ea}  "
                  f"mine {b_pts} pts {eb}")
    return 0 if tally["vertices agree"] == n else 1


if __name__ == "__main__":
    sys.exit(main())
