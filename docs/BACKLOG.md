# Reference backlog

What the absorbed and upstream projects have, what Flare on `main` lacks, and the work packages
that close the gaps that matter most for an Adobe Animate replacement. The summary is in
[ROADMAP.md](ROADMAP.md) (Reference backlog). This file is the evidence behind it.

Checked on 2026-11-04 against `origin/main` (commit 5026a07). Gaps were confirmed by grepping
`crates/` and `apps/` for each feature's name and by reading [ROADMAP.md](ROADMAP.md),
[SURFACES.md](SURFACES.md) and `crates/editing/flare-tools/src/not_yet.rs`. A gap marked
"not found" means no matching name in the code, not a proof of absence.

## Licences

Flare is `BSD-3-Clause` (root `LICENSE`, `Cargo.toml` `license`). A source can feed Flare
code only if its licence allows it to be copied into a BSD-3 project with its notice kept.

| Source | Licence (file read) | Compatible with Flare | Use |
|---|---|---|---|
| Flare-RustBase (`Flare-Animate/rustconversion-animatecraft`, the AnimationCraft code) | `MIT OR Apache-2.0` (`LICENSE-MIT`, `LICENSE-APACHE`) | Yes, with the notices kept | Port allowed, with attribution |
| ArtCraft (`storytold/artcraft`) | `MIT OR Apache-2.0` (`LICENSE-MIT`, `LICENSE-APACHE`, `NOTICE`) | Yes, with notices | Port allowed, with attribution |
| OpenToonz (`opentoonz/opentoonz`) | `BSD-3-Clause` (`LICENSE.txt`); `thirdparty/` and `stuff/library/mypaint brushes/` keep their own licences | Yes, same licence family | Port allowed, notices kept, third-party files checked one by one |
| Tahoma2D (`tahoma2d/tahoma2d`) | `BSD-3-Clause` (`LICENSE.txt`), same third-party carve-outs | Yes | Port allowed, same rule as OpenToonz |
| Old Flare C++ (`original/`, this repo) | `BSD-3-Clause` (root `LICENSE`) | Yes | Read as reference only: [AGENTS.md](../AGENTS.md) forbids depending on, linking or copying from `original/` |
| Next2Flash (`SSF2-Mods-Official/Next2Flash`) | `MIT` (`LICENSE`); its editor is based on the Next2D NoCode Tool, `MIT` (`app/LICENSE`); `toolchain/ffdec-worker` is `GPLv3` (bundles FFDec) | Yes for the MIT parts; no for the GPLv3 worker | MIT parts: port allowed. FFDec and its worker: behaviour only, no code, no bundling |
| minnamate (`mirvoxtm/minnamate`) | No licence file in the repo (root listing, `README.md`, `Cargo.toml`, `CLAUDE.md` checked). Its `CLAUDE.md` says it is clean-room and that MIT/Apache inputs may be reused, which is about its inputs, not its own licence | Unknown: with no licence, all rights are reserved | Behaviour only until the maintainer licenses it |
| nflare (`Flare-Animate/nflare`) | `GPLv3` (full text in `LICENSE`); a fork of nanofl | No: BSD-3 Flare cannot take GPLv3 code without relicensing | Behaviour only, no code |
| ArtCraft's `Replicraft` | Not found (see below) | Unknown | Not used |
| bitmapflow (`Bauxitedev/bitmapflow`) | **No LICENSE file** in the repository; its README says MIT, which is not relied on | Unknown | Behaviour and ideas only, no code (ENGINEERING.md) |
| godot_moho_importer (`jpdurigan/godot_moho_importer`) | `MIT` (`LICENSE`) | Yes, with attribution | Format facts used with attribution (see Sources 9) |
| moho-file-analyzer (`sinahojati/moho-file-analyzer`) | No LICENSE file | No (no permission to copy) | Behaviour only, no code |
| fla-decoder (`eddiemoore/fla-decoder`, binary FLA notes and a Python decoder) | `MIT` (`LICENSE`, Copyright 2026 Ed Moore; `NOTICE` repeats MIT) | Yes, with the notice kept | Port allowed with attribution (see [FLASH-FORMATS.md](FLASH-FORMATS.md)); nothing ported yet |
| fla-viewer (`lifeart/fla-viewer`, TypeScript binary FLA viewer) | `ISC` per `package.json` and the README badge; **no LICENSE file** in the repository | Probably yes, but not confirmed from a licence file | Facts and behaviour only, no code. Its fixtures are used locally, not committed |
| Ruffle (`third_party/ruffle`, including `swf` and the test movies under `tests/` and `swf/tests/swfs`) | `MIT OR Apache-2.0` (`LICENSE-MIT`, `LICENSE-APACHE` in `swf/`) | Yes, with notices | The `swf` crate is a dependency; test movies are read in place, not copied |
| JPEXS Free Flash Decompiler | `GPLv3` (not fetched in this session) | No | Facts only, if ever; no code |
| eisai-anime-interpolator (`ShuhongChen/eisai-anime-interpolator`) | `AGPL-3.0` (`license.txt`) | No: copyleft, not permissive | Behaviour and ideas only, no code; its neural weights are not used (ENGINEERING.md) |
| flipnote.js (`jaames/flipnote.js`, Flipnote Studio parsers, test samples) | `MIT` (`LICENSE`, Copyright (c) 2018 James Daniel) | Yes, with the notice kept | Port allowed with attribution: the PPM parser was followed in `crates/io/flare-import/src/flipnote/` (ENGINEERING.md). Its KWZ parser is not used yet |
| Flipnote Collective `flipnote-studio-docs` (wiki, Flipnote Studio PPM) | `GPL-2.0` (`LICENSE` text fetched from the repository page) | No: GPL | Facts only; no text or code copied |
| Flipnote Collective `flipnote-studio-3d-docs` (KWZ format wiki) | `CC BY-SA 4.0` (`LICENSE`) | No: share-alike does not fit a BSD-3 code base | Facts only; no text copied |
| Hatenatools (`pbsds/Hatenatools`, Flipnote Hatena and Studio tools) | `AGPL-3.0` (README) | No: copyleft | Not read for code; nothing used |
| Ugomemo.NET (`purifetchi/Ugomemo.NET`, a different format, Ugomemo) | `MIT` (repository licence) | Yes | Not used: not a Flipnote format |
| sudomemo-utils (`Sudomemo/sudomemo-utils`) and flipnote-id (`pummelator/flipnote-id`) | `MIT` (repository licence) | Yes | Not read for code; listed as leads for the KWZ and FSID work |

