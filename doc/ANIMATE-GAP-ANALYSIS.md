<!--
Vendored from https://github.com/KMSDax/AnimationCraft (docs/ANIMATE-GAP-ANALYSIS.md),
upstream commit e3aadad1c. AnimationCraft is
licensed MIT OR Apache-2.0, which is compatible with Flare's BSD-3-Clause; the
license text is reproduced in docs/ANIMATIONCRAFT_ABSORPTION.md.

This is a clean-room gap analysis written from Adobe's public documentation, the
published XFL structure, and observed behaviour of Adobe Animate. It contains no
Adobe code. It is kept here because it is the most precise statement of the
target feature set this project is working toward, and because comparing it
against what Flare actually ships is how the remaining gaps get found.
Its "AnimationCraft" column describes that project, not Flare -- Flare's own
status is in FEATURE_STATUS.md and docs/ISSUE_MAP.md.

-->
# AnimationCraft vs Adobe Animate: gap analysis

Snapshot of **AnimationCraft M0.3** (commit `f372248`, Oct 2026), statuses updated after **Phase 1** against **Adobe
Animate 24.x**, the last feature release. Animate has been in maintenance mode
since Feb 2026, so this feature set is a fixed target.

**How this was built:** from Adobe's public documentation, the published XFL
structure, and observed behaviour of the app. This is clean-room work: we
rebuild what Animate *does*, never Adobe's code, artwork, icons or sample
files.

**Legend**
- Status: ✅ done · 🟡 partial · ❌ missing
- Priority: **P0** blocks real use · **P1** expected by Animate users · **P2** nice to have · **P3** legacy / niche

---

## 0. Summary

