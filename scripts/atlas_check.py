#!/usr/bin/env python3
"""Resolve LOGIC-ATLAS.md's own claims against the tree. Exit 1 on any HARD miss.

HARD (fail): cited file missing, subsystem section <-> directory mismatch,
pinned constant value drift, forbidden term present, snapshot unparseable.
SOFT (report only): identifier-shaped token absent from corpus, require-name
missing, snapshot staleness.

Usage: python scripts/atlas_check.py [path-to-atlas]
"""
import os
import re
import subprocess
import sys

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ATLAS = sys.argv[1] if len(sys.argv) > 1 else os.path.join(REPO, "LOGIC-ATLAS.md")
FACTS = os.path.join(REPO, "scripts", "atlas-facts.md")

PROJECT = "Flare"
CODE_DIRS = [
    "flare/sources/common/flash",
    "flare/sources/common/moho",
    "flare/sources/common/tgame",
    "flare/sources/remote",
    "tools/flash/next2flash",
]
SKIP_DIRS = {".git", "build", "build_local", "out", ".vs", "target",
             "node_modules", "__pycache__", ".venv", "dist", "vendor",
             "vcpkg", "thirdparty", ".claude"}
SRC_EXT = {".cpp", ".h", ".hpp", ".c", ".rs", ".py", ".ts", ".js", ".hlsl", ".hlsli"}
RESOLVE_EXT = SRC_EXT | {".txt", ".md", ".cmake", ".yml", ".yaml", ".json",
                         ".ini", ".xml", ".qrc", ".ts", ".nsi", ".gradle"}
IDENT_EXT = {".cpp", ".h", ".hpp", ".c", ".rs", ".py", ".ts", ".js"}
STOP_IDENTS = set("""the and for with from that this with struct class const void int
float double bool true false return if else for while auto mut let fn pub mod use impl
self Self None Some Ok Err String Vec Option Result include define namespace template
typename using new delete override final virtual public private protected static inline
extern const_cast sizeof import export default function var new type interface extends
number string boolean def return pass in is not and or lambda None True False""".split())

hard, soft = [], []


def fail(m):
    hard.append(m)


def note(m):
    soft.append(m)


def tree_files():
    out = []
    for dp, dn, fn in os.walk(REPO):
        dn[:] = [d for d in dn if d not in SKIP_DIRS]
        for f in fn:
            out.append(os.path.relpath(os.path.join(dp, f), REPO).replace("\\", "/"))
    return out


def strip_fences(t):
    return re.sub(r"```.*?```", "", t, flags=re.S)


