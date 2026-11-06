# Gaps against the archived Flare code (`original/`)

What the old C++/Qt Flare tree (`original/`, an OpenToonz/Tahoma fork) has that Flare on `main`
does not, or has only in part. It covers the content in `original/stuff/` (presets, palettes,
brushes, textures, FX presets, translations, shortcuts, themes and rooms), the tools, FX, export
formats and the features of `original/FEATURE_STATUS.md`. The reference projects are in
[BACKLOG.md](BACKLOG.md), and the planned features in [ROADMAP.md](ROADMAP.md) and
[PARITY.md](PARITY.md); this file does not repeat them except where the old tree adds something.

Checked on 2026-11-05 against `origin/main` (commit 4048a479). "Missing" means no matching name
or file in `crates/` and `apps/`, read with `grep` and a directory listing; it is not a proof of
absence. Counts are of files in the old tree, not of features.

## Headline numbers

- **Content (`original/stuff/`)**: 4 of the 8 old shortcut sets are present; 3 of the 5 old room
  layouts are absent or dimmed (AnimateLegacy, FlipaClip, StudioGhibli); 8 of the 10 old QSS
  themes have a match (AnimateDark is missing); 7 of the 11 old interface languages are
  covered before this change, and Russian, Czech and Italian have since been added (rows 5 to 7); the 8 global
  palettes are not shipped (the `.tpl` reader exists); none of the 20 field-guide and paper
  presets are shipped; none of the 271 MyPaint brushes (CC0) can be used without a raster engine.
- **FX**: the old tree has 168 OpenToonz `stdfx` `.cpp` sources: 75 base effects, 47 `ino_*`,
  33 `iwa_*` and 13 `igs_*`. The new tree has an FX parameter model
  (`flare-core/src/fx_params`, 117 lines) and no effect implementation.
- **Blend modes**: the old tree has 22 `ino_blend_*` modes; the new document has none.
- **Tools**: the old tree has 68 tool sources in `tnztools/`; the new tree has 24 tool modules in
  `flare-tools`, and 52 names stubbed in `not_yet.rs` that return `NotYetImplemented`.
- **Export**: the old `tiio` writes BMP, TGA, TIFF, PNG, EXR, SGI, PSD, PLI, SVG and video.
  The new tree writes PNG (`flare-render`), GIF, MP4 (native only), sprite sheets, SWF, HTML5,
  XFL and FLA. JPEG, BMP, TGA and TIFF are missing.
- **FEATURE_STATUS (13 items)**: absent here are the FlipaClip room (2), the Next2Flash AS3
  sidecar (3), the P2P transport (10) and auto-update (11). Partly here are Moho (1), Android
  (6), the installer (7), universal Flash detection (9) and custom layouts (13).

## Table

Size: S under a day, M a few days, L a week or more. Value: how much an Animate user misses it.
Licence: Flare-owned (the old tree's own files, BSD-3 with the root `LICENSE`), OpenToonz (BSD-3
with third-party carve-outs), third-party (its own licence), or unknown. Only the first two may be
ported, with the notice kept and an entry in [ENGINEERING.md](ENGINEERING.md); behaviour can
always be read.

