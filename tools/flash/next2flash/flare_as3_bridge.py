#!/usr/bin/env python3
"""flare-as3 bridge — optional AS3 (ActionScript 3 / ABC bytecode) helper.

Flare-authored CLI contract for the Next2Flash merge (see
doc/NEXT2FLASH_INTEGRATION.md, Track 2). Flare's C++ importer shells out to
this script the same way it detects FFmpeg: if it's absent or reports
unavailable, AS3 import/export is simply skipped and everything else (FLA/XFL
bitmap + timeline import) works exactly as before.

Once vendor/as3_decompiler/ (from Next2Flash, MIT licensed) is populated, the
three commands below call into it. Until then this is a stub that reports
itself unavailable, so `flashimport.cpp` can probe for the bridge safely.

Protocol: one JSON object on stdout per invocation.
  flare_as3_bridge.py status
      -> {"available": bool, "version": str|None}
  flare_as3_bridge.py decompile <input.swf> <output_dir>
      -> {"ok": bool, "classes": [str], "error": str|None}
  flare_as3_bridge.py compile <source_dir> <output.swf>
      -> {"ok": bool, "error": str|None}
  flare_as3_bridge.py patch <input.swf> <patch.json> <output.swf>
      -> {"ok": bool, "error": str|None}
"""
from __future__ import annotations

import json
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
        _, abc_blocks = pkg.read_abc_blocks(swf_path)
        if not abc_blocks:
            # No embedded AS3 is not a failure — ok=True/error=None, with an
            # empty classes list telling the caller there was nothing to do.
            return {"ok": True, "classes": [], "error": None}

        out = Path(out_dir)
        out.mkdir(parents=True, exist_ok=True)
        classes: list[str] = []
        for name, abc_data in abc_blocks:
            abc = pkg.ABCFile(abc_data)
            dec = pkg.AS3Decompiler(abc)
            n = dec.decompile_all(str(out))
            classes.append(f"{name} ({n} class(es))")
        return {"ok": True, "classes": classes, "error": None}
    except Exception as e:
        return {"ok": False, "classes": [], "error": f"decompile failed: {e}"}


def cmd_compile(source_dir: str, out_swf: str) -> dict:
    if not _vendor_available():
        return {"ok": False, "error": "as3_decompiler not vendored"}
    # AS3 recompilation needs the Flex SDK toolchain Next2Flash bundles, which
    # is not part of this vendored slice (decompiler only). Left for a later
    # pass — see doc/NEXT2FLASH_INTEGRATION.md Track 2.
    return {"ok": False, "error": "AS3 recompilation not yet ported (decompile-only for now)"}


def cmd_patch(swf_path: str, patch_json: str, out_swf: str) -> dict:
    if not _vendor_available():
        return {"ok": False, "error": "as3_decompiler not vendored"}
    return {"ok": False, "error": "AS3 patching not yet ported (decompile-only for now)"}


def main(argv: list[str]) -> int:
    if len(argv) < 2:
        print(json.dumps({"ok": False, "error": "usage: flare_as3_bridge.py <status|decompile|compile|patch> [args...]"}))
        return 2
    cmd = argv[1]
    try:
        if cmd == "status":
            result = cmd_status()
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