Policy the work packages follow. [ARCHITECTURE.md](ARCHITECTURE.md) already marks `flare-tween`
as clean-room ("the code follows the feature descriptions and standard easing formulas"), and
[ENGINEERING.md](ENGINEERING.md) says reference projects are not committed. So the default for
every package below is **behaviour-only, written fresh**: read the reference to learn what the
feature does, then write Flare's own code. A package may port a file from a permissive source
(MIT, Apache or BSD) only when the maintainer records that decision in `docs/decisions/` with
the source, its licence and the changes. This is a recommendation for the maintainer to confirm.

## Sources

### 1. Flare-RustBase: AnimationCraft (`/home/user/flare-animate/rustconversion-animatecraft`)

The Rust implementation of the Animate authoring model that Flare absorbed (`original/docs/ANIMATIONCRAFT_ABSORPTION.md`
calls it "AnimationCraft"; its README title is "Flare (Rust)"). The largest feature source here.
Its README, the part read in this session, lists:

- Tools: Free Transform with skew, Distort and Envelope; 3D Rotation and Translation; Bone with
  inverse kinematics; Asset Warp; Width; Magic Wand and Break Apart; Paint Brush modes; Fluid
  Brush; Gradient Transform; Camera and a free camera (VCam) that follows a named object.
- Files: `.acx` (a ZIP of JSON parts and PNG), XFL and `.fla` import and export with custom eases,
  PNG, SVG, animated GIF, PNG sequence, video through ffmpeg, Publish Settings, HTML5 Canvas,
  sprite sheets (PNG with JSON hash, JSON array or Starling XML), animated SVG (SMIL), Lottie,
  Photoshop (PSD) import, XDTS timing sheet import and export, SVG import, images (PNG, JPEG,
  GIF, BMP, TGA, TIFF, WebP, OpenEXR), numbered image sequences.
- Effects: Motion Blur along real movement; about 200 EffectCraft effects; Trace Bitmap with a
  centerline mode; Shift and Trace.
- From OpenToonz: stroke styles (Herringbone, Chain, Rope, and more), deformers (Magnet, Pinch,
  Pump, Bender, Iron), drawing assistants (line, vanishing point, ellipse, perspective, fisheye,
  replicators, jitter), 25 MyPaint raster brushes, IK with joint constraints, particle emitter.
- Motion Editor with expressions, eases, motion tweens, shape hints, reverse frames, onion skin,
  layer parenting, scenes, buttons (Up, Over, Down, Hit), instance names, sound and lip sync.
