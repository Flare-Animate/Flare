"""Flare layout profile tool: validate / export / import.  python tools/layout_tool.py -h

A layout = a directory of room*.ini + layouts.txt + menubar_template.xml
(schema: doc/LAYOUT_CUSTOMIZATION.md). Bundle = one JSON file, easy for an AI agent to edit.
"""
import argparse, json, os, re, sys
import xml.etree.ElementTree as ET

ROOMS = os.path.join(os.path.dirname(__file__), "..", "stuff/profiles/layouts/rooms")


def validate(d):
    errs = []
    lt = os.path.join(d, "layouts.txt")
    if not os.path.exists(lt):
        return ["missing layouts.txt"]
    for r in open(lt).read().split():
        p = os.path.join(d, r)
        if not os.path.exists(p):
            errs.append("layouts.txt lists missing " + r)
            continue
        t = open(p).read()
        panes = set(re.findall(r"^pane_(\d+)[\\]name=", t, re.M))
        m = re.search(r'hierarchy="([^"]*)"', t)
        h = set(re.findall(r"\d+", m.group(1).replace("-1", ""))) if m else set()
        if not m or h != panes:
            errs.append(r + ": hierarchy " + str(sorted(h)) + " != panes " + str(sorted(panes)))
    for x in os.listdir(d):
        if x.endswith("menubar_template.xml") or x.endswith("_menubar.xml"):
            try:
                ET.parse(os.path.join(d, x))
            except ET.ParseError as e:
                errs.append(x + ": " + str(e))
    return errs


def export(d, out):
    b = {"name": os.path.basename(os.path.abspath(d)), "files": {}}
    for x in sorted(os.listdir(d)):
        b["files"][x] = open(os.path.join(d, x), encoding="utf-8", newline="").read()
    json.dump(b, open(out, "w"), indent=1)


def imp(bundle, dest=ROOMS):
    b = json.load(open(bundle))
    if not re.fullmatch(r"[\w .-]+", b["name"]) or b["name"].startswith("."):
        raise SystemExit("bad layout name")
    d = os.path.join(dest, b["name"])
    os.makedirs(d, exist_ok=True)
    for k, v in b["files"].items():
        if os.path.basename(k) != k:
            raise SystemExit("bad file name " + k)
        open(os.path.join(d, k), "w", encoding="utf-8", newline="").write(v)
    return d


if __name__ == "__main__":
    a = argparse.ArgumentParser()
    a.add_argument("cmd", choices=["validate", "export", "import"])
    a.add_argument("path", help="layout dir (validate/export) or bundle .json (import)")
    a.add_argument("-o", help="export: output .json; import: rooms dir (default repo profile dir)")
    a = a.parse_args()
    if a.cmd == "export":
        export(a.path, a.o or a.path.rstrip("/\\") + ".json")
        sys.exit(0)
    d = imp(a.path, a.o or ROOMS) if a.cmd == "import" else a.path
    e = validate(d)
    print("\n".join(e) or "ok")
    sys.exit(bool(e))