| # | Feature or content | Where in `original/` | State in new code | Size | Value | Licence | Surface notes |
|---|---|---|---|---|---|---|---|
| 1 | Shortcut sets: Adobe, Flash, FlipaClip, Harmony, Retas | `stuff/profiles/layouts/shortcuts/otadobe.ini`, `otflash.ini`, `otflipaclip.ini`, `otharmony.ini`, `otretas.ini` | Partial: 4 sets in `flare-menubar/src/shortcuts/sets.rs` (`flare`, `opentoonz`, `animate`, `tahoma`). The old `.ini` files are mostly empty lines (about 20 real overrides each) | S | Medium (an Animate user expects Animate or Flash keys) | OpenToonz BSD-3 | Desktop and browser (Help > Keyboard Shortcuts). CLI and MCP have no keys: `n/a` |
| 2 | Room layouts: AnimateLegacy, StudioGhibli | `stuff/profiles/layouts/rooms/AnimateLegacy`, `StudioGhibli` | Partial: 4 built-in profiles in `flare-profile/data/` (`flare-default`, `opentoonz-classic`, `tahoma-style`, `animate-style`) | S each | Low | OpenToonz BSD-3 | Profiles are read by `flare-profile`; the switcher is a dimmed button (PARITY) |
| 3 | FlipaClip room | `stuff/profiles/layouts/rooms/FlipaClip` | Missing by decision: PARITY says whole-interface switching is dimmed | S once switching exists | Low to medium (beginners) | OpenToonz BSD-3 | Desktop and browser; the interface switcher is the blocker, not the room file |
| 4 | Colour theme: AnimateDark | `config/qss/AnimateDark` | Partial: 8 themes in `flare-ui/src/theme.rs` (`Mode`: Dark, Light, Clay, Synthwave, Blue, Neutral, Green, LiquidGlass). The old `Default` and `Dark` folders have no separate match; the new `Dark` is the default | S | Low to medium | OpenToonz BSD-3 (verify the QSS header) | Desktop and browser (Preferences > Theme) |
| 5 | Interface language: Russian | `translations/russian/*.ts` (5,096 translated, 155 unfinished) | Done: `localizations/ru` (857 keys; 178 from the `.ts` files, the rest machine-translated and not yet reviewed, see ENGINEERING.md); `docs/translations/README.ru.md` | M | High | OpenToonz BSD-3 | Native-speaker review of the machine-translated messages is open |
| 6 | Interface language: Czech | `translations/czech/*.ts` (4,421 translated, 838 unfinished) | Done: `localizations/cs` (857 keys; 170 from the `.ts` files, the rest machine-translated); `docs/translations/README.cs.md` | M | Medium | OpenToonz BSD-3 | As row 5 |
| 7 | Interface language: Italian | `translations/italian/*.ts` (2,147 translated, 3,340 unfinished) | Done: `localizations/it` (857 keys; 114 from the `.ts` files, the rest machine-translated, since most of the `.ts` strings are unfinished); `docs/translations/README.it.md` | M to L | Medium | OpenToonz BSD-3 | As row 5 |
| 8 | Interface language: Norwegian Bokmål | `translations/norwegian_bokmal` (76 translated, 4,266 unfinished) | Missing; the old file is almost empty | L | Low | OpenToonz BSD-3 | Not worth porting from the old file |
| 9 | Global colour palettes (8 `.tpl`: Cleanup, Raster Drawing, Toonz Raster and others) | `stuff/studiopalette/Global Palettes/Default Palettes/*.tpl` | Partial: `flare-toonz/src/palette.rs` reads `.tpl` palette XML. No palette library is shipped and no palette panel exists (SURFACES: "Level strip and palettes not built") | S to ship, M with the panel | Medium | OpenToonz BSD-3 | Desktop and browser for the panel; CLI `flare palette list` would be `partly` |
| 10 | Field-guide and paper presets (9 `.fdg` peg-hole sets, 11 `.pap` paper sizes) | `stuff/config/fdg/`, `stuff/config/pap/` | Missing: the only size presets are in the stage and home tests | S (data) to M (the guide overlay) | Low to medium (animation paper) | OpenToonz BSD-3 (format facts; write new data) | Desktop and browser; CLI `doc set --paper` |
| 11 | MyPaint raster brushes (25 sets: 271 `.myb` with 271 `.png` previews) | `stuff/library/mypaint brushes/` (`Licenses.txt`: CC0-1.0, MyPaint policy) | Missing: no raster brush engine. The "MyPaint Toonz brush tool" is a stub in `flare-tools/src/not_yet.rs` | L for the engine; S for the presets once it exists | High (painting) | CC0-1.0, attribution kept in `Licenses.txt` | Desktop and browser; CLI and MCP through `draw` (WP-11) |
| 12 | Vector brush and style libraries (40 `.pli` brushes, 137 `.png` and 43 `.pli` custom styles) | `stuff/library/vector brushes/`, `stuff/library/custom styles/` | Missing as a library. Stroke styles exist (SURFACES row "Stroke styles") | M | Medium | OpenToonz BSD-3 (verify each `.pli` header) | Desktop and browser; CLI `flare style list` |
| 13 | Particle presets (107 `.tif`, 45 `.rgb`) | `stuff/library/particles/` | Missing. No particle effect; `flare-game` has its own particle component, which is not this | M, after the FX stack (row 20) | Low to medium | Unknown: no licence file; do not port until checked | Desktop and browser |
| 14 | Brush tips and textures (94 `.png`, 37 `.bmp`) | `stuff/library/textures/`, `brush tips/` | Missing | S as data, once a texture consumer exists | Low | Unknown: no licence file; do not port until checked | Desktop and browser |
| 15 | GPU shader effects (9 `.xml`, 9 `.frag`, 7 `.vert`) | `stuff/library/shaders/` | Missing: no GPU effects | L | Low to medium | OpenToonz BSD-3 (verify) | Desktop and browser; needs a WebGL path in the browser build |
| 16 | Clapperboard image | `stuff/library/clapperboards/` | Missing | S | Low | Unknown (verify) | Desktop and browser (the start screen) |
| 17 | Xsheet PDF presets (3) and xsheet settings | `stuff/library/xsheetpdfpresets/`, `stuff/profiles/layouts/settings/` | Missing; the xsheet is not planned (PARITY) | M | Low | OpenToonz BSD-3 | Not planned for Animate users |
| 18 | Camera calibration presets (2) | `stuff/library/camera calibration/` | Missing | M | Low | OpenToonz BSD-3 (verify) | Desktop and browser, with the Camera tool |
| 19 | Panel templates (24 `.ui`) and layout presets (4) | `stuff/library/custom panel templates/`, `stuff/library/layouts/` | Missing; dockable panels with "Reset layout" replace them (PARITY) | M | Low | OpenToonz BSD-3 | Not planned |
| 20 | FX presets and parameter layouts (162 `.xml` and one list file) | `stuff/profiles/layouts/fxs/`, `stuff/fxs/presets/` (`STD_particlesFx`) | Missing: `flare-core/src/fx_params` is the parameter model only | L (with row 21) | Medium | OpenToonz BSD-3 | Desktop and browser; CLI `flare fx list` |
| 21 | Base effects, OpenToonz `stdfx` (75 base sources: colour, blur, glow, drop shadow, emboss, erode and dilate, cloud, dissolve, gradients, channel mixer, levels) | `original/flare/sources/stdfx/` | Missing: no effect is implemented. Filters are WP-8 in BACKLOG | L as a family; each effect S to M | High (Animate Color Effect, blur, glow, drop shadow) | OpenToonz BSD-3 for most; check each file header | Desktop and browser; CLI `flare filter add` (WP-8) |
| 22 | `igs_*` (13), `ino_*` (47) and `iwa_*` (33) effect families (`ino_blend_*` are 22 blend modes) | `original/flare/sources/stdfx/igs_*`, `ino_*`, `iwa_*` | Missing. The `ino_blend_*` modes are not in the document | S per blend mode (pure formulas); M to wire into the stage | High (blend modes are core to Animate) | Per-family header check. Blend formulas are standard compositing maths: write fresh | Desktop, browser, CLI and MCP once wired into layers (document field, one undo step) |
| 23 | Tools not built: Magnet, Pinch, Pump, Bender, Iron, Plastic, Hook, Cutter, Tracker, Ruler, Type, Replicator, Assistant, Skeleton, Fill variants, RGB picker, Style picker, Tape, full-colour brush, Autofill, Set Save Box, Image Grouping | `original/flare/sources/tnztools/*.cpp` (68 files) | Partial: 24 tool modules built; 52 names stubbed in `not_yet.rs` | Mixed: see BACKLOG WP-6, WP-7, WP-10, WP-11 | Mixed | OpenToonz BSD-3 | Surfaces in BACKLOG work packages; each stub names its source |
| 24 | Scan cleanup (`tcleanupper`) | `original/flare/sources/tcleanupper/` | Missing; stub `cleanup` in `not_yet.rs` | L | Low for Animate, medium for scanned work | OpenToonz BSD-3 | Desktop and browser; CLI `flare cleanup` |
| 25 | Render farm (`flarefarm`) | `original/flare/sources/flarefarm/` | Missing; stub `render_farm` | L | Low | OpenToonz BSD-3 | Not planned |
| 26 | Stop motion camera capture and live view (`stopmotion`) | `original/flare/sources/stopmotion/` | Partial: `flare-stopmotion` has session, onion, export and `NoCamera`, which refuses by name (SURFACES, two rows) | M per platform | Medium | OpenToonz BSD-3 | Desktop and browser; CLI `stop-motion-capture` exits with a named error |
| 27 | Sound: browser playback and sound columns | `original/flare/sources/sound/` (`ac_audio`, `ffmpeg`, `aiff`) | Partial: `flare-audio` (mixer, resampler) and `flare-publish` sound. The browser has no sound output (SURFACES) | M | Medium | OpenToonz BSD-3; the ffmpeg code is LGPL and is not copied | Browser needs Web Audio |
| 28 | Still-image export: JPEG, BMP, TGA, TIFF, and PSD (write) | `original/flare/sources/image/bmp`, `tga`, `tif`, `psd`, `png`, `sgi`, `exr` (through `tiio`) | Partial: PNG, GIF, sprite sheet, and now JPEG, BMP and TGA in `flare-render` (`StillFormat`), CLI and MCP; desktop and browser still PNG and GIF. TIFF and PSD are missing | S for TIFF (the `image` crate `tiff` feature, which pulls its own dependencies) | High (JPEG is a standard Animate export) | `image` crate: MIT or Apache-2.0. The old `tiio` is OpenToonz BSD-3 | Desktop Export dialog, browser download, CLI `flare export --format`, MCP `export_*` |
| 29 | Video export: AVI, MOV, 3GP, and FFmpeg-based writers | `original/flare/sources/image/avi`, `mov`, `3gp`, `ffmpeg` | Partial: MP4 native only (`flare-video`) | M | Medium | FFmpeg is LGPL: use a process call, not a link | CLI and MCP `export_mp4`; browser `n/a` |
| 30 | Scene batch converter (`tconverter`) | `original/flare/sources/tconverter/tconverter.cpp` | Missing as a batch: `flare export` converts one document | S | Low to medium | OpenToonz BSD-3 | CLI only (a folder loop over `flare export`) |
| 31 | Scripting and plugins: OpenToonz plugin SDK samples (blur, geom, multiplugin) | `original/plugins/` (`blur`, `geom`, `multiplugin`, `utils`) and `stuff/plugins/` | Missing; PARITY says ActionScript is not run. No plugin ABI | L | Low | OpenToonz BSD-3 | Not planned for 0.1 |
| 32 | Help and manuals (Japanese and Spanish manuals with images, FX reference pages per effect) | `original/stuff/doc/` (206 files; `日本語/`, `español/`, `LICENSE/`) | Partial: in-app Help menu and `docs/`. No manual | M to write fresh | Medium | Manuals are not under a licence in the tree: read for facts only, do not copy | Desktop and browser (Help) |
| 33 | Workspace profiles from user files (`.ini` rooms, `tools/layout_tool.py`) | `stuff/profiles/layouts/rooms/`, `original/tools/layout_tool.py` | Partial: `flare-profile` reads and checks a profile; the surface row says no surface lists or loads user profiles (SURFACES row 75) | S to M | Low to medium | OpenToonz BSD-3 for the format; the tool is Flare's | Desktop and browser, CLI `profile check` |
| 34 | Moho rig rendering (FEATURE_STATUS 1) | `original/flare/sources/common/moho/` (structure read only) | Partial: `flare-moho` reads structure; `flare-rig` solves bones and IK. No rig deformation of Moho artwork | M | Medium | MOHO.md (format facts from MIT sources) | Per BACKLOG Moho row |
| 35 | Next2Flash AS3 decompile, constant patch, transplant (FEATURE_STATUS 3) | `original/flare/sources/common/flash/ac_as3_*.rs`, `common/flash/` | Missing: no `ac_as3` or Next2Flash code in `crates/`. PARITY: "AS3 decompile / ActionScript editing: not planned for 0.1" | L | Low for Animate users; a decision for the maintainer | Own code (BSD-3); Next2Flash is MIT for the editor, FFDec is GPLv3 | Decision needed; not started |
| 36 | `.sol` shared object reader and writer (FEATURE_STATUS 9) | `original/flare/sources/common/flash/lso.rs` (from `rust-flash-lso`, MIT) | Missing; PARITY says "Planned: belongs in `flare-flash`". Another agent works in Flash import | S | Low to medium | MIT (`rust-flash-lso`), keep notice | Desktop and browser through File > Import; leave to the Flash work |
| 37 | Flash game scaffold (`tgame`, FEATURE_STATUS 4) | `original/flare/sources/common/tgame/` | Done elsewhere: `flare-game` (another agent) | n/a | n/a | n/a | Not duplicated here |
| 38 | Android build (FEATURE_STATUS 6) | `original/flare/android/` | Partial: `flare-android` crate and PORTING_TRACKER say a first version exists | n/a | n/a | n/a | Not duplicated here |
| 39 | Windows installer (NSIS, FEATURE_STATUS 7) | `original/packaging/windows/installer.nsi` | Partial: `packaging/windows.wxs` (WiX) and `cargo-packager.json`; NSIS not used | S to decide | Low | Own | Maintainer decision: keep WiX or port NSIS (RELEASE.md) |
| 40 | Auto-update with SHA-256 (FEATURE_STATUS 11) | `original/flare/sources/flareqt/flareupdater.cpp` | Missing; PARITY: "Planned after the first release exists" | M | Medium | Own | Desktop only |
| 41 | P2P remote control (FEATURE_STATUS 10) | `original/flare/sources/remote/` (protocol and tests only) | Missing (PARITY: not planned) | L | Low | Own | Not planned |

