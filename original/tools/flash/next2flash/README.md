# Next2Flash in Flare

Next2Flash (MIT) is being **chopped up and re-implemented inside Flare's own
Rust code**; nothing of it runs as a separate program any more (no Python
sidecar, no vendored package, no Flask / Electron / Flex bridge).

Where things are now:

| Capability | Flare file |
|---|---|
| ABC parse / serialize, constant pools, opcodes, assembler, disassembler, editor | `flare/sources/common/flash/ac_as3_abc.rs` |
| Method + class + script decompiler (AS3 source out) | `ac_as3_decompile.rs` |
| Pool merge, class transplant, inline constant patch, string patch, SWF tag write-back, `mxmlc` driver | `ac_as3_patch.rs` |
| JSON API used by the FFI (`flare_as3_*` in `flare_formats.rs`) | `ac_as3_api.rs` |
| C++ client (`As3Bridge::decompile / patchStrings / patchClassSource / compile`) | `common/flash/As3Bridge.{h,cpp}` |
| Goldens recorded from the Python original | `as3_cases.json` via `tests/native/gen_as3_cases.py` |

Status table, remaining stages and licence notes:
[`docs/NEXT2FLASH_INTEGRATION.md`](../../../docs/NEXT2FLASH_INTEGRATION.md).

`ts/` holds Flare's TypeScript SWF/FCP importers (unrelated to the removed
sidecar; kept, they are the TS side of the JS-to-TS rule).

Regenerating the goldens needs a Next2Flash checkout (`NEXT2FLASH_DIR`):
`python tests/native/gen_as3_cases.py [--check]`.