- Scripting: frame scripts in JavaScript (sandboxed), a Script Console (JavaScript, Python,
  Node.js or Ruby over a JSON protocol) and JSFL compatibility. Crates: `ac-core`, `ac-render`
  (with `effectcraft.rs`), `ac-script`, `ac-xfl`, `ac-audio`, `ac-package`, `ac-app`, `ac-cli`.

### 2. ArtCraft (`storytold/artcraft`, found by `git ls-remote`)

A generative-media creation app: image (16 models), video (25), music (5), 3D mesh (11) and
Gaussian-splat (5) providers, with crates `desktop`, `api_clients`, `lib`, `schema`. Its
EffectCraft, PhotoCraft (`photocraft-psd`) and VectorCraft crates are the parts AnimationCraft
uses (`original/LOGIC-ATLAS.md`). Only its README headings were read here. Issue #207 on
Flare asks to share its UI modules. The AI generation features are out of scope for Flare.

### 3. nflare (`Flare-Animate/nflare`, found by `git ls-remote`)

A fork of nanofl: a Haxe engine (`engine/nanofl`, with `FilterDef.hx`, `Ease.hx`,
`TextureAtlasTools.hx`, `MaskTools.hx`) and a TypeScript IDE (`ide`, `modules/`). Its README
lists: edges, polygons and boolean operations; png, jpeg and svg rasters; masks, classic tweens
and motion guides; blur, drop shadow, glow, tint and brightness; linear, radial and bitmap
gradient fills; canvas text with web fonts; HTML5 output on EaselJS; 3D through three.js with
glTF and Blender conversion. Licence GPLv3: behaviour only.

### 4. Next2Flash (`SSF2-Mods-Official/Next2Flash`, Windows)

An Electron and Python editor for Flash and AIR projects. It imports a SWF to an editable
`.n2d` project or a folder of PNG, WAV and AS files, and exports a SWF back. Other features: a
timeline, layers, library, transform and colour tools, filters; AS3 decompile and recompile with
a bundled Flex lite SDK (`mxmlc`, `compc`, `asconfigc`); SWF to CS6 FLA export; in-place SWF
patching through FFDec; the `n2f` command line (`n2f.py`, `server.py`). Its editor is in
`app/src` and `app/electron`. Licence: the MIT parts are portable; the FFDec worker is GPLv3.

### 5. minnamate (`mirvoxtm/minnamate`)

A clean-room Rust editor for Animate `.fla` files, with crates `core` (`minnamate-core`), `apps/cli`
and `apps/editor` (egui). It reads and saves FLA and XFL, keeping unknown attributes in `Extra`.
It draws with tiny-skia, publishes SWF through Ruffle's `swf` crate, writes HTML5 Canvas and
renders GIF, PNG and video (ffmpeg). Its model covers folders, masks, guides and parenting,
motion and shape tweens (`motion.rs`, `shape_tween.rs`), buttons (`button.rs`), gradients,
bitmaps, filters and blend modes (`filters.rs`), sound (`sound.rs`), layer tree (`layer_tree.rs`),
and a frame-script subset (`script.rs`: stop, play, goto, next and previous frame, trace,
navigateToURL). Not yet, by its README: bones, 3D, Asset Warp, input text, several documents, custom
workspaces. Its layout follows Animate 2024's Essentials workspace.

### 6. OpenToonz (`opentoonz/opentoonz`)

The C++ base of Flare's old code (about 105 tool sources in `toonz/sources/tnztools/`). Relevant
areas: `magnettool.cpp`, `irontool.cpp`, `pinch`, `pump` and `bender` tools; `assistant.cpp` and
`assistants/` (line, ellipse, perspective, vanishing point, fisheye); `shifttracetool.cpp`;
`mypainttoonzbrush.cpp` with its brush library in `stuff/library/mypaint brushes/`; `stopmotion`;
`tcleanupper` (cleanup); `toonz/sources/toonzqt/` (widgets); `stuff/profiles/layouts/fxs/`
(about 160 FX definitions) and `stuff/library/shaders/`. Flare's `flare-profile` already reads
room layouts (ROADMAP, OpenToonz feature areas).

### 7. Tahoma2D (`tahoma2d/tahoma2d`)

An OpenToonz fork with the same tool layout. Only its licence and README sections were read in
this session; its animation-specific changes are not catalogued here. Treat it as a second copy
of the OpenToonz tool sources to compare when a package is started.

### 8. Old Flare C++ (`original/`, in this repo)

Per `original/FEATURE_STATUS.md`: Moho projects (structure and rig read, no rig rendering);
a FlipaClip layout profile; the Next2Flash sidecar (AS3 decompile, constant-string patch,
compile through the Flex SDK); a game-creation scaffold (`common/tgame`, an ECS with 7 systems);
a Rust backend behind a C ABI; an Android scaffold; an NSIS installer. `original/docs/FLASH_CC_GAPS.md`
and `original/docs/animatecraft/FUNCTION-SOURCES.md` list more. Read only.

