#!/usr/bin/env python3
"""Layout-profile coverage audit.  Pure stdlib, no Qt, deterministic.

A *profile* is a directory under stuff/profiles/layouts/rooms/<Name>/ (see
docs/LAYOUT_PROFILES.md).  Every profile must expose ALL UI options, whatever its
philosophy.  This script derives "all UI options" from the C++ sources and reports,
per profile, what is missing:

  commands  every MI_* command the app registers (MenuItemHandler / create*Action /
            OpenFloatingPanel / define ...) must be reachable from the menubar of
            EVERY room of the profile (roomN_menubar.xml if present, else
            menubar_template.xml -- the same resolution makePrivate() uses).
            Profiles hide what their philosophy hides inside the "All Commands" menu
            instead of omitting it.
  panels    every TPanelFactory panel type must be openable (its MI_Open* command is
            a command, so it is covered above) and every panel name used by a room
            must be a registered panel type.
  rooms     layouts.txt -> files exist; hierarchy indices == pane_<i>\\name entries.
  shortcuts every preset in shortcuts/*.ini must define every key of defflare.ini.
  toolbars  settings/commandbar.xml, xsheettoolbar.xml and settings/presets/*.xml
            must only reference registered commands; toolbars/maintoolbar.xml must
            list every registered tool.

    python scripts/check_layout_profiles.py            # report, exit 1 on any gap
    python scripts/check_layout_profiles.py -v         # also list every missing id
    python scripts/check_layout_profiles.py --json     # machine readable
    python scripts/check_layout_profiles.py --sync-all-commands
        # rewrite the generated <menu title="All Commands"> block in every menubar
        # so it lists exactly the registered commands the rest of that menubar lacks.

Exit status: 0 = every profile at 100 % and structurally valid, 1 otherwise.
"""
from __future__ import annotations

import argparse
import json
import re
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
SRC = REPO / "flare" / "sources"
LAYOUTS = REPO / "stuff" / "profiles" / "layouts"
ROOMS = LAYOUTS / "rooms"
SHORTCUTS = LAYOUTS / "shortcuts"

ALL_MENU_TITLE = "All Commands"
GEN_BEGIN = "<!-- BEGIN GENERATED All Commands: scripts/check_layout_profiles.py sync-all-commands. Do not edit by hand. -->"
GEN_END = "<!-- END GENERATED All Commands -->"

# ids that are registered by a code pattern but are not user-facing commands
NOT_COMMANDS = {
    "MI_OpenMyWidget",  # sample in floatingpanelcommand docs
}

# Ids that stock OpenToonz menubars carry but this tree no longer registers.  The loader
# silently skips unknown ids inside a <menu>, so they are harmless; they are reported
# as warnings (kept so the pristine OpenToonz profile stays byte-for-byte upstream).
KNOWN_STALE_COMMANDS = {
    "MI_SafeArea", "MI_TestAnimation", "MI_Export",
    "MI_OpenAlignmentPanel", "MI_OpenBrushPresetPanel", "MI_OpenToolPropertiesPanel", "MI_Quicklink",
}
# Profiles that are preserved exactly as shipped before this audit.  Their room files
# contradict themselves (pane geometry vs hierarchy string); that is reported as a warning
# and left alone on purpose -- see docs/LAYOUT_PROFILES.md "Preserved profiles".
PRESERVED_AS_SHIPPED = {"AnimateLegacy", "FlipaClip"}
# Panel names that rooms use but that have no TPanelFactory in this tree (regression in
# flare/tpanels.cpp: FxBrowserPanel/FxBrowserFactory were dropped, the Open command was
# kept).  Rooms degrade to the "panel not supported" placeholder.  Warning, not error.
KNOWN_MISSING_PANELS = {"FxBrowser"}

# --------------------------------------------------------------------------- registry

_REG = [
    # create*Action / createToggle / createRightClickMenuAction ..., define(), handlers;
    # the id may be a macro (MI_X) or a string literal ("MI_X").
    re.compile(r'\b(?:create\w*|define|setCommandHandler|MenuItemHandler)\s*\(\s*"?(MI_\w+)"?'),
    re.compile(r'\b\w*(?:Handler|FloatingPanel|Command)\s*(?:<[^>;]*>)?\s+\w+\s*\(\s*"?(MI_\w+)"?'),
]
_PANEL_FACTORY = re.compile(r'TPanelFactory\s*\(\s*"(\w+)"\s*\)')
_PANEL_CMD = re.compile(r'OpenFloatingPanel\s+\w+\s*\(\s*"?(MI_\w+)"?\s*,\s*"(\w+)"', re.S)
_PANEL_CMD2 = re.compile(r'^\s*(MI_\w+)\s*,\s*"(\w+)"\s*,', re.M)  # continuation lines
_TOOL = re.compile(r'#define\s+(T_\w+)\s+"')


