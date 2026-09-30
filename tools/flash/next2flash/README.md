# Next2Flash merge

`flare/sources/common/flash/As3Bridge.{h,cpp}` (C++ client) ·
`tools/flash/next2flash/flare_as3_bridge.py` (JSON CLI) ·
`tools/flash/next2flash/vendor/as3_decompiler/` (vendored, MIT)

## What is merged

**Vendored: `vendor/as3_decompiler/`** — Next2Flash's ABC/AS3 decompiler
package (MIT, `LICENSE` included alongside). 12 modules, ~12.6k lines:
`swf_reader.py` (SWF tag streaming, DoABC/DoABC2 extraction), `abc_parser.py`
(AVM2 ABC binary parser), `opcodes.py`, `helpers.py`, `method_decompiler.py`,
`class_decompiler.py`, `abc_editor.py`, `abc_patcher.py`, `swf_patcher.py`,
`cli.py`. Provenance and the local lint deltas are recorded in
`vendor/as3_decompiler/VENDORED.md`.

**`flare_as3_bridge.py`** (Flare-authored) — the `flare-as3` CLI contract from
[`doc/NEXT2FLASH_INTEGRATION.md`](../../../doc/NEXT2FLASH_INTEGRATION.md).
One JSON object on stdout per invocation; every failure path returns a JSON
error rather than a traceback, so a caller can probe it the way Flare probes
FFmpeg.

**`As3Bridge.{h,cpp}`** (Flare-authored) — the C++ client. `As3Bridge::isAvailable()`
is probed once and cached; when the helper or a Python interpreter is missing,
every entry point returns a clear "unavailable" result and the rest of the
import is unaffected. Script resolution walks the executable directory, then
`$FLAREROOT`, then `PATH`, with `$FLARE_AS3_BRIDGE` as an explicit override.

## Command status

| Command   | State | Notes |
|-----------|-------|-------|
| `status`  | live  | `{"available": bool, "version": str}` |
| `decompile` | live | Verified end to end: 1043 AS3 classes recovered from a 3.5 MB SWF. |
| `patch`   | live  | Real AS3 constant-string retexting pass. Re-emits the SWF with every other tag byte-identical. Pure Python, no Flex SDK. |
| `compile` | stub  | Needs the Flex SDK `mxmlc` toolchain, which is not part of the vendored slice. `abc_patcher.transplant_class` and `swf_patcher.recompile_class` are already the bridge to it — only the toolchain is missing. |

## How it is wired in

`flashimport.cpp`'s SWF branch calls `As3Bridge::isAvailable()` after bitmap
extraction and, when present, `As3Bridge::decompile()` to write `.as` files into
`as3/` in the import directory, adding them to the manifest and the asset list.
When it is absent the dialog says so and says nothing else changes.

This is the "optional, auto-detected sidecar" shape Track 2 of the integration
plan describes: no Python, no Java, no JPEXS required to import a Flash file.

## Not merged, and why

Next2Flash's larger body of work is its `app/*.py` SWF pipeline —
`swf_to_n2d.py` (213 KB), `compile_n2d.py` (186 KB), `swf_writer.py`,
`shape_converter.py`, `char_id_allocator.py` — plus an Electron/Next2D editor
shell. The editor shell is redundant (Flare already has a native timeline and
xsheet) and the `app/` pipeline is still being ported natively; see
[`doc/NEXT2FLASH_INTEGRATION.md`](../../../doc/NEXT2FLASH_INTEGRATION.md) for
the per-component port plan.
