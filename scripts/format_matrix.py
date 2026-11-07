#!/usr/bin/env python3
"""Run `flare` over a stratified sample of real sample files, one row per file.

Stdlib only. For each file: `flare import SRC OUT.flare`, then `flare info OUT.flare`, and for an
imported project `flare export OUT.flare OUT.png`. Every call has a timeout. The sources are read
in place and never copied into the repository; the work directory holds only the outputs.

Result classes:
  ok       imported, info and the still export succeeded, nothing reported as not carried over
  partial  imported, but the importer listed things it did not carry over
  refused  a typed error (exit 1 with a message); the file is not one Flare reads
  crash    a panic, a signal, or a timeout

Usage:
  python3 scripts/format_matrix.py --flare BIN --work DIR --out-md FILE [--per-type N] [--seed S]
  python3 scripts/format_matrix.py --flare BIN --work DIR --only-type swf --per-type 50

Input roots are listed in ROOTS; add a root when a new sample source is downloaded.
"""

from __future__ import annotations

import argparse
import json
import os
import random
import re
import subprocess
import sys
from collections import defaultdict
from pathlib import Path

HOME = Path.home()

# Licence labels are what the sample's own licence file says. Nothing here is committed.
ROOTS: list[tuple[str, Path, str]] = [
    ("ruffle-tests", Path("/home/user/Flare/third_party/ruffle"), "Ruffle test corpus, MIT OR Apache-2.0"),
    ("flash-samples", Path("/home/user/ref/flash-samples"), "mixed GitHub samples, see each repo"),
    ("flipaclip", Path("/home/user/ref/flipaclip"), "BFDI FlipaClip zip, see its source page"),
    ("moho-samples", Path("/home/user/ref/moho-samples"), "Moho samples, see the folder"),
    ("godot-moho", Path("/home/user/ref/godot_moho_importer"), "godot_moho_importer example, MIT"),
    ("flipnote-samples", Path("/home/user/ref/flipnote-samples"), "Flipnote samples, see the folder"),
    ("flipnote.js", Path("/home/user/ref/flipnote.js"), "flipnote.js test samples, see the repo"),
    ("opentoonz-lib", Path("/home/user/ref/opentoonz"), "OpenToonz repo (GPL-3), library samples"),
    ("tahoma2d-lib", Path("/home/user/ref/tahoma2d"), "Tahoma2D repo (GPL-3), library samples"),
    ("opentoonz-sample", Path("/home/user/ref/tnz-samples/opentoonz_sample"), "OpenToonz sample scenes, CC BY-NC 4.0"),
    ("wick-examples", Path("/home/user/ref/wick-editor"), "Wick Editor examples, see the repo"),
    ("wick-samples", Path("/home/user/ref/wick-samples"), "Wick samples, see the folder"),
]

# Matrix types, in report order: (type label, extensions it covers, expected verdict, name rule).
# `expected` is the behaviour the type must show: "open" (Flare imports it) or "refuse" (a clear
# typed message). The name rule narrows a type by file name: "only:NAME" keeps exactly that name,
# "not:NAME" drops it, "" keeps everything. An XFL folder's DOMDocument.xml is its own row.
TYPES: list[tuple[str, tuple[str, ...], str, str]] = [
    ("fla", ("fla",), "open", ""),
    ("xfl", ("xfl",), "open", ""),
    ("xfl-document", ("xml",), "open", "only:DOMDocument.xml"),
    ("xml", ("xml",), "refuse", "not:DOMDocument.xml"),
    ("swf", ("swf",), "open", ""),
    ("spl", ("spl",), "open", ""),
    ("swc", ("swc",), "refuse", ""),
    ("as", ("as",), "refuse", ""),
    ("as2", ("as2",), "refuse", ""),
    ("as3", ("as3",), "refuse", ""),
    ("jsfl", ("jsfl",), "refuse", ""),
    ("sol", ("sol",), "refuse", ""),
    ("flv", ("flv",), "refuse", ""),
    ("moho", ("moho", "mohoproj"), "open", ""),
    ("anme", ("anme", "anime", "animeproj"), "open", ""),
    ("toonz", ("tnz", "tpl", "pli", "tlv", "plt"), "open", ""),
    ("tif", ("tif",), "refuse", ""),
    ("tnx", ("tnx",), "refuse", ""),
    ("flipaclip", ("fc",), "open", ""),
    ("wick", ("wick",), "open", ""),
    ("flipnote", ("ppm", "kwz", "kwc"), "open", ""),
    ("png", ("png",), "open", ""),
    ("gif", ("gif",), "open", ""),
    ("jpg", ("jpg", "jpeg"), "open", ""),
    ("svg", ("svg",), "open", ""),
]