def _source_files():
    for p in sorted(SRC.rglob("*")):
        if p.suffix in (".cpp", ".h", ".hpp") and p.is_file():
            yield p


_COMMENT = re.compile(r"//[^\n]*|/\*.*?\*/", re.S)


def _strip_comments(t: str) -> str:
    """Drop // and /* */ comments (commented-out registrations are not registrations).
    String literals containing // (URLs) are only an issue for ids, which have none."""
    return _COMMENT.sub(" ", t)


def read_registry() -> dict:
    cmds: set[str] = set()
    factories: set[str] = set()
    panel_cmd: dict[str, str] = {}
    tools: set[str] = set()
    tool_use: set[str] = set()
    for p in _source_files():
        t = _strip_comments(p.read_text(encoding="utf-8", errors="ignore"))
        for rx in _REG:
            cmds.update(rx.findall(t))
        factories.update(_PANEL_FACTORY.findall(t))
        for c, panel in _PANEL_CMD.findall(t):
            panel_cmd.setdefault(panel, c)
        if "OpenFloatingPanel" in t:
            for c, panel in _PANEL_CMD2.findall(t):
                panel_cmd.setdefault(panel, c)
        if p.name == "toolcommandids.h":
            tools.update(_TOOL.findall(t))
        elif p.suffix == ".cpp":
            tool_use.update(re.findall(r"\bT_[A-Z]\w+", t))
    cmds |= set(panel_cmd.values())
    cmds -= NOT_COMMANDS
    tools &= tool_use  # a tool id nobody instantiates is not a UI option
    return {
        "commands": sorted(cmds),
        "panels": sorted(factories),
        "panel_command": {k: v for k, v in sorted(panel_cmd.items())},
        "tools": sorted(tools),
    }


# --------------------------------------------------------------------------- parsing


def menubar_ids(path: Path) -> tuple[set[str], list[str]]:
    errs: list[str] = []
    try:
        root = ET.parse(path).getroot()
    except ET.ParseError as e:
        return set(), [f"{path.name}: XML parse error: {e}"]
    if root.tag != "menubar":
        errs.append(f"{path.name}: root element is <{root.tag}>, expected <menubar>")
    ids = set()
    for el in root.iter():
        if el.tag in ("command", "command_debug"):
            cid = (el.text or "").strip()
            if cid:
                ids.add(cid)
        elif el.tag not in ("menubar", "menu", "separator"):
            errs.append(f"{path.name}: unexpected element <{el.tag}> (loader skips it)")
    return ids, errs


def parse_room(path: Path) -> dict:
    t = path.read_text(encoding="utf-8", errors="ignore")
    panes = {int(i): n.strip() for i, n in re.findall(r"^pane_(\d+)\\name=(.*)$", t, re.M)}
    rects = {int(i): tuple(int(v) for v in r.split())
             for i, r in re.findall(r"^pane_(\d+)\\geometry=@Rect\(([-\d ]+)\)", t, re.M)}
    m = re.search(r'^hierarchy="([^"]*)"', t, re.M)
    toks = m.group(1).split() if m else []
    hier = {int(x) for x in toks[2:] if x.isdigit()} if m else None
    nm = re.search(r"^name=(.*)$", t, re.M)
    return {"panes": panes, "rects": rects, "tokens": toks, "hierarchy": hier,
            "name": nm.group(1).strip() if nm else None}


TOL = 16  # px; separators between docked panes are a few px wide


def hierarchy_tree(tokens: list[str]):
    """tokens = ['-1','1','[', '7','2','[',...,']',...] -> nested lists of ints.
    First token = maximised pane (-1 = none), second = orientation of the root
    region (1 = children stacked top-to-bottom); orientation alternates per depth
    (flareqt/docklayout.cpp restoreState / writeRegion)."""
    pos = 2

    def region():
        nonlocal pos
        t = tokens[pos]
        pos += 1
        if t == "[":
            kids = []
            while tokens[pos] != "]":
                kids.append(region())
            pos += 1
            return kids
        return int(t)

    return region(), tokens[1] == "1"


