#!/usr/bin/env python3
"""Check every Preferences item id used in code is declared in
preferencesitemids.h and (unless popup-only) has a define(...) in
flarelib/preferences.cpp."""
import re, subprocess, sys, pathlib
root = pathlib.Path(__file__).resolve().parent.parent / "flare" / "sources"
ids_h = (root/"include/flare/preferencesitemids.h").read_text()
body = ids_h[ids_h.index("enum PreferencesItemId"):]
body = re.sub(r"//.*", "", body); body = re.sub(r"/\*.*?\*/", "", body, flags=re.S)
ids = set(re.findall(r"^\s*([A-Za-z_]\w*)\s*,", body, flags=re.M)) | {"PreferencesItemCount"}
cpp = (root/"flarelib/preferences.cpp").read_text()
defined = set(re.findall(r"\bdefine\(\s*(\w+)\s*,", cpp))
def strip(t):
    t = re.sub(r"/\*.*?\*/", "", t, flags=re.S)
    t = re.sub(r"//[^\n]*", "", t)
    return re.sub(r'"(?:\\.|[^"\\])*"', '""', t)

call = re.compile(r"\b(?:get(?:Bool|Int|String|Double)?Value|setValue|getUI(?:<[^>]*>)?|insertUI|m_onEditedFuncMap\.insert|m_controlIdMap\.key|setEnabled|insertDualUIs|setCallBack)\(\s*(\w+)")
pair = re.compile(r"\{\s*(\w+)\s*,\s*(?:tr\(|\{)")   # popup label / combo tables
popup = root/"flare/preferencespopup.cpp"
bad = 0
seen = set()
for p in root.rglob("*"):
    if p.suffix not in (".cpp", ".h") or "legacy" in str(p): continue
    t = strip(p.read_text(errors="ignore"))
    pats = [call] + ([pair] if p == popup else [])
    for rx in pats:
        for m in rx.finditer(t):
            n = m.group(1)
            if n in seen or n in ids: 
                if n in ids and n not in defined and n not in seen and n != "PreferencesItemCount":
                    print(f"NO define(): {n} ({p.relative_to(root)})"); bad += 1
                seen.add(n); continue
            if n in defined:
                continue
            # not an id: only a problem when it is used as a bare member argument
            # of Preferences accessors in the three preference files.
            if p.name in ("preferences.h", "preferencespopup.cpp") and rx is call and re.fullmatch(r"[A-Za-z]\w*", n) \
               and not n.startswith(("m_", "s_")) and n not in ("id", "item", "this", "key", "name"):
                print(f"NOT IN ENUM: {n} ({p.relative_to(root)})"); bad += 1
            elif rx is pair and n not in ("Stage", "color") and n[0].islower():
                print(f"POPUP TABLE ID NOT IN ENUM: {n} ({p.relative_to(root)})"); bad += 1
            seen.add(n)
for n in sorted(defined - ids):
    print(f"define() without enum id: {n}"); bad += 1
sys.exit(1 if bad else 0)