# A matrix type with no sample on disk still gets a row: the report says so instead of hiding it.
NOT_IN_CORPUS_NOTE = "no sample found on disk or on GitHub search; see open issues"

PARTIAL_RE = re.compile(r"not carried over|unsupported|skipped|not imported", re.IGNORECASE)
PANIC_RE = re.compile(r"panicked|stack overflow|SIGSEGV|SIGABRT", re.IGNORECASE)


def name_allowed(name: str, rule: str) -> bool:
    """The name rule from TYPES: "only:NAME", "not:NAME" or "" (anything)."""
    if rule.startswith("only:"):
        return name.lower() == rule[5:].lower()
    if rule.startswith("not:"):
        return name.lower() != rule[4:].lower()
    return True


def collect(extensions: tuple[str, ...], rule: str = "") -> dict[str, list[Path]]:
    """Files under each root whose extension matches and whose name the rule allows, grouped by root."""
    wanted = {f".{e}" for e in extensions}
    found: dict[str, list[Path]] = defaultdict(list)
    for label, root, _licence in ROOTS:
        if not root.is_dir():
            continue
        for dirpath, dirnames, filenames in os.walk(root, followlinks=False):
            dirnames[:] = [d for d in dirnames if d != ".git"]
            for name in filenames:
                if Path(name).suffix.lower() in wanted and name_allowed(name, rule):
                    found[label].append(Path(dirpath) / name)
    return found


def stratified_sample(found: dict[str, list[Path]], per_type: int, rng: random.Random) -> list[tuple[str, Path]]:
    """Round-robin over the roots, so one large corpus does not hide the small ones."""
    pools = {label: sorted(paths) for label, paths in found.items()}
    for paths in pools.values():
        rng.shuffle(paths)
    picked: list[tuple[str, Path]] = []
    labels = sorted(pools)
    while len(picked) < per_type and any(pools.values()):
        for label in labels:
            if len(picked) >= per_type:
                break
            if pools[label]:
                picked.append((label, pools[label].pop()))
    return picked


def licence_of(label: str) -> str:
    for name, _root, licence in ROOTS:
        if name == label:
            return licence
    return "unknown"


def run(cmd: list[str], timeout: float) -> tuple[str, str, int | None]:
    """(verdict-class, combined output, returncode). A timeout or a signal is a crash."""
    env = dict(os.environ, RUST_BACKTRACE="0")
    try:
        proc = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout, errors="replace", env=env)
    except subprocess.TimeoutExpired as e:
        out = (e.stdout or b"").decode(errors="replace") if isinstance(e.stdout, bytes) else (e.stdout or "")
        return "timeout", out, None
    text = proc.stdout + proc.stderr
    if proc.returncode < 0 or PANIC_RE.search(text) or proc.returncode == 101:
        return "panic", text, proc.returncode
    return ("ok" if proc.returncode == 0 else "error"), text, proc.returncode


def first_line(text: str, limit: int = 160) -> str:
    for line in text.splitlines():
        line = line.strip()
        if line:
            return line[:limit].replace("|", "/")
    return ""


def cause_line(text: str, limit: int = 200) -> str:
    """The root cause of an error chain: the line after "Caused by:", else the first line."""
    lines = [line.strip() for line in text.splitlines() if line.strip()]
    for i, line in enumerate(lines):
        if line.startswith("Caused by:") and i + 1 < len(lines):
            return lines[i + 1][:limit].replace("|", "/")
    return first_line(text, limit)


def check_one(flare: str, src: Path, work: Path, timeout: float) -> tuple[str, str]:
    """(result, note) for one source file."""
    out_flare = work / "out.flare"
    out_png = work / "out.png"
    for stale in (out_flare, out_png):
        if stale.exists():
            stale.unlink()
    verdict, import_text, code = run([flare, "import", str(src), str(out_flare)], timeout)
    if verdict == "timeout":
        return "crash", f"import timed out after {timeout:.0f}s"
    if verdict == "panic":
        return "crash", f"import panicked or died (exit {code}): {first_line(import_text)}"
    if verdict == "error" or not out_flare.exists():
        return "refused", cause_line(import_text) or f"import exit {code}"
    notes = []
    verdict, text, code = run([flare, "info", str(out_flare)], timeout)
    if verdict in ("timeout", "panic"):
        return "crash", f"info {verdict} on the imported project"
    if verdict == "error":
        return "crash", f"info failed on the imported project: {first_line(text)}"
    verdict, text, code = run([flare, "export", str(out_flare), str(out_png), "--frame", "0"], timeout)
    if verdict in ("timeout", "panic"):
        return "crash", f"export {verdict} on the imported project"
    if verdict == "error" or not out_png.exists():
        notes.append("still export failed: " + first_line(text))
    counted = import_counts(import_text)
    if counted is None:
        return "crash", "import printed no report: " + first_line(import_text)
    unsupported, warnings = counted
    if unsupported or warnings:
        return "partial", f"{unsupported} unsupported, {warnings} warnings"
    return "ok", "; ".join(notes) if notes else "info and still export ok"