### 9. Flipnote Studio formats (PPM, KWZ)

The reference list for `docs/FLIPNOTE.md`. Each source's licence is in the table above.

- flipnote.js (MIT): `src/parsers/PpmParser.ts` and `KwzParser.ts`. The PPM and KWZ readers are
  ported with attribution. The seven real `.ppm` and eight `.kwz` files in its `test/samples/` were used as
  verification data outside the repository (`FLARE_FLIPNOTE_SAMPLES`); they are not committed.
- Flipnote Collective `flipnote-studio-docs` (GPL-2.0): the PPM wiki, read for layout facts.
- Flipnote Collective `flipnote-studio-3d-docs` (CC BY-SA 4.0): the KWZ wiki, read for layout facts.
- Hatenatools (AGPL-3.0) and Ugomemo.NET (MIT): the second is a different format and not used.
- No `.fpbm` format was found in the search. `.kwc` files are KFH sections like KWZ; the sample
  `comment.kwc` is refused with the KWZ message.
- Open work: the KWZ and PPM sound (ADPCM decodes exist, no document layer), the thumbnails, the
  KWZ layer depths and draw order, and an oracle comparison of decoded pixels with flipnote.js
  (refused in this run).

### 10. Moho and Anime Studio (`flare-moho`)

| Source | Licence (file read) | Use |
|---|---|---|
| jpdurigan/godot_moho_importer (`godot_moho_importer`) | `MIT` (`LICENSE`, 2022 João Pedro Durigan) | Format facts (layer type names, mesh and bone key names, track names) with attribution in ROADMAP; its `CharWalk.moho` and `CharWalk.mohoproj` used as real samples, outside the repo (not committed) |
| sinahojati/moho-file-analyzer | **No LICENSE file** (README and `.ps1` only) | Behaviour only, no code; not a source of format facts that are copied |
| SimplSam/moho-bvh-bones | `CC BY-NC-SA 4.0` | Behaviour only (ENGINEERING.md); no code |
| Web search (`Moho Lost Marble Project.mohoproj`) | Not applicable: only file-extension directories and a forum thread; no open-source parser or JSON schema found | Nothing taken |

The real Moho 13 files and their layout are recorded in [MOHO.md](MOHO.md). Samples are not
committed; the tests use synthetic files written to match the layout.

### Replicraft: not found

Tried: `gh search repos replicraft` (HTTP 502 from the GitHub search API, same for `nflare`
and `artcraft`); `git ls-remote` on `storytold/replicraft`, `storytold/Replicraft`,
`Replicraft/Replicraft`, `replicraft/replicraft`; the same names under `ArtCraft/`,
`artcraft-ai/` and `Flare-Animate/`; a grep of `docs/`, `original/`, `README.md` and
`PORTING_TRACKER.md` for "replicraft" (no match). Next step: the maintainer gives the URL.

## Gap list against `main`

Status: **Built** (in the code, tested), **Partial** (a library or part exists), **Gap** (not
found), **Not planned** (ROADMAP or PARITY says so).

