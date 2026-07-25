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


def cmd_status() -> dict:
    return {"available": _vendor_available(), "version": None}


def cmd_decompile(swf_path: str, out_dir: str) -> dict:
    if not _vendor_available():
        return {"ok": False, "classes": [],
                "error": "as3_decompiler not vendored — see tools/flash/next2flash/README.md"}
    # sys.path.insert(0, str(VENDOR_DIR.parent))
    # from as3_decompiler.swf_reader import ...
    return {"ok": False, "classes": [], "error": "not yet implemented"}


def cmd_compile(source_dir: str, out_swf: str) -> dict:
    if not _vendor_available():
        return {"ok": False, "error": "as3_decompiler not vendored"}
    return {"ok": False, "error": "not yet implemented"}


def cmd_patch(swf_path: str, patch_json: str, out_swf: str) -> dict:
    if not _vendor_available():
        return {"ok": False, "error": "as3_decompiler not vendored"}
    return {"ok": False, "error": "not yet implemented"}


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
