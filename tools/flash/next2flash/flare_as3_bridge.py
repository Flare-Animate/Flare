#!/usr/bin/env python3
"""flare-as3 bridge - optional AS3 (ActionScript 3 / ABC bytecode) helper.

Flare-authored CLI contract for the Next2Flash merge (see
doc/NEXT2FLASH_INTEGRATION.md, Track 2). Flare's C++ importer shells out to
this script the same way it detects FFmpeg: if it's absent or reports
unavailable, AS3 import/export is simply skipped and everything else (FLA/XFL
bitmap + timeline import) works exactly as before.

vendor/as3_decompiler/ (from Next2Flash, MIT licensed) is populated, so
`status`, `decompile` and `patch` are live. `compile` still needs the Flex SDK
toolchain that is not part of the vendored slice and reports itself unported.
When the vendored package is missing or fails to import, every command degrades
to an "unavailable" answer instead of raising, so a caller can probe the bridge
the same way Flare probes for FFmpeg.

Protocol: one JSON object on stdout per invocation.
  flare_as3_bridge.py status
      -> {"available": bool, "version": str|None}
        (schema is fixed; use `capabilities` for what each command can do)
  flare_as3_bridge.py capabilities
      -> {"available": bool, "commands": {name: {"ok": bool, "detail": str}},
          "flex_sdk": str|None}
  flare_as3_bridge.py decompile <input.swf|input.abc> <output_dir>
      -> {"ok": bool, "classes": [str], "error": str|None}
  flare_as3_bridge.py compile <source_dir> <output.swf>
      -> {"ok": bool, "error": str|None, "log": str|None}
  flare_as3_bridge.py patch <input.swf> <patch.json> <output.swf>
      -> {"ok": bool, "replaced": int, "blocks": int, "error": str|None}

`decompile` also accepts a bare .abc block, which is what the Flex SDK and
`asc2.jar` emit and what Next2Flash's own pipeline passes around.

`compile` drives the Flex SDK's mxmlc, which is what Next2Flash uses. The SDK
is not bundled; when it is absent the command says exactly where it looked
rather than failing vaguely, and `capabilities` reports the same thing up
front.

patch.json format:
  {"strings": {"old text": "new text", ...}}

Every AS3 constant-string pool entry in the SWF that matches a key exactly is
replaced with its value, and the SWF is re-emitted with all other tags (shapes,
bitmaps, sounds, timeline) byte-identical. This is the retexting/relinking pass
Flare needs for imported Flash rigs, and it is pure Python — no Flex SDK.
"""
from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

VENDOR_DIR = Path(__file__).parent / "vendor" / "as3_decompiler"


def _vendor_available() -> bool:
    return (VENDOR_DIR / "__init__.py").is_file()


def _import_vendor():
    """Import the vendored as3_decompiler package (adds vendor/ to sys.path)."""
    import sys as _sys
    vendor_root = str(VENDOR_DIR.parent)
    if vendor_root not in _sys.path:
        _sys.path.insert(0, vendor_root)
    import as3_decompiler as pkg
    return pkg


# ---------------------------------------------------------------------------
# Flex SDK discovery
#
# AS3 recompilation needs the Flex SDK's mxmlc, exactly as it does in
# Next2Flash. The SDK is ~70 MB and is not vendored, so we discover it rather
# than ship it. Discovery order:
#   1. $FLARE_FLEX_SDK         - explicit override
#   2. $FLEX_SDK / $AIR_SDK    - the SDK's own variables
#   3. $FLAREROOT              - Flare's installation root
#   4. the usual install locations
# ---------------------------------------------------------------------------
FLEX_SDK_ENV_VARS = ("FLARE_FLEX_SDK", "FLEX_SDK", "AIR_SDK")
FLEX_SDK_FALLBACK_DIRS = (
    "/opt/flex-sdk", "/opt/air-sdk", "/usr/local/flex-sdk",
    "/Applications/Adobe Flex SDK", "C:/flex-sdk", "C:/air-sdk",
)


def find_flex_sdk():
    """Return the Flex SDK root, or None. Never raises."""
    for var in FLEX_SDK_ENV_VARS:
        p = os.environ.get(var)
        if p and os.path.isdir(p):
            return p
    for var in FLEX_SDK_FALLBACK_DIRS:
        if os.path.isdir(var):
            return var
    return None