| Area | Coverage | Biggest gaps |
|---|---|---|
| Workspace and panels | ~60% | docking/workspaces, Color & Swatches, History, preferences |
| Drawing model | ~75% | XFL edge export of merge shapes, Convert Lines to Fills / Expand / Soften, ragged/hatched strokes |
| Tools | ~90% | Magic Wand, pressure/tilt, Art/Pattern brushes |
| Timeline | ~80% | frame view sizes, sound, Synchronize Symbols |
| Symbols | ~75% | 9-slice, Visible / Cache as bitmap, button sounds |
| Animation | ~70% | 3D/filter/colour properties in motion tweens, tween presets, copy/paste motion, lip-sync, rig mapping |
| Text | ~35% | dynamic/input text, rich text, paragraph options, font styles |
| Media | ~45% | audio, video, AI/PSD import, Trace Bitmap |
| Scripting | 🟡 different by design | ActionScript 3 (won't do), HTML5 Canvas / CreateJS runtime, Code Snippets, debugger |
| Publish / export | ~45% | HTML5 Canvas, sprite sheets, SVG animation, publish profiles, WebGL, AIR, SWF |
| File formats | ~5% | **.fla / XFL read & write** |
| Preferences and polish | ~35% | preferences dialog, keyboard shortcut editor, workspaces, themes |

The two engine milestones that unlock most of the rest:

1. **Merge Drawing**, the planar-map shape model: Flash's defining feature. It's needed for
   edge bending, brush modes, the eraser and cutting shapes, and faithful FLA import. ✅ Phase 3.
2. **Motion tweens and the Motion Editor**: property keyframes along spans, motion paths and graph
   eases. Most modern Animate projects use them. ✅ Phase 2.

---

## 1. Workspace and panels

| Feature | Animate | AnimationCraft | Status | Pri |
|---|---|---|---|---|
| Stage, pasteboard, zoom, pan | Yes | Yes | ✅ | |
| Stage rotation (Rotation tool) | Yes | Yes | ✅ | |
| Clip content outside stage | View › Pasteboard toggle | Always shows pasteboard | ❌ | P2 |
| Rulers | View › Rulers (Ctrl+Alt+Shift+R) | Yes (with cursor markers) | ✅ | |
| Guides (drag from rulers, lock, snap, Edit Guides dialog) | Yes | Drag from rulers, move, drop to remove, lock, snap, clear | 🟡 | P2 (Edit Guides dialog) |
| Grid (show, snap, spacing, colour) | Yes | Yes (Edit Grid) | ✅ | |
| Snapping: objects, pixels, align (smart guides), guides, grid | Yes | Objects, stage, guides, grid | 🟡 | P2 (pixels) |
| Panel docking, floating panels, tabs, collapse to icons | Yes | Fixed layout (Library can dock left) | 🟡 | P1 |
| Workspaces (Essentials, Animator, Designer, Developer…, save/reset) | Yes | — | ❌ | P2 |
| UI themes (dark / light / colour) | Yes | Dark only | ❌ | P3 |
| Properties panel (context-sensitive, Tool/Object/Frame/Doc tabs) | Yes | Yes | ✅ | |
| Library: folders, search, sort, preview, linkage, Use Count | Yes | Search, sort, preview, use count | 🟡 | P1 (folders, linkage, Update/Properties dialog) |
| Shared / external libraries, Assets panel (Default/Custom/Animated assets) | Yes | — | ❌ | P2 |
| Align panel (align, distribute, match size, space, to stage) | Yes (Ctrl+K) | Yes (+ Modify › Align) | ✅ | |
| Info panel (x/y, w/h, RGBA under cursor) | Yes (Ctrl+I) | Yes | ✅ | |
| Transform panel | Yes (Ctrl+T) | Yes | ✅ | |
| Color panel (solid / linear / radial / bitmap fills, HSB/RGB/hex, alpha, overflow modes) | Yes | Gradient stops in Properties only | 🟡 | P1 |
| Swatches panel (palette, save/load .clr/.aco/.act, Web 216) | Yes | — | ❌ | P1 |
| History panel (steps, replay, copy steps as JSFL) | Yes | Undo stack exists; no panel | ❌ | P1 |
| Motion Editor | Yes | Per-property graphs, key drag/retime, smooth/corner, per-property eases | ✅ | |
| Output, Actions, Code Snippets, Compiler Errors panels | Yes | Output + Actions | 🟡 | P2 |
| Scene panel (multiple scenes) | Yes | Scene panel (Shift+F2), header switcher | ✅ | |
| Movie Explorer | Yes | — | ❌ | P3 |
| Components / Component Inspector | Yes (AS3, HTML5 components) | — | ❌ | P3 |
| Frame Picker (lip-sync / graphic first-frame picker) | Yes | First frame number only | 🟡 | P2 |
| Native open/save dialogs, recent files | Yes | Yes (+ drag-and-drop files onto the window) | ✅ | |
| Autosave / auto-recovery | Yes | Yes (recover prompt after a crash; unsaved-changes guard) | ✅ | |
| Preferences dialog (General, Sync, Code Editor, Text, Drawing, Script) | Yes | — | ❌ | P1 |
| Keyboard shortcut editor (sets, export as HTML) | Yes | Fixed shortcuts | ❌ | P2 |
| Edit in place / Edit in new window | Yes | Edit-symbol mode only (other content hidden) | 🟡 | P1 |
| Multiple open documents (tabs) | Yes | One document | ❌ | P2 |
| Document settings dialog (units, anchor, scale content, frame rate, platform) | Yes | W/H/FPS/colour in Properties | 🟡 | P1 |
| Templates and sample files | Yes | Demo scene | 🟡 | P3 |

## 2. Drawing model and shape engine

| Feature | Animate | AnimationCraft | Status | Pri |
|---|---|---|---|---|
| Object Drawing mode (J) | Yes | Toggle (J), Tool properties | ✅ | |
| **Merge Drawing** (default mode): overlapping shapes cut and merge; same-colour fills merge; strokes split fills | Yes | Yes (i_overlay booleans; curves flattened) | ✅ | |
| Edge-based shape model (fill style left/right of each edge, as in XFL `<Edge fillStyle0/1 strokeStyle>`) | Yes | Disjoint fill regions + split stroke edges (convertible to XFL edges) | 🟡 | P1 (XFL edge writer, Phase 4) |
| Selection tool: click a fill vs stroke separately, double-click to select a connected stroke | Yes | Yes (marquee/lasso cut the shape too) | ✅ | |
| Selection tool edge bending (drag an edge to curve it; Ctrl/Alt drag adds a corner) | Yes | Edge bending + corner drag (no Ctrl-drag corner yet) | 🟡 | P2 (add corner) |
| Drag a selected part of a merge shape out of it | Yes | Yes (floats until deselected, then merges) | ✅ | |
| Group (Ctrl+G) / Ungroup | Yes | Yes (edit-in-place of groups pending) | 🟡 | P1 (edit group in place) |
| Break Apart: text → letters → shapes; bitmap → bitmap fill | Yes | Instances, groups, drawing objects → merge shape | 🟡 | P2 (text, bitmap) |
| Union / Intersect / Punch / Crop (Modify › Combine Objects) | Yes | Yes | ✅ | |
| Smooth / Straighten / Optimize Curves | Yes | Yes | ✅ | |
| Convert Lines to Fills, Expand Fill, Soften Fill Edges | Yes | — | ❌ | P2 |
| Rectangle / Oval **Primitives** (live corner radii, start/end angle, inner radius) | Yes | Yes (per-corner and inverted radii, close path) | ✅ | |
| Stroke styles: solid, dashed, dotted, ragged, stipple, hatched; cap/join/miter; scale mode; hinting | Yes | Solid / dashed / dotted, cap, join (on drawing objects) | 🟡 | P2 (ragged, stipple, hatched) |
| Variable-width profiles (presets, save width profile) | Yes | Width tool only | 🟡 | P2 |
| Art / Pattern brushes (from Illustrator-style brush library), brush library panel | Yes | — | ❌ | P2 |
| Gradient fills (linear, radial, up to 15 stops, overflow: extend / reflect / repeat, linear RGB) | Yes | Linear/radial, any stops, pad only | 🟡 | P2 |
| Bitmap fills (and Lock Fill) | Yes | Bitmap fills on drawing objects; Lock Fill for gradients | 🟡 | P2 (merge shapes, Gradient Transform on bitmaps) |
| Alpha per fill / stroke colour | Yes | Yes (colour alpha) | ✅ | |
| Pressure / tilt (tablet) | Yes | — | ❌ | P2 |
| Hairline strokes, stroke scaling (normal / horizontal / vertical / none) | Yes | — | ❌ | P2 |
| Shape hints (for shape tweens) | Yes | Ctrl+Shift+H, drag + snap to outline | ✅ | |

## 3. Tools panel

| Tool | Animate | AnimationCraft | Status | Pri |
|---|---|---|---|---|
| Selection (V) | Click, marquee, Shift add, Alt-drag copy, edge bending | Click, marquee, Shift add, IK drag, edge bending | 🟡 | Alt-drag copy P1 |
| Subselection (A) | Anchors and handles | Same | ✅ | |
| Free Transform (Q) incl. Distort / Envelope / Rotate & Skew / Scale modes | Yes | Yes (combined mode + Distort + Envelope) | ✅ | |
| Gradient Transform (F) | Yes (also bitmap fills) | Gradients only | 🟡 | P2 |
| 3D Rotation (W) / 3D Translation (G), global/local toggle | Yes (movie clips) | Yes; no global/local toggle | 🟡 | P2 |
| Lasso (L), Polygon, **Magic Wand** (bitmap) | Yes | Lasso + Polygon | 🟡 | Magic Wand P2 |
| Pen (P) + Add / Delete / Convert Anchor | Yes | Yes | ✅ | |
| Text (T) | Static, Dynamic, Input | Static | 🟡 | P1 |
| Line (N), Rectangle (R), Oval (O), PolyStar | Yes | Yes | ✅ | |
| Rectangle Primitive / Oval Primitive | Yes | Yes | ✅ | |
| Pencil (Shift+Y): Straighten / Smooth / Ink | Yes | Yes | ✅ | |
| Paint Brush (Y): art / pattern / vector brushes, width, pressure | Yes | Calligraphic-free round brush | 🟡 | P2 |
| **Classic Brush** (B): Paint Normal / Fills / Behind / Selection / Inside, brush shape, Lock Fill | Yes | Paint Brush with all five modes | 🟡 | P2 (brush shapes) |
| Fluid Brush (B): velocity, pressure, smoothing, taper, size | Yes | Velocity + taper + size | 🟡 | P2 |
| Bone (M) incl. **shape bones** (bones inside one shape) | Yes | Both (shape bones pose with IK, tween) | ✅ | |
| **Bind** tool (attach shape control points to bones) | Yes | Yes | ✅ | |
| Paint Bucket (K): gap size, Lock Fill | Yes | Yes (fills enclosed areas of merge shapes) | ✅ | |
| Ink Bottle (S) | Yes | Yes | ✅ | |
| Eyedropper (I) incl. bitmap sampling | Yes | Yes (no bitmaps) | ✅ | |
| Eraser (E): Erase Normal / Fills / Lines / Selected Fills / Inside, Faucet, shape and size | Yes | All modes + Faucet on merge shapes (round eraser) | ✅ | |
| Width (U) incl. delete width point, copy/paste width | Yes | Add/drag width points | 🟡 | P2 |
| Asset Warp (Shift+W) incl. bitmaps/symbols, mesh options, Reset Warped Asset | Yes | Shapes only; reset via Properties | 🟡 | P2 |
| Camera (C), camera colour effects, layer depth (Ctrl+D) | Yes | Pan/zoom/rotate | 🟡 | Layer Depth P2 |
| Hand (H), Rotation (Shift+H), Zoom (Z) | Yes | Yes | ✅ | |
| Stroke/Fill swatches, Swap, Black & White, No colour | Yes | Yes | ✅ | |
| Snap toggle, Object Drawing toggle | Yes | Yes | ✅ | |

## 4. Timeline

| Feature | Animate | AnimationCraft | Status | Pri |
|---|---|---|---|---|
| Layers: show/hide, lock, outline, colour, rename, reorder | Yes | Yes | ✅ | |
| **Layer folders** (nest, collapse, folder show/lock) | Yes | Yes (drag into/out of folders, collapse, hide) | ✅ | |
| **Mask layers** (mask + masked layers, show mask) | Yes | Yes (any shapes/symbols as mask, several masked layers) | ✅ | |
| Guide layers and **Classic motion guides** (orient to path, snap) | Yes | Yes (guides don't export; Orient to path) | ✅ | |
| **Layer parenting** (parent layer view) | Yes | Layer menu ▸ Parent Layer; no drag-to-link column yet | 🟡 | P2 (parent view column) |
| Layer depth (2.5D, with camera) | Yes | — | ❌ | P2 |
| Layer properties dialog; layer effects (filters on layers) | Yes | Properties dialog; no layer effects | 🟡 | P2 |
| Frames: insert/remove (F5), keyframe (F6), blank (F7), clear, convert | Yes | Yes | ✅ | |
| Select frames, copy/cut/paste/move frames, labels | Yes | Yes | ✅ | |
| Frame label types: Name / Comment / Anchor | Yes | Name only | 🟡 | P2 |
| Reverse Frames, Synchronize Symbols, Insert Blank Keyframe on ranges | Yes | Reverse Frames, ranges | 🟡 | P2 (Synchronize Symbols) |
| Drag the end of a span to extend / shorten it | Yes | Ctrl+drag a layer's last frame; motion tweens stretch | 🟡 | P2 (spans mid-layer) |
| Auto Keyframe toggle | Yes | Edits auto-key | 🟡 | P2 |
| Copy / Paste Layers (with folders) | Yes | Duplicate Layer | 🟡 | P1 |
| Onion skin: range markers, Advanced settings, colours, outline mode | Yes | Draggable/anchored markers, Onion 2/5/All, outlines | 🟡 | P3 (colour settings) |
| **Edit multiple frames** | Yes | Keyframes between markers shown and moved together | ✅ | |
| Frame view sizes (tiny…large), preview thumbnails, timeline zoom | Yes | One size | ❌ | P2 |
| Frame-rate change with "scale spans", time display (s / frames) | Yes | FPS only | 🟡 | P2 |
| Loop playback range, play in a range | Yes | Whole timeline loop | 🟡 | P2 |
| Scenes (add, duplicate, reorder, Scene panel) | Yes | Yes; exports play every scene | ✅ | |
| Sound on frames (sync: event/start/stop/stream; loop; effects) and waveform | Yes | — | ❌ | P1 |

## 5. Symbols and instances

| Feature | Animate | AnimationCraft | Status | Pri |
|---|---|---|---|---|
| Graphic, Movie Clip, Button symbols; Convert (F8); New (Ctrl+F8) | Yes | Yes | ✅ | |
| Graphic looping: Loop / Play Once / Single Frame / Play Reverse once / Loop Reverse | Yes | First three | 🟡 | P2 |
| **Button states** Up / Over / Down / Hit + preview with Control › Enable Simple Buttons | Yes | Yes (Hit area, live on stage) | ✅ | |
| Instance name (for scripting) | Yes | Yes; frame scripts read/set `this.name.x/y/rotation/scaleX/scaleY/alpha/visible` | ✅ | |
| Swap Symbol | Yes | Properties ▸ Symbol | ✅ | |
| Duplicate Symbol and swap | Yes | Duplicate only | 🟡 | P2 |
| Colour effects: Brightness, Tint, Advanced (RGBA multiply/offset), Alpha | Yes | All five, tweenable | ✅ | |
| **Filters**: Drop Shadow, Blur, Glow, Bevel, Gradient Glow, Gradient Bevel, Adjust Color; presets; filter tweening | Yes | Drop Shadow, Blur, Glow, Bevel, Adjust Color, tweened; SVG export of blur/shadow/glow | 🟡 | P2 (gradient glow/bevel, presets) |
| **Blend modes** (Normal, Layer, Darken, Multiply, Lighten, Screen, Overlay, Hard Light, Add, Subtract, Difference, Invert, Alpha, Erase) | Yes | All 14 | ✅ | |
| Render: Original / Cache as Bitmap / Export as Bitmap; Visible flag | Yes | — | ❌ | P2 |
| 9-slice scaling guides | Yes | — | ❌ | P2 |
| Registration point choice on convert; transformation point vs registration | Yes | Registration from bounds | 🟡 | P1 |
| Edit in Place / Edit Symbols across the library; Update library item | Yes | Isolated edit | 🟡 | P1 |
| Linkage (class name, export for ActionScript/runtime) | Yes | — | ❌ | P2 |
| Symbol folders in Library | Yes | — | ❌ | P1 |

## 6. Animation

| Feature | Animate | AnimationCraft | Status | Pri |
|---|---|---|---|---|
| Classic tween (scale, skew, rotate ±turns, colour, ease, Sync, Snap, Orient to path) | Yes | Scale/skew/rotate/turns/colour/ease | 🟡 | P1 (Sync, orient, Snap) |
| Classic tween **custom ease** (Edit easing curve per property, presets) | Yes | Curve editor + 30 presets per span (not per property) | 🟡 | P2 (per-property) |
| Shape tween + **shape hints** | Yes | Yes (first shape on the layer) | ✅ | |
| **Motion tweens** (object-based, property keyframes, auto-span, motion path you can edit with Selection/Subselection, Orient to path) | Yes | Position/rotation/skew/scale/alpha/brightness; 3D, filters, colour pending | 🟡 | P2 |
| **Motion Editor** (per-property graphs, Bezier eases, add anchor, presets, Copy/paste curves) | Yes | Graphs, keys, eases; no handle editing or copy/paste | 🟡 | P2 |
| Motion presets panel (bounce, fly-in…, save your own) | Yes | — | ❌ | P2 |
| Copy / Paste Motion, Copy Motion as JS/XML | Yes | — | ❌ | P2 |
| Convert to Frame-by-Frame Animation, Convert to Keyframes | Yes | Yes | ✅ | |
| Bones: IK poses on keyframes, joint rotation/translation constraints, **springiness / damping**, bone ease, runtime vs authoring | Yes | Poses + IK | 🟡 | P2 |
| Asset Warp: pins, mesh density, tween warped poses, **Create Warped Asset / Reset** | Yes | Pins, rigid MLS | 🟡 | P2 |
| Camera: tween, colour effects, attach layers, layer depth | Yes | Tweenable pan/zoom/rotate | 🟡 | P2 |
| **Auto lip-sync** (map visemes from audio) | Yes | — | ❌ | P2 |
| **Rig mapping** (map a rig onto symbols) | Yes | — | ❌ | P3 |
| Frame-by-frame workflow helpers (onion skin keyframes-only, Ctrl+Alt+L loop) | Yes | Basic | 🟡 | P1 |

## 7. Text

| Feature | Animate | AnimationCraft | Status | Pri |
|---|---|---|---|---|
| Static text, outlined at render | Yes | Yes (2 bundled fonts) | 🟡 | |
| **System and web fonts**: font family/style picker, Typekit/Adobe Fonts | Yes | Installed fonts, searchable picker | 🟡 | P2 (styles/weights, web fonts) |
| Character: size, letter spacing, colour, auto-kern, anti-alias, selectable, link, superscript/subscript | Yes | Size, spacing, colour | 🟡 | P1 |
| Paragraph: align, justify, indent, line spacing, margins, wrap, fixed-width boxes | Yes | Align + newlines | 🟡 | P1 |
| Vertical text, orientation | Yes | — | ❌ | P3 |
| Rich text runs (mixed styles in one field) | Yes | — | ❌ | P2 |
| Dynamic and Input text (variable, max chars, embed glyphs) | Yes | — | ❌ | P1 (with scripting runtime) |
| Spell checking, Find and Replace | Yes | — | ❌ | P3 |

## 8. Media: images, audio, video

| Feature | Animate | AnimationCraft | Status | Pri |
|---|---|---|---|---|
| Import bitmaps (PNG, JPG, GIF, BMP, TIFF) to stage / library | Yes | PNG, JPG, GIF, BMP (library + stage, smoothing) | ✅ | |
| Bitmap properties (smoothing, compression, quality), Trace Bitmap | Yes | — | ❌ | P1 (Trace P2) |
| Import image sequences | Yes | Yes | ✅ | |
| Import SVG, Illustrator (.ai), Photoshop (.psd) with layer options | Yes | SVG (shapes, gradients, text as outlines) | 🟡 | P2 (.ai/.psd) |
| Import audio (WAV, MP3, AIFF), sound properties, edit envelope | Yes | — | ❌ | P1 |
| Import video (embedded or FLVPlayback), video layers | Yes | — | ❌ | P3 |
| Import animated GIF as frames | Yes | Yes | ✅ | |

## 9. Scripting

| Feature | Animate | AnimationCraft | Status | Pri |
|---|---|---|---|---|
| ActionScript 3 (frame scripts, classes, Flash Player/AIR) | Yes | Not planned: dead runtime | — | P3 |
| HTML5 Canvas frame scripts (JavaScript/CreateJS) | Yes | Sandboxed QuickJS, CreateJS-style subset | 🟡 | P1 (full `this.*` API, events, instance names) |
| Code Snippets panel | Yes | Actions snippets | 🟡 | P2 |
| **JSFL** (authoring automation, Commands menu, `fl.getDocumentDOM()`) | Yes | Own command registry + JS/Python/Node/Ruby | 🟡 | P1 (a JSFL compatibility shim) |
| Run Command, saved Commands, Manage Saved Commands | Yes | Run Script / Run Example | 🟡 | P2 |
| History → Save as Command | Yes | — | ❌ | P2 |
| Debugger, Compiler Errors | Yes | Output panel | 🟡 | P3 |
| Extensions (CEP panels, extension manager) | Yes | — | ❌ | P3 (plug-ins in M7) |

## 10. Publish and export

| Feature | Animate | AnimationCraft | Status | Pri |
|---|---|---|---|---|
| Publish Settings dialog (targets, per-target options, profiles) | Yes | Scale, frame range, transparency, GIF loop/quality, PNG prefix, ffmpeg | 🟡 | P2 (profiles) |
| **HTML5 Canvas** publish (JS + HTML, CreateJS runtime, sprite sheets for bitmaps) | Yes | — | ❌ | **P0** |
| Export Video / Media (MP4/H.264 via Adobe Media Encoder, MOV) | Yes | MP4 (H.264), WebM (VP9, alpha), MOV (ProRes 4444) via ffmpeg, background job with progress | ✅ | |
| Export Animated GIF (with settings: size, colours, loop, dither) | Yes (Save for Web) | Size, range, loop, quality, transparency | ✅ | |
| Export Image (PNG/JPG/GIF/SVG with options, transparency) | Yes | PNG ×2, SVG | 🟡 | P1 |
| PNG sequence | Yes | Yes | ✅ | |
| Generate Sprite Sheet / Texture Atlas | Yes | — | ❌ | P1 |
| Animated SVG (SMIL) export | Yes | — | ❌ | P2 |
| WebGL glTF (standard/extended), VR 360/Panorama (deprecated) | Yes | — | ❌ | P3 |
| SWF / Flash Player, AIR desktop/mobile | Yes | — | ❌ | P3 (SWF via Ruffle crates maybe) |
| Publish to YouTube / social presets (post-2020) | Yes | — | ❌ | P3 |
| Lottie | No (third-party plug-in) | — | ❌ | P2 (our edge) |
| Test Movie (Ctrl+Enter) in a player window, Test Scene | Yes | Play on stage | 🟡 | P1 |

## 11. Files and interchange

| Feature | Animate | AnimationCraft | Status | Pri |
|---|---|---|---|---|
| **.fla read** (ZIP of XFL: `DOMDocument.xml`, `LIBRARY/*.xml`, `bin/*.dat` bitmaps, media) | Yes | Edge path parser only | ❌ | **P0** |
| **.fla / XFL write** | Yes | — | ❌ | P1 |
| Legacy binary .fla (CS4 and earlier) | Yes | — | ❌ | P3 |
| Save As .xfl (uncompressed folder) | Yes | — | ❌ | P1 |
| Import SWF | Old versions | — | ❌ | P3 |
| Copy/paste with Illustrator/Photoshop (clipboard formats) | Yes | Internal clipboard | ❌ | P3 |
| Native format | .fla | JSON `.acx.json` | 🟡 | (switch to XFL as native) |

## 12. Accessibility, collaboration, misc

| Feature | Animate | AnimationCraft | Status | Pri |
|---|---|---|---|---|
| Accessibility panel (names/descriptions for screen readers) | Yes | — | ❌ | P3 |
| Co-authoring, Sync settings, CC Libraries | Yes | — | ❌ | P3 |
| Find and Replace (text, fonts, colours, symbols) | Yes | — | ❌ | P2 |
| Spell check | Yes | — | ❌ | P3 |
| Localisation (multi-language UI) | Yes | English | ❌ | P3 |

## 13. Where we're already ahead

- Native Rust with a CPU renderer shared by stage and exports: what you see is what you export.
- Authoring automation in **JavaScript, Python, Node and Ruby** over one command registry, plus a headless CLI.
- Sandboxed frame scripts: they can't touch files or the network.
- Open document format, and an open source project.
- Path to a web build (WASM), Lottie export and an MCP server for AI agents.

---

## 14. Proposed build order

The phases put dependencies first. Each phase ends with tests, a UI run under Xvfb, docs, and a push.

### Phase 1: "Usable for real projects" (P0 that don't need the new engine) — ✅ done (Oct 2026)
1. Native file dialogs, recent files, **autosave + crash recovery**
2. **Group / Ungroup**, Align panel (Ctrl+K), Info panel
3. **Layer folders**, **mask layers**, guide layers (+ classic motion guide)
4. **Instance names** and the Properties fields for them
5. **Filters** (Drop Shadow, Blur, Glow, Bevel, Adjust Color) + **blend modes** in the renderer
6. **Bitmap import** (PNG/JPG) as library items, plus SVG import
7. **System fonts** for text (font picker via `font-kit` / `fontdb`)
8. Export: **PNG sequence**, **MP4/WebM** (ffmpeg), GIF settings, Publish Settings dialog
9. Rulers, guides, grid + snapping to them

### Phase 2: Animation depth — ✅ done (Oct 2026)
1. **Custom eases** (graph editor, presets) for classic tweens
2. **Motion tweens** (object spans, property keyframes, editable motion path)
3. **Motion Editor** panel
4. Shape hints; Convert to Frame-by-Frame; Reverse Frames; span-end dragging
5. Onion skin markers + Edit multiple frames
6. Button states + Enable Simple Buttons; Swap Symbol; Brightness / Advanced colour effects
7. Scenes; layer parenting

### Phase 3: The Merge Drawing engine — ✅ done (Oct 2026)
1. Planar-map shape model (edges with left/right fill styles) matching XFL `<DOMShape>`
2. Merge/cut on draw, Selection-tool fill/stroke picking, **edge bending**
3. Classic Brush modes, Eraser modes + Faucet, Paint Bucket gap size / Lock Fill
4. Combine Objects (Union / Intersect / Punch / Crop), Optimize / Smooth / Straighten
5. Primitives, stroke styles, bitmap fills, shape bones + Bind tool

### Phase 4: Interchange and publishing
1. **.fla / XFL import** with a test corpus (shapes, symbols, tweens, motion XML, bitmaps, sounds)
2. XFL export (round-trip tests)
3. **HTML5 Canvas publish** (own runtime or a CreateJS-compatible output)
4. Sprite sheets / texture atlas; animated SVG; Lottie
5. Audio layers (waveform, sync modes), auto lip-sync
6. JSFL compatibility shim for existing Animate scripts

### Phase 5: Workspace polish
Docking/floating panels and workspaces, preferences, a shortcut editor, the History panel,
Swatches/Color panels, Find and Replace, multiple documents, themes, accessibility.

---

## 15. Notes for reverse-engineering

- **XFL** is the primary reference. Element and attribute names (`DOMFrame`, `DOMSymbolInstance`,
  `DOMShape` with `fills`/`strokes`/`edges`, `Motion` XML for motion tweens) define what the
  model must hold. Mirror them 1:1 so import/export is a mapping, not a translation.
- Edge path strings in XFL (`!x y|x y[cx cy x y`, `S` style changes, coordinates in twips/20) are
  already parsed by `ac_core::xfl`. Extending this to fill/stroke style tables gives the planar map.
- Behavioural references (all public): Adobe's Animate user guide, the JSFL API reference
  (documents the DOM: `Document`, `Timeline`, `Layer`, `Frame`, `Element`, `Shape`, `Edge`,
  `HalfEdge`, `Contour`), the SWF file format spec (shape records, fill styles, morph shapes,
  filters, blend modes), and the CreateJS docs for the HTML5 runtime.
- Test against real `.fla` files from users with permission. Never ship Adobe samples.