def _bbox(node, rects):
    if isinstance(node, int):
        x, y, w, h = rects[node]
        return x, y, x + w, y + h
    bs = [_bbox(c, rects) for c in node]
    return min(b[0] for b in bs), min(b[1] for b in bs), max(b[2] for b in bs), max(b[3] for b in bs)


def geometry_problems(info: dict) -> list[str]:
    """Pane geometry must describe the same arrangement as the hierarchy, otherwise the
    dock layout reconstructs the room from contradictory data and panes land in the
    wrong place (e.g. the timeline docked to the right of the viewer)."""
    out = []
    toks = info["tokens"]
    origins = [r[:2] for r in info["rects"].values()]
    if len(set(origins)) < len(origins):
        return []  # stock OpenToonz built-in rooms ship placeholder geometry (all 0 0); the dock redistributes
    if len(toks) < 3 or set(info["panes"]) - set(info["rects"]):
        return ["missing pane geometry"] if info["panes"] else []
    try:
        tree, vertical = hierarchy_tree(toks)
    except (IndexError, ValueError):
        return ["hierarchy string is malformed"]

    def walk(node, vert, path):
        if isinstance(node, int):
            return
        boxes = [_bbox(c, info["rects"]) for c in node]
        for i in range(len(node) - 1):
            a, b = boxes[i], boxes[i + 1]
            lo, hi = (a[3], b[1]) if vert else (a[2], b[0])
            if hi < lo - TOL:
                out.append(f"region {path}: child {i + 1} overlaps/precedes child {i} along the "
                           f"{'vertical' if vert else 'horizontal'} axis ({lo} > {hi})")
            c0, c1 = (0, 2) if vert else (1, 3)
            if abs(a[c0] - b[c0]) > TOL or abs(a[c1] - b[c1]) > TOL:
                out.append(f"region {path}: children {i} and {i + 1} are not aligned on the cross axis")
        for i, c in enumerate(node):
            walk(c, not vert, f"{path}/{i}")

    walk(tree, vertical, "root")
    return out


def read_ini_keys(path: Path) -> set[str]:
    return set(re.findall(r"^([A-Za-z_0-9][^=\n]*)=", path.read_text(encoding="utf-8"), re.M))


# --------------------------------------------------------------------------- audit


def effective_menubars(pdir: Path, rooms: list[str]) -> dict[str, Path]:
    out = {}
    for r in rooms:
        own = pdir / (Path(r).stem + "_menubar.xml")
        out[r] = own if own.exists() else pdir / "menubar_template.xml"
    return out


