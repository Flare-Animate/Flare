# Merging Next2Flash into Flare — Integration Plan

Status: **assessment / roadmap** (no code merged yet)

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

## Concrete first steps

- [ ] Add Next2Flash as a git submodule under `thirdparty/next2flash/` (or vendored
      `tools/flash/next2flash/`) so the Python helper can be packaged optionally.
- [ ] Define the `flare-as3` CLI contract (stdin/stdout JSON: `decompile`,
      `compile`, `patch`).
- [ ] Wire an optional "ActionScript (via Next2Flash)" path into
      `flashimport.cpp`, detected like FFmpeg.
- [ ] Begin Track 1.1: port char-ID preservation into `common/flash/tflash`.
- [ ] Credit Next2Flash in `README.md` and `doc/FLASH_SUPPORT.md`.

> This plan keeps Flare installable with zero extra runtime for the common case
> (open a FLA, get the art), while still delivering Next2Flash's AS3 superpowers to
> users who opt in — and leaves a clean path to fully native C++ later.