def find_mxmlc():
    """Return (path_to_mxmlc, sdk_root_or_None). Never raises.

    Falls back to mxmlc on PATH, which is how a system Flex install usually
    presents itself.
    """
    sdk = find_flex_sdk()
    if sdk:
        for name in ("mxmlc.bat", "mxmlc", "mxmlc.exe", "mxmlc.sh"):
            cand = os.path.join(sdk, "bin", name)
            if os.path.isfile(cand):
                return cand, sdk
    for name in ("mxmlc", "mxmlc.bat", "mxmlc.exe"):
        found = shutil.which(name)
        if found:
            return found, sdk
    return None, sdk


def _sdk_search_note():
    looked = [v for v in FLEX_SDK_ENV_VARS if os.environ.get(v)]
    looked += list(FLEX_SDK_FALLBACK_DIRS)
    return "looked in " + ", ".join(looked)


def _is_abc(path):
    """A bare ABC block rather than a SWF."""
    if path.lower().endswith(".abc"):
        return True
    try:
        with open(path, "rb") as f:
            head = f.read(4)
    except OSError:
        return False
    # A SWF always starts FWS/CWS/ZWS; anything else is treated as raw ABC.
    return head[:2] not in (b"FW", b"CW", b"ZW")


def cmd_status() -> dict:
    # Schema is fixed at {available, version} per the documented protocol —
    # diagnostics on the failure path go to stderr, not into the JSON, so
    # callers can rely on a stable shape rather than checking for an
    # occasionally-present extra key.
    if not _vendor_available():
        return {"available": False, "version": None}
    try:
        _import_vendor()
        return {"available": True, "version": "next2flash-as3-decompiler"}
    except Exception as e:
        print(f"flare_as3_bridge: vendor import failed: {e}", file=sys.stderr)
        return {"available": False, "version": None}


def cmd_decompile(swf_path: str, out_dir: str) -> dict:
    if not _vendor_available():
        return {"ok": False, "classes": [],
                "error": "as3_decompiler not vendored — see tools/flash/next2flash/README.md"}
    try:
        pkg = _import_vendor()

        # A bare .abc block (what asc2.jar and the Flex SDK emit, and what
        # Next2Flash's own pipeline passes around) is not a SWF, so the
        # tag-walking reader cannot be used on it.
        if _is_abc(swf_path):
            with open(swf_path, "rb") as f:
                abc_data = f.read()
            if not abc_data:
                return {"ok": False, "classes": [],
                        "error": "ABC block is empty"}
            out = Path(out_dir)
            block_dir = out / "block_0"
            block_dir.mkdir(parents=True, exist_ok=True)
            abc = pkg.ABCFile(abc_data)
            dec = pkg.AS3Decompiler(abc)
            n = dec.decompile_all(str(block_dir))
            return {"ok": True,
                    "classes": [f"{Path(swf_path).name} ({n} class(es), in block_0/)"],
                    "error": None}

        _, abc_blocks = pkg.read_abc_blocks(swf_path)
        if not abc_blocks:
            # No embedded AS3 is not a failure — ok=True/error=None, with an
            # empty classes list telling the caller there was nothing to do.
            return {"ok": True, "classes": [], "error": None}

        out = Path(out_dir)
        out.mkdir(parents=True, exist_ok=True)
        classes: list[str] = []
        # A SWF can carry multiple DoABC/DoABC2 tags, and class/package names
        # can collide across blocks — decompiling them all into the same
        # out_dir would let a later block silently overwrite an earlier
        # block's files. Give each block its own subdirectory instead.
        for i, (name, abc_data) in enumerate(abc_blocks):
            block_dir = out / f"block_{i}"
            block_dir.mkdir(parents=True, exist_ok=True)
            abc = pkg.ABCFile(abc_data)
            dec = pkg.AS3Decompiler(abc)
            n = dec.decompile_all(str(block_dir))
            classes.append(f"{name} ({n} class(es), in block_{i}/)")
        return {"ok": True, "classes": classes, "error": None}
    except Exception as e:
        return {"ok": False, "classes": [], "error": f"decompile failed: {e}"}