def audit_profile(pdir: Path, reg: dict) -> dict:
    errs: list[str] = []
    warns: list[str] = []
    res: dict = {"profile": pdir.name, "errors": errs, "warnings": warns}
    lt = pdir / "layouts.txt"
    rooms = lt.read_text().split() if lt.exists() else []
    if not rooms:
        errs.append("layouts.txt missing or empty")
    if len(set(rooms)) != len(rooms):
        errs.append("layouts.txt lists a room twice")
    res["rooms"] = len(rooms)
    used_panels: set[str] = set()
    room_names = []
    for r in rooms:
        rp = pdir / r
        if not rp.exists():
            errs.append(f"layouts.txt lists missing {r}")
            continue
        info = parse_room(rp)
        room_names.append(info["name"])
        if info["name"] is None:
            errs.append(f"{r}: no name=")
        if info["hierarchy"] is None:
            errs.append(f"{r}: no hierarchy=")
        elif info["hierarchy"] != set(info["panes"]):
            errs.append(f"{r}: hierarchy {sorted(info['hierarchy'])} != panes {sorted(info['panes'])}")
        for g in geometry_problems(info)[:3]:
            tgt = warns if pdir.name in PRESERVED_AS_SHIPPED else errs
            tgt.append(f"{r}: geometry/hierarchy mismatch: {g}")
        for i, n in sorted(info["panes"].items()):
            used_panels.add(n)
            if n not in reg["panels"]:
                if n in KNOWN_MISSING_PANELS:
                    warns.append(f"{r}: pane_{i} uses '{n}', which has no TPanelFactory (known gap)")
                else:
                    errs.append(f"{r}: pane_{i} uses unregistered panel '{n}'")
    if len(set(room_names)) != len(room_names):
        errs.append("duplicate room names")
    for f in sorted(pdir.glob("room*.ini")):
        if f.name not in rooms:
            errs.append(f"{f.name} exists but is not listed in layouts.txt (orphan)")
    res["room_names"] = room_names
    res["panels_in_rooms"] = sorted(used_panels)

    cmdset = set(reg["commands"]) | set(reg["tools"])
    mb = effective_menubars(pdir, rooms)
    per_room = {}
    union: set[str] = set()
    seen_files: set[Path] = set()
    for r, path in mb.items():
        if path in seen_files:
            per_room[r] = per_room[next(k for k, v in mb.items() if v == path)]
            continue
        seen_files.add(path)
        if not path.exists():
            errs.append(f"{r}: menubar {path.name} missing")
            continue
        ids, e = menubar_ids(path)
        errs += e
        for u in sorted(ids - cmdset):
            known = u in KNOWN_STALE_COMMANDS
            msg = f"{path.name}: command '{u}' is not registered by the app" + (" (stale upstream id)" if known else "")
            tgt = warns if known else errs
            if msg not in tgt:
                tgt.append(msg)
        per_room[r] = path.name, sorted(cmdset - ids)
        union |= ids
    res["menubars"] = {r: {"file": n, "missing": m, "coverage": round(100 * (1 - len(m) / len(cmdset)), 1)}
                       for r, (n, m) in per_room.items()}
    worst = min((v["coverage"] for v in res["menubars"].values()), default=0.0)
    res["command_coverage_min"] = worst
    res["command_coverage_union"] = round(100 * len(union & cmdset) / len(cmdset), 1)
    # panels: each type needs an open command in every effective menubar
    pc = reg["panel_command"]
    nocmd = sorted(p for p in reg["panels"] if p not in pc)
    res["panels_without_command"] = nocmd
    miss_panels = sorted({p for p, c in pc.items()
                          if any(c in v["missing"] for v in res["menubars"].values())})
    res["panels_unreachable"] = miss_panels
    return res


def audit_shared(reg: dict) -> dict:
    errs: list[str] = []
    warns: list[str] = []
    cmdset = set(reg["commands"]) | set(reg["tools"])
    base = read_ini_keys(SHORTCUTS / "defflare.ini")
    presets = {}
    for f in sorted(SHORTCUTS.glob("*.ini")):
        keys = read_ini_keys(f)
        miss = sorted(base - keys)
        presets[f.name] = miss
        if miss:
            errs.append(f"shortcuts/{f.name}: {len(miss)} keys of defflare.ini missing")
    for k in sorted(base - cmdset):
        if k.startswith(("MI_", "T_")):
            warns.append(f"shortcuts/defflare.ini: '{k}' has no registered command/tool (stale key)")
    xmls = [LAYOUTS / "settings" / "commandbar.xml", LAYOUTS / "settings" / "xsheettoolbar.xml"]
    xmls += sorted((LAYOUTS / "settings" / "presets").glob("*.xml"))
    for f in xmls:
        try:
            root = ET.parse(f).getroot()
        except (ET.ParseError, OSError) as e:
            errs.append(f"{f.relative_to(LAYOUTS)}: {e}")
            continue
        for el in root.iter("command"):
            cid = (el.text or "").strip()
            if cid and cid not in cmdset:
                (warns if cid in KNOWN_STALE_COMMANDS else errs).append(f"{f.relative_to(LAYOUTS)}: command '{cid}' is not registered")
    mt = LAYOUTS / "toolbars" / "maintoolbar.xml"
    tb = set(re.findall(r"\bT_\w+", mt.read_text(encoding="utf-8")))
    miss_tools = sorted(set(reg["tools"]) - tb)
    for t in sorted(tb - set(reg["tools"])):
        errs.append(f"toolbars/maintoolbar.xml: '{t}' is not a registered tool")
    if miss_tools:
        errs.append(f"toolbars/maintoolbar.xml: {len(miss_tools)} registered tools missing")
    return {"errors": errs, "warnings": warns, "shortcut_presets_missing": presets, "tools_missing": miss_tools}


# --------------------------------------------------------------------------- sync