def import_counts(text: str) -> tuple[int, int] | None:
    """The importer's own counts from its JSON report on stdout; None if there is none."""
    start = text.find("{")
    if start < 0:
        return None
    try:
        report = json.loads(text[start:])
    except json.JSONDecodeError:
        return None
    return int(report.get("unsupported", 0)), int(report.get("warnings", 0))


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--flare", required=True, help="path to the flare binary")
    ap.add_argument("--work", required=True, type=Path, help="scratch directory for outputs")
    ap.add_argument("--out-md", type=Path, help="write the report table here")
    ap.add_argument("--out-tsv", type=Path, help="write one row per file here")
    ap.add_argument("--per-type", type=int, default=20, help="files sampled per type")
    ap.add_argument("--seed", type=int, default=7)
    ap.add_argument("--timeout", type=float, default=60.0, help="seconds per flare call")
    ap.add_argument("--only-type", action="append", default=[], help="run only these type labels")
    args = ap.parse_args()

    if not Path(args.flare).is_file():
        print(f"flare binary not found: {args.flare}", file=sys.stderr)
        return 2
    args.work.mkdir(parents=True, exist_ok=True)
    rng = random.Random(args.seed)

    rows: list[dict[str, str]] = []
    for label, exts, expected, rule in TYPES:
        if args.only_type and label not in args.only_type:
            continue
        found = collect(exts, rule)
        picks = stratified_sample(found, args.per_type, rng)
        if not picks:
            rows.append({"type": label, "file": "-", "source": "-", "result": "none",
                         "notes": NOT_IN_CORPUS_NOTE, "expected": expected})
            continue
        for source_label, src in picks:
            result, note = check_one(args.flare, src, args.work, args.timeout)
            rows.append({"type": label, "file": str(src), "source": f"{source_label}: {licence_of(source_label)}",
                         "result": result, "notes": note, "expected": expected})
            print(f"{label:9} {result:8} {src.name}", file=sys.stderr)

    if args.out_tsv:
        with args.out_tsv.open("w", encoding="utf-8") as fh:
            fh.write("type\tfile\tsource\tresult\texpected\tnotes\n")
            for r in rows:
                fh.write("\t".join([r["type"], r["file"], r["source"], r["result"], r["expected"], r["notes"]]) + "\n")
    if args.out_md:
        args.out_md.write_text(render(rows), encoding="utf-8")
    print(summary(rows))
    return 0


def met_expectation(row: dict[str, str]) -> bool:
    """An "open" type must import (ok or partial); a "refuse" type must be refused cleanly."""
    if row["expected"] == "open":
        return row["result"] in ("ok", "partial")
    return row["result"] == "refused"


def summary(rows: list[dict[str, str]]) -> str:
    lines = []
    for label, _e, _x, _r in TYPES:
        mine = [r for r in rows if r["type"] == label and r["result"] != "none"]
        if not mine:
            lines.append(f"{label}: no samples")
            continue
        count = {k: sum(1 for r in mine if r["result"] == k) for k in ("ok", "partial", "refused", "crash")}
        met = sum(1 for r in mine if met_expectation(r))
        lines.append(
            f"{label}: ok {count['ok']}/{len(mine)} partial {count['partial']} "
            f"refused {count['refused']} crash {count['crash']} expected-met {met}/{len(mine)}"
        )
    return "\n".join(lines)


def render(rows: list[dict[str, str]]) -> str:
    out = ["| type | file | licence/source | result | notes |", "|---|---|---|---|---|"]
    for r in rows:
        out.append(f"| {r['type']} | {r['file']} | {r['source']} | {r['result']} | {r['notes']} |")
    return "\n".join(out) + "\n"


if __name__ == "__main__":
    sys.exit(main())