def cmd_compile(source_dir: str, out_swf: str) -> dict:
    """Compile ActionScript sources to an SWF using the Flex SDK's mxmlc.

    This is the same toolchain Next2Flash uses. The SDK is not vendored (it is
    ~70 MB), so it is discovered at run time; when it is missing we say exactly
    where we looked instead of failing vaguely.
    """
    if not _vendor_available():
        return {"ok": False, "error": "as3_decompiler not vendored", "log": None}

    src = Path(source_dir)
    if not src.is_dir():
        return {"ok": False, "error": f"not a directory: {source_dir}", "log": None}
    sources = sorted(src.rglob("*.as"))
    if not sources:
        return {"ok": False, "error": f"no .as sources found in {source_dir}",
                "log": None}

    mxmlc, sdk = find_mxmlc()
    if not mxmlc:
        return {"ok": False,
                "error": ("AS3 compilation needs the Flex SDK (mxmlc), which is "
                          "not vendored. Set FLARE_FLEX_SDK to the SDK root, or "
                          "put mxmlc on PATH. " + _sdk_search_note()),
                "log": None}

    out = Path(out_swf)
    if out.parent and str(out.parent):
        out.parent.mkdir(parents=True, exist_ok=True)
    cmd = [mxmlc, "-source-path", str(src), "-output", str(out)]
    cmd += [str(p) for p in sources]
    try:
        proc = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                              timeout=600)
    except Exception as e:
        return {"ok": False, "error": f"mxmlc could not be run: {e}", "log": None}
    except subprocess.TimeoutExpired:
        return {"ok": False, "error": "mxmlc timed out after 600s", "log": None}

    log = proc.stdout.decode("utf-8", "replace") if proc.stdout else ""
    if proc.returncode != 0 or not out.is_file():
        return {"ok": False,
                "error": f"mxmlc exited with status {proc.returncode}",
                "log": log[-8000:]}
    return {"ok": True, "error": None, "log": None,
            "output": str(out), "sources": len(sources),
            "flex_sdk": sdk}


def cmd_capabilities() -> dict:
    """What can this bridge actually do right now, and why not?

    `status` has a deliberately fixed two-key schema that the C++ client and
    the tests both rely on, so capability reporting lives here instead.
    """
    available = False
    version = None
    if _vendor_available():
        try:
            _import_vendor()
            available = True
            version = "next2flash-as3-decompiler"
        except Exception as e:
            print(f"flare_as3_bridge: vendor import failed: {e}", file=sys.stderr)

    mxmlc, sdk = find_mxmlc()
    commands = {
        "status": {"ok": True, "detail": "always available"},
        "decompile": {"ok": available,
                      "detail": "SWF and bare .abc blocks"
                                if available else "as3_decompiler not vendored"},
        "patch": {"ok": available,
                  "detail": "AS3 constant-string rewrite, no SDK needed"
                            if available else "as3_decompiler not vendored"},
        "compile": {"ok": bool(mxmlc),
                    "detail": f"mxmlc at {mxmlc}" if mxmlc
                              else "needs the Flex SDK; " + _sdk_search_note()},
    }
    return {"available": available, "commands": commands, "flex_sdk": sdk}