_CATS = [  # (menu title, regex on the id without its MI_ prefix); first match wins
    ("Tools", re.compile(r"^T_")),
    ("Help and System", re.compile(r"About|Manual|Update|Discord|Website|Forum|Bug|WhatsNew|FlashGuide|Preferences|ShortcutPopup")),
    ("Windows and Panels", re.compile(r"^Open(?!Child|Recent)|Docking|ResetRoomLayout|CustomPanel|StartupPopup|RunScript|MaximizePanel|FullScreenWindow|SeeThrough|ZoomInAndFit|ZoomOutAndFit")),
    ("Guided Drawing and Tweening", re.compile(r"GuideStroke|TweenGuide|VectorGuidedDrawing|SelectGuides|SelectBothGuide")),
    ("Tool Modes", re.compile(
        r"^(Eraser|Fill(?!EmptyCell)|Tape|PaintBrush|PickStyle|RGBPicker|Selection|Geometric|Edit(All|Center|Position|Rotation|Scale|Shear|Shift|NextMode)"
        r"|Type(Bold|BoldOblique|Oblique|Regular|NextStyle)|PlasticNextMode|PlasticPaintRigid|SkeletonNextMode|NoShift|ResetShift|ShowShiftOrigin|ShiftTrace"
        r"|ZeroThick|Autocenter|RemoveEndpoints|ApplyMatchLines|DeleteMatchLines|DeleteInk|CursorOutline)")),
    ("Stop Motion, Scan and Cleanup", re.compile(r"StopMotion|Scan|Cropbox|Cleanup|Tracking|PencilTest|Camera(Stage|Test)|DefineScanner|RevertToCleanedUp")),
    ("Scene, Rigging and Sound", re.compile(r"CameraSettings|SceneSettings|Child|Collapse|Resequence|XSheetToolbar|LipSync|Audio|SoundTrack|Plastic|Skeleton")),
    ("Columns", re.compile(r"Column|SwapEnabled")),
    ("Keyframes", re.compile(r"Keyframe|Interpolation|Acceleration|Deceleration|ConstantSpeed|ResetInterpolation|KeysIn")),
    ("Frames and Timing", re.compile(
        r"Step|Each\d|Frame|Tagged|[Rr]enumber|AutoInput|DrawingSub|Reframe|TimeIndicator|Blank|^Play|Pause|Loop|ShortPlay|Rollup|Rolldown|Swing|Reverse|Random|Dup\b|Increment|FillEmptyCell|AutoFill|ClearTags|TimeStretch|Drawing\b")),
    ("Files and Project", re.compile(r"Import|Export|Print|SavePreset|ShowFolderContents|Revert|Recent|Load|Save|Collect|ConvertFiles|ConvertTZP|ReplaceParentDirectory|Quit|NewScene|NewProject|ProjectSettings|Cache")),
    ("Preview and Render", re.compile(r"Preview|Render|Output|Compare|Snapshot|Freeze|Regenerate|Fx|Histogram")),
    ("Adjust and Convert", re.compile(r"Adjust|Brightness|Convert|Rasterize|SwfRoundTrip|MergeCmapped|SeparateColors|Merge|Binarize|Antialias|Lines")),
    ("Palettes and Styles", re.compile(r"Style|Palette|Plt|Color|Ink1|Studio|^Link|ToggleLink")),
    ("Levels", re.compile(r"Level|Clone|Expose|Duplicate|Remove|Canvas|FileInfo|NewMeta|MetaLevel|EditInPlace")),
    ("Edit and Selection", re.compile(r"Select|Delete|Clear|Cut|Copy|Paste|Undo|Redo|[Gg]roup|Insert|Bring|Send")),
    ("View and Display", re.compile(r"View|Check|Channel|Greyscale|Opacity|Layout|Touch|Breadcrumb|Indicator|Refresh|IOnly|Onion|Field|Safe")),
]


def categorize(cid: str) -> str:
    name = cid if cid.startswith("T_") else cid[3:]
    for title, rx in _CATS:
        if rx.search(name):
            return title
    return "Other"


def render_all_commands(ids: list[str]) -> str:
    groups: dict[str, list[str]] = {}
    for i in ids:
        groups.setdefault(categorize(i), []).append(i)
    order = [t for t, _ in _CATS] + ["Other"]  # fixed order -> stable output
    out = [f"    {GEN_BEGIN}", f'    <menu title="{ALL_MENU_TITLE}">']
    for title in order:
        if title not in groups:
            continue
        out.append(f'        <menu title="{title}">')
        out += [f"            <command>{i}</command>" for i in sorted(groups[title])]
        out.append("        </menu>")
    out += ["    </menu>", f"    {GEN_END}"]
    return "\n".join(out) + "\n"


