# Merging Next2Flash into Flare — Integration Plan

Status: **stage 1 done: AS3/ABC core is native Rust; sidecar removed**

[Next2Flash](https://github.com/SSF2-Mods-Official/Next2Flash) and Flare are being
brought together as the Flare-Animate org consolidates its Flash tooling. This
document records what Next2Flash actually is, why a naïve "copy the files in"
merge will not work, and the concrete path that will.

## What Next2Flash is

A desktop **SWF round-trip editor** — "import a SWF, edit it visually, export it
back to a working SWF." MIT-licensed, built on the open-source Next2D engine.

| Layer | Tech | Role |
|-------|------|------|
| Shell | Electron | Desktop window, native menus/dialogs |
| Editor | Next2D (JS / HTML / CSS) | Visual timeline, stage, library |
| Backend | Python + Flask | SWF parse, conversion, **AS3 (de)compilation** |
| Toolchain | Flex SDK (bundled) | Compiles AS3 back into the SWF |

The genuinely valuable, hard-to-replicate parts live in `app/`:

- `app/as3_decompiler/` — a from-scratch **ABC/AS3 bytecode** parser, decompiler,
  and patcher (`abc_parser.py`, `method_decompiler.py`, `opcodes.py`,
  `swf_reader.py`, `swf_patcher.py`). This is the crown jewel — native AS3
  round-tripping without JPEXS.
- `bitmap_converter.py`, `char_id_allocator.py`, `cycle_detector.py`,
  `conversion_service.py`, `compilation_pipeline.py` — the SWF asset/ID pipeline.
- `app/assets/js/swf-parse-worker.js`, `swf-timeline-importer.js` — JS-side SWF
  tag parsing and timeline reconstruction.

## Why a direct merge does not work

Flare is **C++ / Qt**. Next2Flash is **Python + JavaScript + Electron**. There is
no shared language, build system, or object model:

- Flare's `doc/FLASH_SUPPORT.md` deliberately moved *away* from external runtimes
  (it dropped the JPEXS/Python approach for native C++) for licensing and
  zero-dependency reasons. Pulling a Python+Flask+Flex-SDK stack back in reverses
  that decision and adds a heavy runtime to every install.
- Electron/Next2D's renderer cannot be embedded in a Qt app.

So "merge the codebase" has to mean **merge the capabilities**, choosing per
component between *porting* and *bridging*.

## Component-by-component mapping

| Next2Flash capability | Flare today | Recommended path |
|-----------------------|-------------|------------------|
| SWF tag parsing | `common/flash/FSWFStream`, `FDT*`, `flashimport.cpp::readSwfHeader/extractSwfBitmaps` | **Port** — extend Flare's native tag reader using N2F's tag handling as reference (both MIT/BSD-compatible) |
| FLA/XFL parsing | `common/flash/XFLReader` | Keep Flare's; cross-check against N2F timeline importer |
| **AS3 / ABC (de)compile** | `common/flash/FAction.*` (stubs only) | **Bridge first, port later** — see below |
| SWF *export* / round-trip | `common/flash/tflash` (writer) | Port N2F's `swf_patcher` ID-preservation approach |
| Visual timeline editor | Flare xsheet/timeline (native) | Already covered by Flare — no port needed |

## Direction (changed)

The first plan bridged AS3 through an optional Python sidecar. That is gone.
Next2Flash is now **chopped up and re-implemented in Flare's Rust code** to
extend Flare during the rewrite. No Python, Flask, Electron or Flex bridge is
used as the integration; Flex's `mxmlc` is the only external tool left, and
only for compiling brand-new ActionScript.

## Licensing

Next2Flash is **MIT**; Flare is **Modified BSD (3-Clause)**. MIT code can be
incorporated into a BSD project provided the MIT copyright notice is preserved.
Any ported file or bundled sidecar must keep Next2Flash's `LICENSE` and a header
note. The bundled **Flex SDK** is Apache-2.0 and must ship with its own NOTICE.

## Status of the plan (stage 1 done: AS3 / ABC core)

| Next2Flash piece | Flare | State |
|---|---|---|
| `abc_parser.py`, `opcodes.py`, `abc_editor.py`, serializer + instruction tables of `abc_patcher.py`, DoABC extraction of `swf_reader.py` | `common/flash/ac_as3_abc.rs` | ported, goldens vs Python |
| `helpers.py`, `method_decompiler.py`, `class_decompiler.py` | `common/flash/ac_as3_decompile.rs` | ported (all structuring passes), goldens vs Python on 148 methods / 33 ABC files incl. a seeded random-method generator |
| `abc_patcher.py` (pool merge, transplant, inline constant patch, `difflib`), `swf_patcher.py` (tags, string patch) | `common/flash/ac_as3_patch.rs` | ported, goldens vs Python; SWF container comes from `ac_swf.rs` (`swf_reader.py` tag streaming not re-ported) |
| `cli.py`, `flare_as3_bridge.py` | `ac_as3_api.rs` + `flare_as3_*` C ABI | replaced: `As3Bridge.cpp` calls Rust directly (no process, no JSON-over-stdout) |
| `swf_patcher.recompile_class(es)` (mxmlc JVM shell, decompile-all-then-compile) | `ac_as3_patch.rs::compile_sources` (one-shot mxmlc) | partial: JVM shell manager and the full recompile workflow not ported |

New functionality beyond the Python CLI:

- class / member / script inventory with signatures and code sizes (`flare_as3_list`)
- P-code disassembly of any method (`flare_as3_disasm`)
- script-level (top-level function / variable) decompile
- **`patch_class_source`: write an edited class back into the SWF with no Flex
  SDK** when the edit is expressible (string literals; ID-preserving: only the
  changed methods' constants are repointed, every other class, tag and pool
  index keeps its bytes). Anything else reports `needs compiler`.
- `.abc` bare blocks and SWFs go through one entry point

Deviations from the Python: `// decompile error:` text differs; ZWS (LZMA) SWFs
are refused (`ac_swf` has no LZMA); diagnostics are returned as a log instead
of printed; the SWF writer re-compresses with flate2 (same content as Python's
zlib, different bytes); an `inf` `pushdouble` is a decompile error instead of
a Python crash.

Remaining stages (not started):

- [ ] `swf_to_n2d.py`, `compile_n2d.py`: Next2D-specific export. Do **not**
      port the Next2D format; replace with Flare-native SWF / HTML5 exports via
      the existing `ac_swf_write.rs` and `html5_runtime.ts`.
- [ ] `shape_converter.py`, `swf_shape_to_recodes.py`: compare with the shape
      code in `ac_swf_read.rs` / `ac_swf_write.rs` and merge improvements.
- [ ] `fla_xfl.py`: compare with `ac_xfl*.rs`.
- [ ] `bitmap_converter.py`: byte pass-through already done (see below); check
      the remaining lossless variants.
- [ ] `char_id_allocator.py`: `CharIds` in `ac_swf.rs` already keeps ids; check
      the compaction edge cases only.
- [ ] `cycle_detector.py`, `conversion_service.py`, `compilation_pipeline.py`.
- [ ] `swf-parse-worker.js`, `swf-timeline-importer.js`: TS only if still
      needed after the Rust SWF reader covers them.
- [ ] `nflare`: the request mentioned it, but nothing named `nflare` exists in
      this repository (only `sunflare` shader files match). Unidentified; no
      action taken.

## Rust re-implementation status (Track 1)

Done in Rust inside the `ac-xfl` and `ac-render` crates, from the public SWF
File Format Specification and Next2Flash's observable behaviour (MIT notice kept
in `flare/sources/THIRD-PARTY-NOTICES.md`; NFlare, which is GPL-3, was not
consulted). No SWF shape/tag code is copied from Python.