## Why these, in this order

The first slices are the rows that are small, use code already in the workspace, and have an
Animate user waiting on them: JPEG and the other still-image formats (row 28), the Russian
interface (row 5), and the Adobe and Flash shortcut sets (row 1). The FX and blend-mode rows
(21 and 22) are the largest value and are the next package after the pixel primitives exist.
Rows 9 and 10 are data-only and can follow any slice. Rows 13 and 14 wait for their consumer.

## Not duplicated here

Flash and FLA import (agent ab15), bitmaps and effects on FLA import, the game engine
(`flare-game`), the stroke selection and frames fix, the tween shear fix, pen pressure, the
FlipaClip, Flipnote, Moho and Toonz importers, ViewSpace, and the nightly CI are other work in
progress. Rows 36 and 37 point at them and do not start them.

## Method and limits

- Read: `original/FEATURE_STATUS.md`, `original/docs/FEATURES.md`, the directory listings of
  `original/stuff/` and `original/flare/sources/` (counts only, not contents), the licence files
  under `original/stuff/library/mypaint brushes/` and `original/doc/LICENSE/`, and the new code's
  `not_yet.rs`, `flare-i18n/src/lib.rs`, `flare-menubar/src/shortcuts/sets.rs`,
  `flare-profile/src/builtin.rs`, `flare-ui/src/theme.rs` and `docs/SURFACES.md`.
- Not read in full: the file headers of most `stdfx`, `ino_*`, `igs_*` and `iwa_*` sources, and
  the `.ts` translation contents beyond counts. The licence column is therefore "verify" wherever
  a header was not read; the default is behaviour only (ENGINEERING.md).
- The FX and blend rows were counted from file names, not run.
- `docs/SURFACES.md` lists the stop-motion rows twice (camera capture and session settings).
  This is fixed in the same commit as this file.