def sync_menubar(path: Path, reg: dict) -> bool:
    text = path.read_text(encoding="utf-8")
    block = re.compile(r"[ \t]*" + re.escape(GEN_BEGIN) + r".*?" + re.escape(GEN_END) + r"\n?", re.S)
    base = block.sub("", text)
    try:
        ET.fromstring(base)
    except ET.ParseError as e:
        raise SystemExit(f"{path}: cannot sync, not well-formed: {e}")
    tmp = path.with_suffix(".tmp")
    tmp.write_text(base, encoding="utf-8")
    have, _ = menubar_ids(tmp)
    tmp.unlink()
    missing = sorted((set(reg["commands"]) | set(reg["tools"])) - have)
    new = base
    if missing:
        idx = base.rfind("</menubar>")
        if idx < 0:
            raise SystemExit(f"{path}: no </menubar>")
        head = base[:idx].rstrip("\n") + "\n"
        new = head + render_all_commands(missing) + "</menubar>\n"
    if new != text:
        path.write_text(new, encoding="utf-8", newline="\n")
        return True
    return False


def all_menubar_files() -> list[Path]:
    files = []
    for pdir in sorted(p for p in ROOMS.iterdir() if p.is_dir()):
        files += sorted(pdir.glob("menubar_template.xml")) + sorted(pdir.glob("room*_menubar.xml"))
    return files


# --------------------------------------------------------------------------- main


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("-v", "--verbose", action="store_true", help="list every missing id")
    ap.add_argument("--json", action="store_true")
    ap.add_argument("--profile", action="append", help="only this profile (repeatable)")
    ap.add_argument("--sync-all-commands", action="store_true",
                    help="regenerate the 'All Commands' menu in every menubar file")
    a = ap.parse_args(argv)

    reg = read_registry()
    if len(reg["commands"]) < 100 or not reg["panels"]:
        print("registry extraction failed (sources moved?)", file=sys.stderr)
        return 2
    if a.sync_all_commands:
        for f in all_menubar_files():
            if sync_menubar(f, reg):
                print("updated", f.relative_to(REPO).as_posix())
        return 0

    profiles = [p for p in sorted(ROOMS.iterdir()) if p.is_dir() and (not a.profile or p.name in a.profile)]
    results = [audit_profile(p, reg) for p in profiles]
    shared = audit_shared(reg)
    bad = bool(shared["errors"]) or any(
        r["errors"] or r["command_coverage_min"] < 100.0 or r["panels_unreachable"] for r in results)
    if a.json:
        print(json.dumps({"registry": {k: (len(v) if isinstance(v, (list, dict)) else v) for k, v in reg.items()},
                          "profiles": results, "shared": shared}, indent=1, sort_keys=True))
        return 1 if bad else 0

    print(f"registry: {len(reg['commands'])} commands, {len(reg['panels'])} panel types, {len(reg['tools'])} tools")
    print(f"{'profile':<14}{'rooms':>6}{'menubars min%':>15}{'union%':>9}{'panels missing':>16}  errors")
    for r in results:
        print(f"{r['profile']:<14}{r['rooms']:>6}{r['command_coverage_min']:>15.1f}{r['command_coverage_union']:>9.1f}"
              f"{len(r['panels_unreachable']):>16}  {len(r['errors'])}")
    for r in results:
        if r["errors"] or r["command_coverage_min"] < 100 or r["panels_unreachable"]:
            print(f"\n[{r['profile']}]")
            for e in r["errors"]:
                print("  ERROR", e)
            for room, v in sorted(r["menubars"].items()):
                if v["missing"]:
                    sample = v["missing"] if a.verbose else v["missing"][:6]
                    more = "" if a.verbose or len(v["missing"]) <= 6 else f" ... +{len(v['missing']) - 6}"
                    print(f"  {room} ({v['file']}): {len(v['missing'])} commands missing: {' '.join(sample)}{more}")
            if r["panels_unreachable"]:
                print("  panels with no Windows-menu command:", " ".join(r["panels_unreachable"]))
        if r["panels_without_command"] and a.verbose:
            print(f"  [{r['profile']}] panel types with no registered open command:", " ".join(r["panels_without_command"]))
    for r in results:
        for w in r["warnings"]:
            print(f"WARN [{r['profile']}] {w}")
    for w in shared["warnings"]:
        print("WARN [shared]", w)
    if shared["errors"]:
        print("\n[shared]")
        for e in shared["errors"]:
            print("  ERROR", e)
    print("\nRESULT:", "FAIL" if bad else "OK")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