def cmd_patch(swf_path: str, patch_json: str, out_swf: str) -> dict:
    if not _vendor_available():
        return {"ok": False, "replaced": 0, "blocks": 0,
                "error": "as3_decompiler not vendored"}
    try:
        with open(patch_json, encoding="utf-8") as f:
            patch = json.load(f)
    except Exception as e:
        return {"ok": False, "replaced": 0, "blocks": 0,
                "error": f"could not read patch file: {e}"}

    mapping = patch.get("strings") or {}
    if not isinstance(mapping, dict):
        return {"ok": False, "replaced": 0, "blocks": 0,
                "error": 'patch "strings" must be an object of {old: new}'}
    if not mapping:
        return {"ok": False, "replaced": 0, "blocks": 0,
                "error": 'patch contains no "strings" entries'}

    try:
        _import_vendor()
        from as3_decompiler import abc_editor, abc_patcher, swf_patcher

        swf = swf_patcher.read_swf_full(swf_path)
        tags = swf["tags"]
        abc_tags = (swf_patcher.TAG_DOABC, swf_patcher.TAG_DOABC2)

        replaced = 0
        blocks = 0
        for i, (tag_type, body) in enumerate(tags):
            if tag_type not in abc_tags:
                continue

            # Split the tag body ourselves rather than through the vendor's
            # extract helper: DoABC2 carries a flags word and a name that must
            # both survive the round-trip byte-for-byte, and the helper
            # substitutes a display name for an empty one.
            if tag_type == swf_patcher.TAG_DOABC2:
                if len(body) < 5:
                    continue
                null_pos = body.find(b"\x00", 4)
                if null_pos < 0:
                    continue
                prefix, abc_data = body[:null_pos + 1], body[null_pos + 1:]
            else:
                prefix, abc_data = b"", body

            editor = abc_editor.ABCEditor(abc_data)
            strings = editor.abc.strings
            hits = 0
            # Index 0 is the ABC "any" sentinel (always the empty string) and is
            # never a real constant — replacing it corrupts every multiname.
            for idx in range(1, len(strings)):
                new_value = mapping.get(strings[idx])
                if new_value is not None and new_value != strings[idx]:
                    strings[idx] = new_value
                    hits += 1
            if not hits:
                continue

            tags[i] = (tag_type, prefix + abc_patcher.serialize_abc(editor.abc))
            replaced += hits
            blocks += 1

        if replaced == 0:
            return {"ok": False, "replaced": 0, "blocks": 0,
                    "error": "no matching AS3 constant strings found in this SWF"}

        swf_patcher.write_swf_from_tags(swf, out_swf)
        return {"ok": True, "replaced": replaced, "blocks": blocks, "error": None}
    except Exception as e:
        return {"ok": False, "replaced": 0, "blocks": 0,
                "error": f"patch failed: {e}"}


USAGE = """flare-as3 bridge - optional AS3 (ActionScript 3) helper for Flare

usage: flare_as3_bridge.py <command> [args...]

commands:
  status                          probe availability
                                  -> {"available": bool, "version": str|None}
  capabilities                    what each command can do right now, and why not
                                  -> {"available": bool, "commands": {...},
                                      "flex_sdk": str|None}
  decompile <in.swf|in.abc> <dir> write ActionScript sources into <dir>
                                  -> {"ok": bool, "classes": [str], "error": str|None}
  compile <src_dir> <out.swf>     compile .as sources with the Flex SDK's mxmlc
                                  -> {"ok": bool, "error": str|None, "log": str|None}
  patch <in.swf> <p.json> <out>    rewrite AS3 constant strings
                                  -> {"ok": bool, "replaced": int, "blocks": int,
                                      "error": str|None}

patch.json format:
  {"strings": {"old text": "new text", ...}}

Every command prints exactly one JSON object on stdout. Diagnostics go to
stderr, so stdout stays machine-readable.

Optional: set FLARE_FLEX_SDK to the Flex SDK root to enable `compile`.
"""


def main(argv: list[str]) -> int:
    if len(argv) < 2 or argv[1] in ("-h", "--help", "help"):
        if len(argv) > 1 and argv[1] in ("-h", "--help", "help"):
            print(USAGE)
            return 0
        print(json.dumps({"ok": False,
                          "error": "usage: flare_as3_bridge.py "
                                   "<status|capabilities|decompile|compile|patch> "
                                   "[args...]"}))
        return 2
    cmd = argv[1]
    try:
        if cmd == "status":
            result = cmd_status()
        elif cmd == "capabilities":
            result = cmd_capabilities()
        elif cmd == "decompile" and len(argv) == 4:
            result = cmd_decompile(argv[2], argv[3])
        elif cmd == "compile" and len(argv) == 4:
            result = cmd_compile(argv[2], argv[3])
        elif cmd == "patch" and len(argv) == 5:
            result = cmd_patch(argv[2], argv[3], argv[4])
        else:
            result = {"ok": False, "error": f"unknown command or bad args: {argv[1:]}"}
    except Exception as e:  # bridge must never crash the caller
        result = {"ok": False, "error": f"bridge exception: {e}"}
    print(json.dumps(result))
    return 0 if result.get("ok", result.get("available", False)) else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv))
