# Next2Flash merge — status

Tracks the merge described in [`doc/NEXT2FLASH_INTEGRATION.md`](../../../doc/NEXT2FLASH_INTEGRATION.md).

**Vendored: `vendor/as3_decompiler/`** — Next2Flash's ABC/AS3 decompiler
package (MIT, `LICENSE` included alongside). 12 files, ~12.7k lines:
`swf_reader.py` (SWF tag streaming, DoABC/DoABC2 extraction), `abc_parser.py`
(AVM2 ABC binary parser), `opcodes.py`, `helpers.py`, `method_decompiler.py`,
`class_decompiler.py`, `abc_editor.py`, `abc_patcher.py`, `swf_patcher.py`,
`cli.py`.

**`flare_as3_bridge.py`** (Flare-authored) — the `flare-as3` CLI contract from
`doc/NEXT2FLASH_INTEGRATION.md`. `status`/`decompile` are wired to the
vendored package and tested (import works, decompile runs end-to-end against
a real SWF). `compile`/`patch` (AS3 *recompilation*) are stubs — that needs
the Flex SDK toolchain Next2Flash bundles, not vendored yet.

`flashimport.cpp` probes this bridge the same way it detects FFmpeg: absent
or unavailable → AS3 import is skipped, everything else (FLA/XFL bitmap +
timeline import) is unaffected. C++ wiring into the import menu is next.