| Feature | Done | File | Test |
|---|---|---|---|
| SWF character-id preservation (ids stored on import in `BitmapAsset::swf_char_id` / `Document::swf_char_ids`, reused by the writer, fresh ids above the max, never colliding, 0 reserved) | yes | `common/flash/ac_swf.rs` (`CharIds`), `ac_swf_read.rs`, `ac_swf_write.rs`; model fields in `flarelib/{bitmap,model}.rs`; saved in `.acx` by `common/tsystem/ac_package.rs` | `ac_swf_tests.rs`: `export_reuses_remembered_character_ids`, `fresh_ids_are_above_the_maximum_and_unique`, `duplicate_and_missing_ids_never_collide`, `import_export_import_keeps_ids`; `ac_swf.rs`: `ids_*` |
| Minimal SWF tag writer (DefineShape3, DefineBitsLossless2/JPEG2/JPEG3, DefineSprite, PlaceObject2, RemoveObject2, FrameLabel, ShowFrame, End; CWS/zlib) | yes | `ac_swf.rs`, `ac_swf_write.rs`, CLI `flare-cli swf` | `multi_frame_timeline_moves_and_removes`, `bitmap_fill_and_stroke_survive_export`, `curves_and_cubics_export_as_quadratics_and_return`, `container_roundtrip_both_compressions` |
| SWF reader for the above (+ DefineBits, Lossless v1, shapes 1-4, sprites, timeline) | yes | `ac_swf_read.rs` | `imported_sprite_keeps_children_and_placement`, `jpeg3_alpha_and_8bit_lossless_import`, `lossless_pixels_round_trip_through_swf` |
| Bitmap pass-through (original JPEG/PNG/GIF/lossless/JPEG3 bytes kept in `BitmapAsset::original`, written back byte-identical while the pixels are unedited; an edit invalidates it via a pixel hash) | yes | `flarelib/bitmap.rs`, `ac_swf_write.rs`, `ac_package.rs` | `jpeg_png_and_lossless_bitmaps_are_written_back_byte_identical`, `edited_bitmap_is_reencoded_not_passed_through` |
| SWF font (DefineFont2/3) ↔ TrueType, pure Rust | yes (standalone; not yet embedded as a font in exported SWFs, no text import/export) | `ac_swf_font.rs`, CLI `flare-cli swf-font` | `define_font3_round_trip`, `swf_font_to_ttf_is_a_valid_font`, `ttf_checksum_adjustment_is_consistent`, `ttf_to_define_font3_and_back_round_trip` |
| Texture atlas (MaxRects, optional rotation, padding, power of two) + `flare-cli atlas` (PNG + JSON) | yes | `common/tvrender/ac_formats.rs`, `flareqt/ac_cli_main.rs` | `common/tvrender/tests/atlas.rs` |

Not done / known limits: text, buttons, gradients, filters, blend modes,
sounds and ActionScript are not written or read by the Rust SWF code (each is
reported as a warning; AS3 is covered by the native core above); graphic-symbol
playback options are exported as looping sprites; ZWS (LZMA) SWFs are refused;
mask/guide layers are skipped; a DefineBitsJPEG (tag 6) is passed through as
the spliced JPEG, not as a tag-6 record. The output has been checked only by
this crate's own reader (no Flash Player / Ruffle in the build sandbox); the
generated TrueType was checked with FreeType (`fc-scan`).