| Feature | Found in | Status on main | Evidence |
|---|---|---|---|
| Merge drawing (planar map, shapes cut and merge) | AnimationCraft | Gap | ROADMAP high priority; only a drawing-mode switch in `flare-animate` `panels/drawing_mode.rs` |
| Motion Editor and custom eases | AnimationCraft (`motion.rs`, `tween_edit.rs`), minnamate motion tweens | Partial | Segment editor built (`flare_tween::edit`, Motion Editor panel); per-property graphs left (ROADMAP, SURFACES) |
| Bones and inverse kinematics | AnimationCraft (`rig.rs`, `ik.rs`), OpenToonz skeleton | Gap | `bone` appears only in tool slots, `flare-moho`'s model and a test; no IK |
| Trace Bitmap, centerline | AnimationCraft (`trace.rs`, `centerline.rs`) | Gap | only a stub in `flare-tools/src/not_yet.rs` |
| Shift and Trace | AnimationCraft, OpenToonz `shifttracetool.cpp` | Gap | listed in `not_yet.rs`, not implemented |
| Deformers: Magnet, Pinch, Pump, Bender, Iron | AnimationCraft (`deform.rs`), OpenToonz | Gap | listed in `not_yet.rs` |
| Stroke styles (Herringbone, Rope, Zigzag, and Animate's Dashed, Dotted, Ragged, Stipple, Hatched) | AnimationCraft (`strokes.rs`), OpenToonz | Partly | `StrokeStyle` (solid, dashed, dotted, stipple, hatched, ragged, herringbone, rope, zigzag) is on every stroke, drawn by `flare-render` (the geometric ones in `pattern.rs`), saved in `.flare`, set in Properties and the tool options, and by `draw-rect --stroke-style` and `draw_rect`'s `stroke_style`. Deformers are still open |
| Drawing assistants, perspective, vanishing point | AnimationCraft (`assist.rs`), OpenToonz `assistants/` | Gap | "Assistant tool" and "Edit assistant tool" in `not_yet.rs`; `flare-animate/src/workspace/assist.rs` exists and was not read, so check it first |
| Raster (MyPaint) brushes, art brushes | AnimationCraft, OpenToonz | Gap | "MyPaint Toonz brush tool" in `not_yet.rs`; ROADMAP lower priority |
| Particles | AnimationCraft (`particles.rs`) | Gap | only an i18n string matches |
| Drop shadow, glow, blur, motion blur, colour filters | AnimationCraft (`fx.rs`), nflare, minnamate (`filters.rs`) | Gap | "drop shadow", "glow", "motion blur" match nothing |
| EffectCraft effects (about 200) | AnimationCraft (`effectcraft.rs`) | Gap | none found; a git dependency on ArtCraft crates would be needed |
| Import SVG | AnimationCraft | Gap | "svg" appears only in icon build code and the HTML5 publisher; ROADMAP high priority |
| Import Photoshop PSD | AnimationCraft, ArtCraft (`photocraft-psd`) | Gap | no match; ROADMAP high priority ("artwork from Photoshop") |
| Import WebP, TIFF, TGA, OpenEXR | AnimationCraft | Gap | no match |
| XDTS timing sheet import and export | AnimationCraft (`xdts.rs`) | Gap | no match |
| Sprite sheets (JSON hash, array, Starling XML) | AnimationCraft | Gap | no match; ROADMAP lower priority (sprite export) |
| Animated SVG (SMIL) and Lottie JSON | AnimationCraft | Gap | no match |
| Editable keyboard shortcuts and shortcut sets | AnimationCraft (keyboard section), OpenToonz `shortcuts/*.ini` | Partial | `ShortcutMap` (`flare-menubar/src/shortcuts/`) holds four sets, conflict checks and a validated file; the Keyboard Shortcuts window edits it, tool letters follow it; Ctrl/Alt chords are still fixed key handling in `furniture.rs` and the macOS native menu |
| Scripting: sandboxed frame scripts, Script Console, JSFL | AnimationCraft (`ac-script`), minnamate frame-script subset | Gap | not built; ROADMAP lists ActionScript 3 as lower priority |
| Lip sync (mouth-shape workflow) | AnimationCraft | Gap | no match; ROADMAP lower priority |
| Shape hints | AnimationCraft | Partial | `flare-eval/src/morph.rs` mentions shape hints; ROADMAP says not done |
| Free Transform Distort and Envelope | AnimationCraft | Gap | no match for Distort or Envelope in the code |
| Gradient Transform tool | AnimationCraft, nflare | Partial | gradient fills exist (50 files); the tool is ROADMAP lower priority |
| Find and Replace | AnimationCraft (not checked), ROADMAP lower priority | Gap | dock pane `Pane::FindReplace` (`flare-dock/src/lib.rs`) and an empty `find_replace_panel` stub (`flare-animate/src/workspace/panels/info.rs`); no behaviour |
| Stop motion camera capture | OpenToonz `stopmotion`, `flare-stopmotion` | Partial | `NoCamera` refuses by name (SURFACES) |
| Toonz import (`.tnz`, `.pli`, `.tlv`) | OpenToonz | Partial | `flare-toonz` reads headers; levels not decoded (ROADMAP) |
| Moho import | `original/`, Flare `flare-moho` | Partial | parse and conversion done, image layers included; bone-bound layers keep their own placement, actions not converted ([MOHO.md](MOHO.md)) |
| Xsheet, FX schematic, Plastic, cleanup, render farm | OpenToonz, Tahoma2D | Not planned or P2/P3 | ROADMAP OpenToonz feature areas |
| Camera, VCam, layer parenting, masks, onion skin, buttons | AnimationCraft, minnamate | Built (buttons: Up state only, ROADMAP item 2) | ROADMAP, SURFACES |
| 3D rotation and translation, glTF | AnimationCraft, nflare | Not planned | out of scope in ROADMAP for now |

## Work packages

Each package is independent: it can be a branch of its own. Sizes are rough: S under a week of
focused work, M one or two weeks, L several weeks, XL a milestone. Priority combines value to an
Animate user (ROADMAP's rule: how much a person animating with Flare misses it) with effort. The
list is in the order to start.

Surfaces follow [SURFACES.md](SURFACES.md): a feature is not done until desktop, browser,
`flare` (CLI) and `flare-mcp` have it, or the gap is written down there with its reason. "Test
approach" names the library tests (with failure paths) and the wiring tests the rule needs.

### WP-1 Editable keyboard shortcuts (S, high)

- **Goal:** the shortcut table can be edited and saved, with named sets (`flare`, `animate`,
  `opentoonz`, which profiles name).
- **Read in reference:** AnimationCraft README "Keyboard" section; OpenToonz `stuff/config/shortcuts/`
  (the `.ini` set format, BSD).
- **Target:** `flare-input` (foundation: key parsing and maps) and `flare-menubar` (presentation:
  the table), with settings stored in `flare-recent`'s neighbour config, whichever fits.
- **Surfaces:** desktop yes, browser yes (same workspace); CLI and MCP n/a (no keys).
- **Test approach:** map round trip, conflict detection, a rejected duplicate leaves the map
  unchanged, a corrupt file falls back to the default set with a message.
- **Licence:** OpenToonz format and AnimationCraft behaviour are portable; write the parser fresh.

### WP-2 Motion Editor and tween curves (M, high)

- **Goal:** per-property graphs, custom ease curves and a Motion Editor panel, driven by commands.
- **Read in reference:** AnimationCraft README "Animation" section and `ac-core/src/motion.rs`,
  `tween_edit.rs`; minnamate `motion.rs` for XFL custom eases.
- **Target:** `flare-tween` (foundation, built), new commands in `flare-commands` (editing:
  `SetKeyframeInterpolation`, `CreateInbetweens`, both one undo step each, as ROADMAP's tween
  section lists), the editor panel in `flare-animate` (presentation).
- **Surfaces:** desktop and browser (the panel); CLI `flare tween list|eval`; MCP `tween_eval`
  with the same calls, so SURFACES rows change to yes.
- **Test approach:** command tests with rejected batches, undo back to the exact document,
  `flare-tween` curve tests (exist), a surface test for the CLI and MCP wiring.
- **Licence:** clean-room (ARCHITECTURE: flare-tween is clean-room). Standard easing formulas are public.

### WP-3 Trace Bitmap and Shift and Trace (M, high)

- **Goal:** turn a bitmap into merge-shape fills (colour threshold, minimum area, curve fit,
  corner threshold) and a centerline mode; ghost keyframes that slide under the drawing.
- **Read in reference:** AnimationCraft `ac-core/src/trace.rs`, `centerline.rs`; OpenToonz
  `shifttracetool.cpp`.
- **Target:** a new module in `flare-core` (foundation: the image work) and `flare-tools`
  (editing: the command that places the shapes as one undo step); the ghost display in
  `flare-animate` and `flare-render`.
- **Surfaces:** desktop and browser; CLI `flare trace-bitmap` and MCP `trace_bitmap` with the
  parameters; Shift and Trace is desktop and browser only (`n/a` for CLI and MCP, with a reason).
- **Test approach:** golden tests on small synthetic bitmaps (a square, a ring, line art), a
  bound on the number of points, a failing bitmap (empty, huge) refused with an error.
- **Licence:** AnimationCraft's trace is MIT/Apache; behaviour first, port only with a decision.

### WP-4 Import SVG, Photoshop and more image formats (L, high)

- **Status:** SVG done (`flare-import` `svg.rs`, usvg 0.45 without its text feature; MIT/Apache). Bitmap content kind, PNG and animated GIF import, and `import_image_sequence` are done. Left: JPEG/WebP (decoder dependency), PSD, a CLI/MCP/desktop entry for sequences, bitmap blend modes, SWF/XFL export of bitmaps.

- **Goal:** SVG as editable shapes, PSD as one bitmap layer per Photoshop layer (in place),
  WebP, TIFF, TGA and OpenEXR bitmaps, animated GIF as frames, XDTS timing sheets.
- **Read in reference:** AnimationCraft `ac-core/src/xdts.rs`, `ac-render/src/formats.rs`, its
  README "Import" paragraphs; ArtCraft `photocraft-psd` crate (MIT/Apache) for the PSD reader.
- **Target:** `flare-import` (io) for the readers, `flare-core` for the decoders where a crate is
  the right tool; each format behind a bound on size and pixel count (ROADMAP: input is untrusted).
- **Surfaces:** desktop and browser (File > Import); CLI `flare import` extended; MCP `import_project`.
- **Test approach:** fixtures for each format; corrupt and truncated files refused, never a panic;
  size limits tested at the bound and one past it.
- **Licence:** photocraft-psd and the image crates are MIT/Apache; check each dependency's licence
  before adding it, and record it in `THIRD-PARTY-NOTICES`.

### WP-5 Export: sprite sheets, animated SVG, Lottie, XDTS (M, medium)

- **Status:** sprite sheet (PNG grid plus JSON hash atlas) done in `flare-runtime` `sprites.rs`, CLI and MCP;
  left: desktop and browser entries, trimming and duplicate sharing, animated SVG, Lottie, XDTS.

- **Goal:** the game and web outputs of the references: sprite sheets (PNG plus JSON hash, JSON
  array or Starling XML, trimmed, duplicates shared), animated SVG with SMIL, Lottie JSON, XDTS out.
- **Read in reference:** AnimationCraft export section and `ac-render/src/formats.rs`; nflare's
  `TextureAtlasTools.hx` for the atlas packing idea (behaviour only, GPLv3).
- **Target:** `flare-export` (io) and `flare-publish` (io), with the frame data from `flare-eval`.
- **Surfaces:** desktop (Publish dialog), browser (download), CLI `flare export --format ...`, MCP
  `export_*` tools.
- **Test approach:** golden output files for each format (a file a viewer loads), a frame range
  test at both ends, failure paths for an empty document and an over-limit atlas.
- **Licence:** all formats are public specs (Starling XML, Lottie JSON, SMIL); write fresh.

### WP-6 Deformers and stroke styles (M, medium)

- **Goal:** Magnet, Pinch, Pump, Bender and Iron deformers on drawing objects and merge shapes;
  OpenToonz's extra stroke styles (Herringbone, Chain, Rope, Zigzag, Wave, Curl and the rest).
- **Read in reference:** AnimationCraft `ac-core/src/deform.rs`, `strokes.rs`; OpenToonz
  `magnettool.cpp`, `irontool.cpp`, and the stroke style sources in `toonz/sources/`.
- **Target:** `flare-tools` (editing: the operations, as `ops::` functions and commands) and
  `flare-render` (the stroke pattern).
- **Surfaces:** desktop and browser; CLI and MCP partly (a `deform` command over a shape id;
  stroke styles desktop and browser only, with the reason in SURFACES).
- **Test approach:** operation tests with radius and strength bounds, undo, rejected empty
  selection, a stroke pattern golden image at fixed parameters.
- **Licence:** OpenToonz is BSD-3: porting is allowed with the notice; the default is still
  behaviour-only.

### WP-7 Drawing assistants and perspective guides (M, medium)

- **Goal:** straight-line, ellipse, perspective (one, two and three point), vanishing point and
  fisheye assistants that straighten strokes; replicators (mirror, star, grid, affine); jitter.
- **Read in reference:** AnimationCraft `ac-core/src/assist.rs`; OpenToonz `tnztools/assistants/`
  and `modifiers/modifierassistants.cpp`.
- **Target:** `flare-tools` (the geometry, pure functions), `flare-animate` (the assist panel;
  first read `flare-animate/src/workspace/assist.rs`, which exists).
- **Surfaces:** desktop and browser; CLI and MCP `n/a` for live drawing, with a reason.
- **Test approach:** geometry tests on known inputs (a line snaps to the given vanishing point),
  a degenerate guide (zero length, coincident points) refused, the overlay never enters the export.
- **Licence:** geometry is public; write fresh.

### WP-8 Filters and effects (L, medium)

- **Goal:** an ordered effect stack for objects and groups: drop shadow, glow, blur, colour
  adjust, motion blur along real movement, and then the EffectCraft set (about 200 effects).
- **Read in reference:** AnimationCraft `ac-core/src/fx.rs`, `ac-render/src/effectcraft.rs`;
  minnamate `filters.rs` (kept in the element's `Extra`); nflare's `FilterDef.hx` (behaviour).
- **Target:** `flare-document` (the stack, as data), `flare-core` (pixel work), `flare-render`
  (draw), `flare-publish` (SWF PlaceObject3 and HTML5, where the format can hold it, else a warning).
- **Surfaces:** desktop, browser, CLI (`flare filter add|list`), MCP (`add_filter`).
- **Test approach:** golden images for each filter at fixed parameters; a bound on radius and
  sample count; a stack with a cycle or unknown filter refused; undo.
- **Licence:** EffectCraft is MIT/Apache and a git dependency today; adopting it needs a decision
  on the dependency (size, build time, WebAssembly). The rest is clean-room.

### WP-9 Bones and inverse kinematics (XL, high)

- **Goal:** bones on shapes and symbols, IK chains posed with the Selection tool, poses as
  keyframes, joint limits and pins (ROADMAP's rigging section sets the larger goal).
- **Read in reference:** AnimationCraft `ac-core/src/rig.rs`, `ik.rs`; OpenToonz skeleton and
  `tnztools` skeleton sources (BSD); `flare-moho` for the Moho bone model.
- **Target:** `flare-document` (bones in the model, with a version bump and an upgrade test),
  `flare-commands`, `flare-eval` (the solver), `flare-animate` (the Bone tool). Decision first in
  `docs/decisions/RIGGING.md`, because ROADMAP asks for a rig that is Flare's own.
- **Surfaces:** desktop and browser; CLI and MCP `add_bone`, `pose_bone`; Spine- and DragonBones-style
  formats stay out.
- **Test approach:** solver tests on known chains (two-bone reach, unreachable target leaves
  the chain at its limit), random edit sequences keep the document valid, undo exact.
- **Licence:** IK solvers are public algorithms; write fresh. OpenToonz and AnimationCraft behaviour only.

### WP-10 Timeline and symbol extras (M, medium)

- **Goal:** Find and Replace across text, fonts, colours and symbols; shape hints on shape tweens
  (their evaluation is partly there in `flare-eval/src/morph.rs`); a visual frame picker for a
  graphic's first frame; Convert to Frame-by-Frame; Gradient Transform (F).
- **Read in reference:** AnimationCraft README "Animation" and "Objects"; minnamate `shape_tween.rs`
  for hints.
- **Target:** `flare-commands` and `flare-document` (the operations), `flare-animate` (dialogs).
- **Surfaces:** desktop and browser; CLI `flare replace` and MCP `find_replace` for the text and
  symbol parts; the gradient tool and frame picker are desktop and browser only.
- **Test approach:** replace on a document with no match changes nothing; a replace that
  breaks a symbol name is refused; shape hints with two points and with an unmatched count fail
  with an error; undo.
- **Licence:** clean-room; behaviour from the references.

### WP-11 Raster brush engine (L, medium)

- **Goal:** pixel brushes on a keyframe's bitmap, with MyPaint-style brush presets (pencil, ink,
  charcoal, airbrush, watercolour, smudge, erasers).
- **Read in reference:** AnimationCraft raster brush code and its 25 presets; OpenToonz
  `mypainttoonzbrush.cpp`. Check each brush file's licence in `stuff/library/mypaint brushes/`
  (OpenToonz `Licenses.txt`) before using any preset.
- **Target:** `flare-core` (the brush stamping, bounded by brush size and dab count), `flare-tools`
  (the tool), `flare-animate` (presets panel).
- **Surfaces:** desktop and browser; CLI `flare paint-raster` and MCP as a `draw` call (partly).
- **Test approach:** golden images at fixed stroke input, a stroke's dab count bounded, an
  unreadable preset file refused, and a performance measurement against PERFORMANCE.md (not a
  timing claim without numbers).
- **Licence:** brush presets are the blocker: each preset file needs its own check. Engine written fresh.

### WP-12 Scripting (XL, lower, decision first)

- **Goal:** frame scripts run in a sandbox during playback, a Script Console and a JSFL
  compatibility layer, as AnimationCraft has them; or a decision that ActionScript 3 (ROADMAP's
  lower-priority item) is the only scripting Flare offers.
- **Read in reference:** AnimationCraft `ac-script/src/lib.rs`, `jsfl.js`, its `SCRIPTING.md`
  (the API and the safety model); minnamate `script.rs` (the frame-script subset).
- **Target:** a new crate only when it has code and tests (AGENTS): likely `flare-script`
  (services layer, with a JavaScript engine as an adapter). The decision goes first in
  `docs/decisions/SCRIPTING.md`: sandbox, resource limits, what a script may change (commands
  only, one undo step per run).
- **Surfaces:** desktop and browser (Script Console); CLI `flare script run` and MCP `run_script`,
  which must refuse anything outside the sandbox.
- **Test approach:** the sandbox refuses file and network access by test, a runaway loop hits the
  limit, a script that writes through commands is one undo step, a failed script changes nothing.
- **Licence:** AnimationCraft's API and safety model are MIT/Apache, so a port is allowed; the
  engine choice must have a licence that fits BSD-3.

## Not in this backlog

- Moho rendering, 3D, the Spine and DragonBones formats, Android features beyond what
  [SURFACES.md](SURFACES.md) lists: out of scope per ROADMAP's "Not planned" section.
- Tahoma2D-specific features beyond OpenToonz's tool set were not catalogued (see Sources, 7).
- Replicraft, until its URL is known.
