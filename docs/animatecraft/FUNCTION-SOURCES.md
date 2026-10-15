# Flare: functions, sources and references

This lists Flare's features, where each one lives in the code, and **where it came from**.
Use it with `THIRD-PARTY-NOTICES.md` (licences and notices) and `CONTRIBUTING.md` (clean-room
policy). Updated Oct 15 2026, at commit `e2a6aa3`.

**Origin key**
- **Own:** written for this project. Any behaviour reference is public documentation or what the
  app visibly does (clean-room).
- **Port:** code translated from another open-source project. The file header names the
  source, and the BSD-3-Clause notice is kept where it applies.
- **Approach:** the method follows another project's design, but the code is our own.
- **Dep:** a library linked as a dependency (crates.io or git), not copied into this repo.
- **Ext:** an external program, called only if the user has it installed. Nothing of it is bundled.
- **Algo:** a standard published algorithm.

**Behaviour reference for Animate-compatible features**
- Adobe's public user guide and JSFL/CreateJS documentation.
- What Animate visibly does.
- `.fla`/XFL files we wrote ourselves or have permission to use.

Never used: Adobe code, binaries, icons or sample files (see `CONTRIBUTING.md`).

---

## 1. Core engine (`crates/ac-core`)

| Function | Code | Origin | Source / reference |
|---|---|---|---|
| Document model (Document → Timeline → Layer → Keyframe → Element, Library) | `model.rs` | Own | Mirrors the public XFL DOM structure (`DOMDocument`, `DOMFrame`, `DOMSymbolInstance`) |
| Geometry: Flash-style affine matrices, paths, Bezier splitting, simplification | `geom.rs` | Own + Algo | de Casteljau subdivision; Ramer–Douglas–Peucker simplification |
| Timeline evaluation (pure `evaluate(timeline, frame)`), graphic/movie-clip clocks | `lib.rs`, `model.rs` | Own | Animate's documented symbol behaviour (Loop / Play Once / Single Frame…) |
| Frame operations (insert/remove/keyframes, copy/paste frames, reverse, span drag, Synchronize Symbols) | `frames.rs` | Own | Animate timeline behaviour (user guide) |
| Retained tween curves (`TweenBoundary`), playback clock fixes | `tween_edit.rs`, parts of `model.rs` / `frames.rs` / `editor.rs` | Port | **Flare** `flare/sources/flare/rust/core` @ `a27136a` (our own earlier core, merged back) |
| Classic and shape tweens, matrix decomposition, shortest-path rotation | `model.rs`, `geom.rs` | Own | Animate behaviour |
| Eases: Quad … Elastic presets, custom Bezier ease curves | `motion.rs` | Own + Algo | Standard easing equations (Penner-style families) |
| Motion tweens, property keyframes, motion paths | `motion.rs` | Own | Animate motion-tween behaviour |
| Keyframe interpolation types (Constant, Linear, Ease In/Out, Exponential, Expression, Similar Shape) | `motion.rs` (`Interp`) | Port | **OpenToonz** `common/tparam/tdoubleparam.cpp` |
| Smooth motion keys | `motion.rs` | Algo | Cubic Hermite with Catmull-Rom tangents |
| Expression language for curves | `expr.rs` | Port | **OpenToonz** `common/expressions/tgrammar.cpp` |
| Editing session, undo/redo snapshots, every editor command | `editor.rs` | Own | Animate menu behaviour |
| Command registry (dotted ids + JSON args, used by scripts/CLI) | `commands.rs` | Own | Our own API; command names follow Animate menu wording |
| Merge Drawing (cut/merge fills, splitting lines, floating selections) | `merge.rs` | Own + Dep | Flash shape model as documented and observed; booleans by **i_overlay** (crates.io) |
| Convert Lines to Fills, Expand Fill, Soften Fill Edges | `merge.rs` | Own + Dep | i_overlay stroke outlines and offsets |
| Distort, Envelope, variable-width outlines, 3D projection | `deform.rs` | Own | Animate behaviour |
| Asset Warp (pins) | `deform.rs` (`MeshWarp`, `MeshMap`) | Algo + Approach | Rigid moving-least-squares (Schaefer, McPhail & Warren 2006); triangle-grid approach from **OpenToonz** `tnzext/plasticdeformer.cpp` |
| Bones, FK at evaluation | `model.rs`, `editor.rs` | Own | Animate behaviour |
| Inverse kinematics (damped least squares, pins, step limit) | `ik.rs` | Port | **OpenToonz** `toonzlib/ikjacobian.cpp`, `ikengine.cpp` (after S. Buss's Jacobian methods) |
| Shape bones + Bind tool | `rig.rs` | Own + Algo | Linear-blend skinning |
| Stroke styles Ragged / Stipple / Hatched | `strokes.rs` | Own | Animate behaviour |
| Stroke styles Herringbone, Rope, Chain, Plait, Fuzz, Tulle, Wave, Curl, Zigzag… | `strokes.rs` | Port | **OpenToonz** `colorfx/strokestyles.cpp` |
| Deformation tools Magnet / Pinch / Pump / Bender / Iron | `sculpt.rs` | Port | **OpenToonz** `tnztools/{magnet,pinch,pump,bender,iron}tool.cpp`, `tnzext/Stroke*Deformation.cpp` |
| Drawing assistants (line, vanishing point, ellipse, perspective, fisheye) + mirror/star/grid/affine/jitter replicators | `assist.rs` | Port | **OpenToonz** `tnztools/assistants/*`, `assistant.cpp`, `modifiers/modifierjitter.cpp` |
| Raster Brush with MyPaint brushes | `paint.rs` | Dep + Approach | **hokusai** (pure-Rust libmypaint port, crates.io); usage modelled on **OpenToonz** `toonzrasterbrushtool.cpp`; brushes from **mypaint-brushes** (CC0) |
| Particle emitters | `particles.rs` | Own | Our own closed-form design (same role as OpenToonz's particles effect and Animate's Particle System) |
| Trace Bitmap (colour regions → outlines) | `trace.rs` | Approach + Algo | **OpenToonz** `toonzlib/toutlinevectorizer.cpp` (approach); Douglas–Peucker + Catmull-Rom |
| Trace Bitmap › Centerline | `centerline.rs` | Approach + Algo | **OpenToonz** `toonzlib/tcenterline*.cpp` (approach); chamfer 3-4 distance, Zhang–Suen thinning |
| XDTS timing sheets (export/import) | `xdts.rs` | Approach | **OpenToonz** `toonz/xdtsio.cpp`, `xdtsimportpopup.cpp` (format and cell rules) |
| Text layout (wrap, justify, indent, margins, super/subscript), Dynamic/Input fields | `text.rs` | Own + Dep | Animate text behaviour; glyph outlines by **ab_glyph**; fallback fonts from **epaint_default_fonts** |
| Filters and blend modes model (Animate units) | `fx.rs` | Own | Animate Filters/Blending documentation |
| Bitmaps in the library | `bitmap.rs` | Own + Dep | **png** crate |
| Image decoders (PNG, JPEG, GIF, BMP, TGA, TIFF, WebP, OpenEXR) | `codecs.rs` | Dep | **image** crate |
| Sounds on keyframes (Event/Start/Stop/Stream) | `sound.rs` | Own | Animate sound behaviour |
| Built-in demo document | `demo.rs` | Own | Original artwork |

## 2. Rendering and publishing (`crates/ac-render`)

| Function | Code | Origin | Source / reference |
|---|---|---|---|
| Rasteriser shared by stage and exports, gradients incl. focal points | `lib.rs` | Own + Dep | **tiny-skia** (incl. two-point conical gradients) |
| Filters, blend modes, masks (offscreen groups) | `fx.rs` | Own | Animate filter/blend definitions |
| EffectCraft effects (~200) + Motion Blur as filters | `effectcraft.rs` | Dep | **EffectCraft** `effectcraft-*` (git dependency, ArtCraft) |
| GIF export | `lib.rs` | Dep + Algo | **gif** crate; NeuQuant palette (**color_quant**) |
| PNG / JPEG / PNG sequence | `lib.rs` | Dep | **png**, **image** |
| Video (MP4/WebM/MOV) with sound | `lib.rs` | Ext | **ffmpeg** (user-installed) |
| SVG and animated SVG (SMIL) | `lib.rs` | Own | SVG 1.1 / SMIL specs |
| HTML5 Canvas publish + runtime (display lists, scripts, buttons, sounds, text fields) | `html5.rs`, `html5_runtime.js` | Own | CreateJS public API docs (for the script API shape); HTML Canvas spec |
| Sprite sheets (JSON hash/array, Starling XML), Lottie JSON | `formats.rs` | Own | Public Lottie (Bodymovin) schema; Starling/TexturePacker atlas formats |
| System fonts | `fonts.rs` | Dep | **fontdb**, **ab_glyph** |

## 3. File formats (`crates/ac-xfl`, `crates/ac-package`)

| Function | Code | Origin | Source / reference |
|---|---|---|---|
| .fla / XFL read and write (document, symbols, folders, linkage, text, swatches, focal gradients) | `ac-xfl/read.rs`, `write.rs`, `lib.rs` | Own + Dep | Public XFL structure + self-authored/permitted files; **roxmltree**, **zip**, **flate2** |
| XFL `edges` strings (twips, hex fixed point, curves) | `ac-xfl/edges.rs` | Own | Worked out from self-authored files |
| `bin/*.dat` bitmaps | `ac-xfl/dat.rs` | Own | Worked out from self-authored files |
| Art brushes from `PaintBrushDefinitions.xml` | `ac-xfl/brush.rs` | Own + Dep | File's own embedded SVG parsed by **usvg** |
| Native `.acx` package | `ac-package/lib.rs` | Own | Our own format (ZIP of JSON parts); **zip** |

## 4. Audio (`crates/ac-audio`)

| Function | Code | Origin | Source / reference |
|---|---|---|---|
| Decode WAV/MP3/OGG/FLAC/AIFF | `lib.rs` | Dep | **symphonia** |
| Mixing by Animate's sound rules, waveforms, heuristic lip-sync | `lib.rs` | Own | Animate sound sync behaviour |
| Speech-based lip sync | `rhubarb.rs` | Ext | **Rhubarb Lip Sync** (MIT), run as a separate program the way OpenToonz and Flare use it |
| Live playback | `playback.rs` | Dep | **cpal** |

## 5. Scripting (`crates/ac-script`)

| Function | Code | Origin | Source / reference |
|---|---|---|---|
| Sandboxed frame scripts (CreateJS-style `this.stop()`, instance access) | `lib.rs` | Own + Dep | **rquickjs** (QuickJS); CreateJS public docs for API shape |
| Authoring scripts: JS embedded; Python/Node/Ruby via JSON-lines bridge | `lib.rs` | Own | Our command registry |
| JSFL compatibility shim | `lib.rs` | Own | Adobe's published JSFL API reference (names only, own code) |

## 6. Desktop editor (`crates/ac-app`)

| Function | Code | Origin | Source / reference |
|---|---|---|---|
| App shell, menus, shortcuts | `main.rs` | Own + Dep | **eframe/egui 0.29**; shortcuts follow Animate's defaults |
| Flare logo, app icon, 28 tool icons | `main.rs`, `toolbar.rs`, `assets/` | Port (artwork) | **Flare** (`Flare.ico`, `icons/dark/tools`, BSD-3-Clause; descended from OpenToonz icons); drawn by **resvg/usvg** |
| Tools panel structure (sections, flyout slots, Edit Toolbar) | `toolbar.rs` | Approach | **PhotoCraft** toolbar structure |
| Movable docks (grips, drop targets, Workspace menu) | `dock.rs` | Approach | **Flare Studio** workspace docking behaviour (own code) |
| Startup window | `startup.rs` | Approach | **Flare** `startuppopup.cpp` (behaviour) |
| Navigator, command search, Tool Options bar | `navigator.rs`, `palette.rs`, `options_bar.rs` | Approach | **Flare Studio** (behaviour; own code) |
| History panel | `history.rs` | Approach | **PhotoCraft** history panel layout |
| Swatches (.aco, .ase, .grd) | `swatches.rs` | Dep | **PhotoCraft** `photocraft-psd` readers/writers (git dependency) |
| Swatches (.gpl, .act) | `swatches.rs` | Own | GIMP palette / ACT colour table formats (public) |
| Color panel, Preferences, Document Settings, Align/Info, Shape dialogs | `color_panel.rs`, `prefs.rs`, `windows.rs` | Own | Animate behaviour |
| Stage, tools, selection overlays | `stage.rs` | Own | Animate tool behaviour |
| Timeline panel (loop range, seconds, zoom, thumbnails, Shift & Trace) | `timeline.rs` | Own + Approach | Animate timeline; Shift & Trace from **OpenToonz** |
| Properties, Library (folders, linkage), Transform | `panels.rs` | Own | Animate panels |
| Motion Editor, ease editor, motion path | `motion_ui.rs` | Own | Animate Motion Editor behaviour |
| Rulers, guides, grid | `rulers.rs` | Own | Animate behaviour |
| Import: bitmaps, SVG, PSD | `import.rs` | Dep | **image**, **usvg**, **psd** crate |
| Native file dialogs | `files.rs` | Dep | **rfd** |
| Publish Settings, Test Movie | `publish.rs` | Own | Our HTML5 runtime, opened in the default browser |

## 7. CLI and tooling

| Function | Code | Origin | Source / reference |
|---|---|---|---|
| `flare-cli` (info, gif, png, svg, html5, sprites, lottie, xfl, convert, run, effects) | `ac-cli/main.rs` | Own | Same engine as the editor |
| Windows installer | `tools/build-windows.sh`, `packaging/windows/installer.nsi` | Own + Ext | MinGW + **NSIS** |
| UI test driver under Xvfb | `tools/ui_drive.py`, `tools/restart.sh` | Own | **python-xlib** (XTEST) |

---

## Projects we draw from

| Project | Licence | How it's used |
|---|---|---|
| Flare (github.com/Flare-Animate/flare) | BSD-3-Clause (app) / MIT-Apache (Rust core) | Core merged back; app and tool icons; Studio UI behaviour. **Not used:** its Ghidra-derived theme material and GPL/AGPL-referenced SWF readers |
| OpenToonz (github.com/opentoonz/opentoonz) | BSD-3-Clause | Ports and approaches listed above; see `docs/OPENTOONZ-REVIEW.md` |
| PhotoCraft (ArtCraft) | MIT OR Apache-2.0 | `photocraft-psd` git dependency; toolbar and history structure; `panel_docking.rs` is the reference for future floating panels |
| EffectCraft (ArtCraft) | MIT OR Apache-2.0 | `effectcraft-*` git dependencies |
| hokusai / mypaint-brushes | MIT OR Apache-2.0 / CC0 | Raster brush engine and brush presets |
| Rhubarb Lip Sync | MIT | Optional external program |
| Main crates | mostly MIT / Apache-2.0 | tiny-skia, i_overlay, ab_glyph, fontdb, usvg/resvg, image, png, gif, color_quant, symphonia, cpal, rquickjs, roxmltree, zip, flate2, psd, rfd, eframe/egui. `cargo tree` gives the full list |
