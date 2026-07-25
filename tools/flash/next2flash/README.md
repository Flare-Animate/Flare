# Next2Flash merge — status

Tracks the merge described in [`doc/NEXT2FLASH_INTEGRATION.md`](../../../doc/NEXT2FLASH_INTEGRATION.md).

**Vendored source: not yet present.** Pulling in
[Next2Flash](https://github.com/SSF2-Mods-Official/Next2Flash)'s actual
`app/as3_decompiler/` package requires network access to clone/download the
upstream repo, which this environment could not do this pass. `vendor/` is the
drop-in slot for it — MIT license, add a `LICENSE`/`NOTICE` copy alongside it
per the terms in the integration doc.

**What's here now:** `flare_as3_bridge.py`, Flare-authored (not Next2Flash
code) — the CLI contract Flare's importer will call once the vendored
decompiler lands. It mirrors the `flare-as3` interface from the integration
plan (`decompile` / `compile` / `patch`, JSON over stdout) and works today
in a stub form (reports AS3 support as unavailable) so `flashimport.cpp` can
detect the bridge the same way it detects FFmpeg — no hard dependency, no
behavior change until `vendor/` is populated.

## Wiring up the real thing

1. Drop Next2Flash's `app/as3_decompiler/` into `vendor/as3_decompiler/`.
2. Implement the three commands in `flare_as3_bridge.py` by calling into
   `vendor/as3_decompiler/{swf_reader,abc_parser,class_decompiler,swf_patcher}.py`.
3. Nothing else changes — the CLI contract stays the same.
