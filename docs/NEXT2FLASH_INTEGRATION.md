# Merging Next2Flash into Flare — Integration Plan

Status: **AS3 track merged; Track 1 re-implemented in Rust** (see the table below)

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

## Recommended approach

**Two tracks, in order:**

### Track 1 — Port the native-friendly pieces (no new runtime)
Improve Flare's existing C++ SWF reader/writer using Next2Flash's parsing logic as
a reference implementation. Targets, in priority order:

1. **Character-ID preservation on export** — port the `char_id_allocator` /
   `swf_patcher` strategy into `tflash` so re-exported SWFs keep working.
2. **Fuller SWF tag coverage** in `extractSwfBitmaps` / `FDT*` (shapes, sprites,
   placements) cross-referenced against `swf-parse-worker.js`.
3. **Bitmap pass-through** (`bitmap_converter.py`) so embedded images survive a
   round-trip byte-for-byte.

These are pure C++ changes, keep the zero-dependency promise, and directly move
the needle on issues #16 and #47.

### Track 2 — AS3 round-tripping as an *optional* bridge
A native C++ ABC decompiler is a large project. Next2Flash already has a working
Python one. Rather than reverse the no-runtime decision globally, expose AS3
features through an **optional, auto-detected helper**, exactly like Flare already
treats FFmpeg:

- Ship the `app/as3_decompiler/` Python package as an optional `flare-as3` sidecar.
- Flare calls it over a small CLI/JSON boundary (it already has a `cli.py`).
- If the helper isn't present, AS3 import/export is simply greyed out — core FLA
  import still works with no Python at all.

This gives users N2F's AS3 power immediately without forcing the dependency, and
buys time to port the decompiler to C++ later if desired.

## Licensing

Next2Flash is **MIT**; Flare is **Modified BSD (3-Clause)**. MIT code can be
incorporated into a BSD project provided the MIT copyright notice is preserved.
Any ported file or bundled sidecar must keep Next2Flash's `LICENSE` and a header
note. The bundled **Flex SDK** is Apache-2.0 and must ship with its own NOTICE.

## Status of the plan

- [x] Vendor the AS3 decompiler under `tools/flash/next2flash/vendor/` so the
      Python helper can be packaged optionally. Provenance recorded in
      `vendor/as3_decompiler/VENDORED.md`.
- [x] Define the `flare-as3` CLI contract (one JSON object on stdout:
      `status`, `decompile`, `patch`, `compile`).
- [x] Wire the optional "ActionScript (via Next2Flash)" path into
      `flashimport.cpp`, detected like FFmpeg.
      `common/flash/As3Bridge.{h,cpp}` + the SWF branch of the import dialog.
      Verified: 1043 AS3 classes recovered from a 3.5 MB SWF.
- [x] Credit Next2Flash in `README.md` and `doc/FLASH_SUPPORT.md`.
- [x] Add bridge tests (`tools/flash/tests/test_as3_bridge.py`).
- [x] Character-ID preservation (Track 1.1), done in Rust instead of `tflash`:
      `common/flash/ac_swf*.rs`. See the table.
- [ ] Port `app/swf_to_n2d.py` and `app/compile_n2d.py` natively (Track 1.2).
      These two are ~400 KB of SWF<->XML conversion and are where most of
      Next2Flash's remaining value sits; see "What is still upstream-only" in
      `tools/flash/next2flash/README.md`.
- [ ] `compile`: blocked only on the Flex SDK `mxmlc` toolchain. The patcher
      side (`abc_patcher.transplant_class`, `swf_patcher.recompile_class`)
      is already vendored and reachable.

> This plan keeps Flare installable with zero extra runtime for the common case
> (open a FLA, get the art), while still delivering Next2Flash's AS3 superpowers to
> users who opt in — and leaves a clean path to fully native C++ later.

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
reported as a warning; the AS3 sidecar path above is unchanged); graphic-symbol
playback options are exported as looping sprites; ZWS (LZMA) SWFs are refused;
mask/guide layers are skipped; a DefineBitsJPEG (tag 6) is passed through as
the spliced JPEG, not as a tag-6 record. The output has been checked only by
this crate's own reader (no Flash Player / Ruffle in the build sandbox); the
generated TrueType was checked with FreeType (`fc-scan`).