def main():
    try:
        atlas = open(ATLAS, encoding="utf-8").read()
    except OSError:
        print("atlas_check: cannot read %s" % ATLAS)
        return 1
    body = strip_fences(atlas)
    files = tree_files()
    by_base, suffixes = {}, set()
    for f in files:
        by_base.setdefault(os.path.basename(f), []).append(f)
        suffixes.add(f)

    # --- cited files resolve -------------------------------------------
    exempt = parse_facts().get("exempt", set())
    for m in re.finditer(r"`([^`]+)`", body):
        tok = m.group(1).strip()
        if not tok or " " in tok.strip(".,;:"):
            continue
        if tok.startswith("<") or tok.startswith("../") or "://" in tok:
            continue
        if "<" in tok or ">" in tok:
            continue  # rule/placeholder path (<subsystem>, <name>), not a real one
        if re.fullmatch(r"\.[A-Za-z0-9]+", tok):
            continue  # bare extension label (.rs, .ini), not a filename
        if "%" in tok or "*" in tok:
            continue
        if any(e in tok for e in exempt):
            continue
        base = tok.split("/")[-1]
        if "." not in base:
            continue
        if "." + base.rsplit(".", 1)[-1] not in RESOLVE_EXT:
            continue
        if "/" in tok:
            if not (tok in suffixes or any(f.endswith("/" + tok) for f in files)):
                fail("cited file missing: %s" % tok)
        else:
            if base not in by_base:
                fail("cited file missing: %s" % tok)

    # --- sections both ways ----------------------------------------------
    sections = re.findall(r"^## (.+?) —", atlas, re.M)
    have_dirs = {d for d in CODE_DIRS
                 if os.path.isdir(os.path.join(REPO, d))}
    named = set()
    for s in sections:
        name = s.strip().split(" ")[0]
        for d in have_dirs:
            if d == name or d.endswith("/" + name) or os.path.basename(d) == name:
                named.add(d)
                break
    for d in sorted(have_dirs - named):
        fail("code dir with no atlas section: %s" % d)

    # --- facts: constants / require / forbid -------------------------------
    facts = parse_facts()
    corpus = build_corpus(files)
    for name, code_path in facts.get("constants", []):
        p = os.path.join(REPO, code_path)
        try:
            src = open(p, encoding="utf-8", errors="replace").read()
        except OSError:
            fail("pinned constant %s: code path missing: %s" % (name, code_path))
            continue
        vals = set()
        for m in re.finditer(r"\b%s\b(?:\[[^\]]*\])?\s*=\s*([^;\n]+)" % re.escape(name), src):
            for n in re.findall(r"-?\d+(?:\.\d+)?", m.group(1)):
                vals.add(n.rstrip("0").rstrip(".") if "." in n else n)
        if not vals:
            fail("pinned constant %s not found (facts stale or code moved)" % name)
            continue
        near = []
        for m in re.finditer(r"\b%s\b" % re.escape(name), atlas):
            win = atlas[m.end():m.end() + 80].split("\n")[0]
            hit = re.search(r"-?\d+(?:\.\d+)?", win)
            near.append(hit.group(0).rstrip("0").rstrip(".")
                        if hit and "." in hit.group(0) else (hit.group(0) if hit else None))
        if near and not any(v in near for v in vals):
            fail("constant %s is %s in %s but the atlas states %s near its mention"
                 % (name, "/".join(sorted(vals)), code_path,
                    "/".join(str(n) for n in near)))
    words = set(re.findall(r"[A-Za-z_][A-Za-z0-9_]*", atlas))
    for name in facts.get("forbid", set()):
        if name in words:
            fail("forbidden term present: %s" % name)
    for name in facts.get("require", set()):
        if not re.search(r"\b%s\b" % re.escape(name), atlas):
            note("soft: required term missing from atlas: %s" % name)

    # --- identifiers (soft) ---------------------------------------------------
    for m in re.finditer(r"`([^`]+)`", body):
        tok = m.group(1).strip()
        if "/" in tok or "." in tok or " " in tok:
            continue
        if any(c in tok for c in "*=<>") or re.match(r"^0x[0-9A-F]+$", tok):
            continue  # wildcard, assignment, or error code — a pattern, not an id
        if tok.endswith("_"):
            continue  # doc placeholder (pane_K), not a concrete identifier
        shaped = ("_" in tok or re.match(r"k[A-Z]", tok) or
                  re.search(r"[a-z][A-Z]", tok) or "::" in tok)
        if not shaped:
            continue
        # Namespace-qualified names are tokenised apart in source; check each part.
        parts = [p for p in tok.split("::") if p]
        if parts and all(p not in corpus for p in parts):
            note("soft: identifier not found in tree: %s" % tok)

    # --- snapshot --------------------------------------------------------------
    m = re.search(r"Snapshot:\s*Flare\s*@\s*([0-9a-f]{7,40})", atlas)
    if not m:
        fail("snapshot line missing or malformed (want 'Snapshot: Flare @ <sha>')")
    else:
        try:
            r = subprocess.run(["git", "log", "--format=%H", "-1", "--"]
                               + CODE_DIRS, capture_output=True, text=True, cwd=REPO)
            newest = (r.stdout.strip().split("\n") or [""])[0]
            if newest:
                c = subprocess.run(["git", "rev-list", "--count",
                                    m.group(1) + ".." + newest],
                                   capture_output=True, text=True, cwd=REPO)
                n = int((c.stdout or "0").strip() or 0)
                # The commit that moves subsystem code also moves the snapshot, so
                # allow a small batch before nagging; the atlas is a map, not a diff.
                if n >= 5:
                    note("soft: snapshot %s is %d code commits behind" % (m.group(1)[:9], n))
        except Exception:
            pass

    for s in soft:
        print(s)
    for h in hard:
        print("HARD: " + h)
    print("atlas_check: %d hard, %d soft" % (len(hard), len(soft)))
    return 1 if hard else 0


def parse_facts():
    out = {"constants": [], "require": set(), "forbid": set(), "exempt": set()}
    try:
        text = open(FACTS, encoding="utf-8").read()
    except OSError:
        return out
    sec = None
    for line in text.split("\n"):
        s = line.strip()
        low = s.lower()
        if low.startswith("## constants"):
            sec = "constants"
            continue
        if low.startswith("## require"):
            sec = "require"
            continue
        if low.startswith("## forbid"):
            sec = "forbid"
            continue
        if low.startswith("## exempt"):
            sec = "exempt"
            continue
        if not s or s.startswith("#") or sec is None:
            continue
        if sec == "constants":
            parts = s.split()
            if len(parts) >= 2:
                out["constants"].append((parts[0], parts[1]))
        else:
            out[sec].add(s.split()[0])
    return out


def build_corpus(files):
    corpus = set()
    for f in files:
        _, ext = os.path.splitext(f)
        if ext not in IDENT_EXT:
            continue
        try:
            src = open(os.path.join(REPO, f), encoding="utf-8", errors="replace").read()
        except OSError:
            continue
        corpus.update(re.findall(r"[A-Za-z_][A-Za-z0-9_]*", src))
    return corpus


if __name__ == "__main__":
    sys.exit(main())
