# Flare vs Adobe Animate: gap analysis

Snapshot of **M0.3** (commit `f372248`, Oct 2026, when this Rust code base was called AnimationCraft), statuses updated after **Phase 1** against **Adobe
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
| Workspace and panels | ~65% | docking/workspaces, Color & Swatches, History, preferences |
| Drawing model | ~92% | stroke-style parameters, hairlines and stroke scaling |
| Tools | ~96% | pressure/tilt, Art/Pattern brushes |
| Timeline | ~95% | sound envelopes, Auto Keyframe toggle, layer depth |
| Symbols | ~82% | 9-slice, Visible / Cache as bitmap |
| Animation | ~88% | tween presets, copy/paste motion, rig mapping, bone springs |
| Text | ~40% | dynamic/input text, rich text, paragraph options |
| Media | ~78% | envelope editing, video, AI import, bitmap compression settings |
| Scripting | 🟡 different by design | ActionScript 3 (won't do), full CreateJS API, Code Snippets, debugger, rest of JSFL |
| Publish / export | ~82% | publish profiles, WebGL, AIR, SWF |
| File formats | ~65% | a corpus of real .fla files, legacy binary .fla |
| Preferences and polish | ~35% | preferences dialog, keyboard shortcut editor, workspaces, themes |

The two engine milestones that unlock most of the rest:

1. **Merge Drawing**, the planar-map shape model: Flash's defining feature. It's needed for
   edge bending, brush modes, the eraser and cutting shapes, and faithful FLA import. ✅ Phase 3.
2. **Motion tweens and the Motion Editor**: property keyframes along spans, motion paths and graph
   eases. Most modern Animate projects use them. ✅ Phase 2.

---

## 1. Workspace and panels

| Feature | Animate | Flare | Status | Pri |
|---|---|---|---|---|
| Stage, pasteboard, zoom, pan | Yes | Yes | ✅ | |
| Stage rotation (Rotation tool) | Yes | Yes | ✅ | |
| Clip content outside stage | View › Pasteboard toggle | View › Clip Content Outside Stage (Ctrl+Shift+W), edit-bar toggle | ✅ | |
| Rulers | View › Rulers (Ctrl+Alt+Shift+R) | Yes (with cursor markers) | ✅ | |
| Guides (drag from rulers, lock, snap, Edit Guides dialog) | Yes | Drag from rulers, move, drop to remove, lock, snap, clear, Edit Guides (colour, snap accuracy) | ✅ | |
| Grid (show, snap, spacing, colour) | Yes | Yes (Edit Grid) | ✅ | |
| Snapping: objects, pixels, align (smart guides), guides, grid | Yes | Objects, stage, guides, grid, pixels (View › Snapping) | 🟡 | P3 (smart-guide preferences) |
| Panel docking, floating panels, tabs, collapse to icons | Yes | Docks move by their grips (Tools, Properties, Library, Timeline); no floating panels or tab groups | 🟡 | P1 (see Gaps to close) |
| Workspaces (Essentials, Animator, Designer, Developer…, save/reset) | Yes | — | 🟡 movable docks (Tools/Properties L·R, Timeline top·bottom, Library column), saved, Reset Layout; no named/saved workspaces or floating panels yet | P2 |
| UI themes (dark / light / colour) | Yes | Dark only | ❌ | P3 |
| Properties panel (context-sensitive, Tool/Object/Frame/Doc tabs) | Yes | Yes | ✅ | |
| Library: folders, search, sort, preview, linkage, Use Count | Yes | Folders (tree, new/rename/delete, drag items in, Move to Folder; read/written in .fla), search, sort, preview, use count, item Properties (name, type, linkage class) | ✅ | P3 (shared libraries) |
| Shared / external libraries, Assets panel (Default/Custom/Animated assets) | Yes | — | ❌ | P2 |
| Align panel (align, distribute, match size, space, to stage) | Yes (Ctrl+K) | Yes (+ Modify › Align) | ✅ | |
| Info panel (x/y, w/h, RGBA under cursor) | Yes (Ctrl+I) | Yes | ✅ | |
| Transform panel | Yes (Ctrl+T) | Yes | ✅ | |
| Color panel (solid / linear / radial / bitmap fills, HSB/RGB/hex, alpha, overflow modes) | Yes | Window › Color (Ctrl+Shift+F9): stroke/fill, None/Solid/Linear/Radial/Bitmap, picker + HSB/RGB/hex/alpha, gradient ramp (add/move/remove stops), flow, focal point; applies to the selection | ✅ | P3 (linear RGB) |
| Swatches panel (palette, save/load .clr/.aco/.act, Web 216) | Yes | Column beside the tools (Ctrl+F9): colours and gradients; click = fill, Shift+click = stroke, applied to the selection; per-document swatches read from and written to .fla (`swatchLists`, `extendedSwatchLists`) or kept in the app; import .aco/.ase/.grd (PhotoCraft readers), .gpl, .act; save .ase/.aco/.gpl/.grd | 🟡 | P3 (.clr colour sets) |
| History panel (steps, replay, copy steps as JSFL) | Yes | Window › History (Ctrl+F10): every step, click to go back or forward, Clear History | 🟡 | P2 (replay steps, copy steps as script) |
| Motion Editor | Yes | Per-property graphs, key drag/retime, smooth/corner, per-property eases; OpenToonz keyframe types and expressions | ✅ | |
| Output, Actions, Code Snippets, Compiler Errors panels | Yes | Output + Actions | 🟡 | P2 |
| Scene panel (multiple scenes) | Yes | Scene panel (Shift+F2), header switcher | ✅ | |
| Movie Explorer | Yes | — | ❌ | P3 |
| Components / Component Inspector | Yes (AS3, HTML5 components) | — | ❌ | P3 |
| Frame Picker (lip-sync / graphic first-frame picker) | Yes | First frame number; Lip Syncing "Mouth shapes" picker | 🟡 | P3 (visual frame picker) |
| Native open/save dialogs, recent files | Yes | Yes (+ drag-and-drop files onto the window) | ✅ | |
| Autosave / auto-recovery | Yes | Yes (recover prompt after a crash; unsaved-changes guard) | ✅ | |
| Preferences dialog (General, Sync, Code Editor, Text, Drawing, Script) | Yes | Edit › Preferences (Ctrl+U): undo levels, auto-recovery, interface size, Shift select, tooltips, Startup window; snap distance, default drawing model, tween curves; default text size; frame scripts | 🟡 | P3 (sync, code editor settings) |
| Keyboard shortcut editor (sets, export as HTML) | Yes | Fixed shortcuts | ❌ | P2 |
| Edit in place / Edit in new window | Yes | Edit in place (double-click; rest of the scene dimmed) and isolated edit-symbol mode | ✅ | P3 (new window) |
| Multiple open documents (tabs) | Yes | One document | ❌ | P2 |
| Document settings dialog (units, anchor, scale content, frame rate, platform) | Yes | Modify › Document (Ctrl+J): units (px/in/pt/cm/mm), size, Match Contents, scale content or anchor, stage colour, frame rate with Scale spans | ✅ | |
| Templates and sample files | Yes | Demo scene | 🟡 | P3 |
| Navigator (overview of the stage, click to scroll) | No (Animate has none) | Window › Navigator: frame thumbnail, visible area outlined, click/drag to scroll, zoom slider | ✅ | |
| Command search | No | Ctrl+Shift+P: menu commands and tools by name | ✅ | |
| Tool options bar | No (Properties only) | Window › Tool Options Bar: the tool's main settings, colours, drawing mode | ✅ | |

## 2. Drawing model and shape engine

| Feature | Animate | Flare | Status | Pri |
|---|---|---|---|---|
| Object Drawing mode (J) | Yes | Toggle (J), Tool properties | ✅ | |
| **Merge Drawing** (default mode): overlapping shapes cut and merge; same-colour fills merge; strokes split fills | Yes | Yes (i_overlay booleans; curves flattened) | ✅ | |
| Edge-based shape model (fill style left/right of each edge, as in XFL `<Edge fillStyle0/1 strokeStyle>`) | Yes | Disjoint fill regions + split stroke edges; XFL export shares edges between fills | ✅ | |
| Selection tool: click a fill vs stroke separately, double-click to select a connected stroke | Yes | Yes (marquee/lasso cut the shape too) | ✅ | |
| Selection tool edge bending (drag an edge to curve it; Ctrl/Alt drag adds a corner) | Yes | Edge bending, corner drag, Ctrl-drag adds a corner | ✅ | |
| Drag a selected part of a merge shape out of it | Yes | Yes (floats until deselected, then merges) | ✅ | |
| Group (Ctrl+G) / Ungroup | Yes | Yes, and edit groups in place (double-click) | ✅ | |
| Break Apart: text → letters → shapes; bitmap → bitmap fill | Yes | Instances, groups, drawing objects, text (letters, then shapes), bitmaps (merge shape with a bitmap fill) | ✅ | |
| Union / Intersect / Punch / Crop (Modify › Combine Objects) | Yes | Yes | ✅ | |
| Smooth / Straighten / Optimize Curves | Yes | Yes | ✅ | |
| Convert Lines to Fills, Expand Fill, Soften Fill Edges | Yes | Modify › Shape, on merge-shape selections and drawing objects (i_overlay outlines/offsets; soften fades colour and gradient fills in steps) | ✅ | |
| Rectangle / Oval **Primitives** (live corner radii, start/end angle, inner radius) | Yes | Yes (per-corner and inverted radii, close path) | ✅ | |
| Stroke styles: solid, dashed, dotted, ragged, stipple, hatched; cap/join/miter; scale mode; hinting | Yes | Solid / dashed / dotted / ragged / stipple / hatched, cap, join | 🟡 | P3 (style parameters, scale mode, hinting) |
| Variable-width profiles (presets, save width profile) | Yes | Width tool on drawing objects and merge-shape lines, delete width points, copy/paste profiles | 🟡 | P3 (named presets) |
| Art / Pattern brushes (from Illustrator-style brush library), brush library panel | Yes | Art brushes in .fla files (PaintBrushDefinitions.xml) are drawn: their SVG artwork stretched along the stroke; 13 generated OpenToonz stroke styles; 25 MyPaint raster brushes | 🟡 | P2 (pattern brushes, drawing with art brushes, brush library panel, writing them back to .fla) |
| Gradient fills (linear, radial, up to 15 stops, overflow: extend / reflect / repeat, focal point, linear RGB) | Yes | Linear/radial, any stops, extend / reflect / repeat, focal point (Properties slider; .fla, SVG, HTML5, Lottie) | 🟡 | P3 (linear RGB) |
| Bitmap fills (and Lock Fill) | Yes | Drawing objects and merge shapes; Gradient Transform edits them; Lock Fill for gradients | ✅ | |
| Alpha per fill / stroke colour | Yes | Yes (colour alpha) | ✅ | |
| Pressure / tilt (tablet) | Yes | — | ❌ | P2 |
| Hairline strokes, stroke scaling (normal / horizontal / vertical / none) | Yes | — | ❌ | P2 |
| Shape hints (for shape tweens) | Yes | Ctrl+Shift+H, drag + snap to outline | ✅ | |

## 3. Tools panel

| Tool | Animate | Flare | Status | Pri |
|---|---|---|---|---|
| Selection (V) | Click, marquee, Shift add, Alt-drag copy, edge bending | Click, marquee, Shift add, Alt-drag copy (one undo step), IK drag, edge bending | ✅ | |
| Subselection (A) | Anchors and handles | Same | ✅ | |
| Free Transform (Q) incl. Distort / Envelope / Rotate & Skew / Scale modes | Yes | Yes (combined mode + Distort + Envelope) | ✅ | |
| Gradient Transform (F) | Yes (also bitmap fills) | Gradients and bitmap fills | ✅ | |
| 3D Rotation (W) / 3D Translation (G), global/local toggle | Yes (movie clips) | Yes; no global/local toggle | 🟡 | P2 |
| Lasso (L), Polygon, **Magic Wand** (bitmap) | Yes | Lasso, Polygon, Magic Wand (threshold) on broken-apart bitmaps | ✅ | |
| Pen (P) + Add / Delete / Convert Anchor | Yes | Yes | ✅ | |
| Text (T) | Static, Dynamic, Input | Static, Dynamic, Input; click for auto width, drag for a fixed-width box | ✅ | |
| Line (N), Rectangle (R), Oval (O), PolyStar | Yes | Yes | ✅ | |
| Rectangle Primitive / Oval Primitive | Yes | Yes | ✅ | |
| Pencil (Shift+Y): Straighten / Smooth / Ink | Yes | Yes | ✅ | |
| Paint Brush (Y): art / pattern / vector brushes, width, pressure | Yes | Calligraphic-free round brush | 🟡 | P2 |
| **Classic Brush** (B): Paint Normal / Fills / Behind / Selection / Inside, brush shape, Lock Fill | Yes | All five modes, seven brush shapes, Lock Fill | ✅ | |
| Fluid Brush (B): velocity, pressure, smoothing, taper, size | Yes | Velocity + taper + size | 🟡 | P2 |
| Bone (M) incl. **shape bones** (bones inside one shape) | Yes | Both (shape bones pose with IK, tween) | ✅ | |
| **Bind** tool (attach shape control points to bones) | Yes | Yes | ✅ | |
| Paint Bucket (K): gap size, Lock Fill | Yes | Yes (fills enclosed areas of merge shapes) | ✅ | |
| Ink Bottle (S) | Yes | Yes | ✅ | |
| Eyedropper (I) incl. bitmap sampling | Yes | Yes (no bitmaps) | ✅ | |
| Eraser (E): Erase Normal / Fills / Lines / Selected Fills / Inside, Faucet, shape and size | Yes | All modes + Faucet on merge shapes (round eraser) | ✅ | |
| Width (U) incl. delete width point, copy/paste width | Yes | Add/drag, Alt+click delete, copy/paste profile | ✅ | |
| Asset Warp (Shift+W) incl. bitmaps/symbols, mesh options, Reset Warped Asset | Yes | Shapes, bitmaps, symbols and groups (pins deform a triangle grid; stage, video, SVG as a picture, HTML5); Remove warp in Properties | 🟡 | P2 (mesh density option; .fla export unwarped) |
| Camera (C), camera colour effects, layer depth (Ctrl+D) | Yes | Pan/zoom/rotate, colour effects + blur; plus a Free (VCam) mode: the camera is a movable frame on the stage, with follow and shake | 🟡 | Layer Depth P2 |
| Hand (H), Rotation (Shift+H), Zoom (Z) | Yes | Yes | ✅ | |
| Stroke/Fill swatches, Swap, Black & White, No colour | Yes | Yes | ✅ | |
| Snap toggle, Object Drawing toggle | Yes | Yes | ✅ | |

## 4. Timeline

| Feature | Animate | Flare | Status | Pri |
|---|---|---|---|---|
| Layers: show/hide, lock, outline, colour, rename, reorder | Yes | Yes | ✅ | |
| **Layer folders** (nest, collapse, folder show/lock) | Yes | Yes (drag into/out of folders, collapse, hide) | ✅ | |
| **Mask layers** (mask + masked layers, show mask) | Yes | Yes (any shapes/symbols as mask, several masked layers) | ✅ | |
| Guide layers and **Classic motion guides** (orient to path, snap) | Yes | Yes (guides don't export; Orient to path) | ✅ | |
| **Layer parenting** (parent layer view) | Yes | Layer menu ▸ Parent Layer and Show Parenting View (drag link dots) | ✅ | |
| Layer depth (2.5D, with camera) | Yes | — | ❌ | P2 |
| Layer properties dialog; layer effects (filters on layers) | Yes | Properties dialog incl. layer height 100/200/300 %; no layer effects | 🟡 | P2 |
| Frames: insert/remove (F5), keyframe (F6), blank (F7), clear, convert | Yes | Yes | ✅ | |
| Select frames, copy/cut/paste/move frames, labels | Yes | Yes | ✅ | |
| Frame label types: Name / Comment / Anchor | Yes | All three (comments are never jump targets) | ✅ | |
| Reverse Frames, Synchronize Symbols, Insert Blank Keyframe on ranges | Yes | All three; Synchronize Symbols turns on classic-tween Sync and advances First Frame between plain keyframes of the same graphic symbol | ✅ | |
| Drag the end of a span to extend / shorten it | Yes | Any span (Ctrl, or any motion tween); later keyframes move | ✅ | |
| Auto Keyframe toggle | Yes | Edits auto-key | 🟡 | P2 |
| Copy / Paste Layers (with folders) | Yes | Copy/Cut/Paste Layers (folders, masks, parenting kept), Duplicate Layer | ✅ | (cross-document with multiple documents, 5.6) |
| Onion skin: range markers, Advanced settings, colours, outline mode | Yes | Draggable/anchored markers, Onion 2/5/All, outlines; Shift & Trace ghosts (from OpenToonz) | 🟡 | P3 (colour settings) |
| **Edit multiple frames** | Yes | Keyframes between markers shown and moved together | ✅ | |
| Frame view sizes (tiny…large), preview thumbnails, timeline zoom | Yes | Frames menu Tiny/Small/Normal/Medium/Large, Ctrl+wheel zoom around the pointer (remembered); Preview / Preview in Context thumbnails, per-layer height | ✅ | |
| Frame-rate change with "scale spans", time display (s / frames) | Yes | Document Settings › Scale spans; click the frame counter (or Frames menu / right-click the frame numbers) to show seconds in the counter and the header | ✅ | |
| Loop playback range, play in a range | Yes | Control › Loop Range from Selection (Ctrl+Alt+L): orange markers over the frame numbers, drag the ends; playback starts in and loops over the range | ✅ | |
| Scenes (add, duplicate, reorder, Scene panel) | Yes | Yes; exports play every scene | ✅ | |
| Sound on frames (sync: event/start/stop/stream; loop; effects) and waveform | Yes | Sync modes, repeat/loop, effects (left/right, fades), sounds inside symbols, waveform, playback, video audio | 🟡 | P3 (custom envelopes) |

## 5. Symbols and instances

| Feature | Animate | Flare | Status | Pri |
|---|---|---|---|---|
| Graphic, Movie Clip, Button symbols; Convert (F8); New (Ctrl+F8) | Yes | Yes | ✅ | |
| Graphic looping: Loop / Play Once / Single Frame / Play Reverse once / Loop Reverse | Yes | All five | ✅ | |
| **Button states** Up / Over / Down / Hit + preview with Control › Enable Simple Buttons | Yes | Yes (Hit area, live on stage) | ✅ | |
| Instance name (for scripting) | Yes | Yes; frame scripts read/set `this.name.x/y/rotation/scaleX/scaleY/alpha/visible` | ✅ | |
| Swap Symbol | Yes | Properties ▸ Symbol | ✅ | |
| Duplicate Symbol and swap | Yes | Properties ▸ Symbol ▸ Duplicate: copies the symbol into the same folder ("Ball copy", "Ball copy 2") and swaps the instance, one undo step | ✅ | |
| Colour effects: Brightness, Tint, Advanced (RGBA multiply/offset), Alpha | Yes | All five, tweenable | ✅ | |
| **Filters**: Drop Shadow, Blur, Glow, Bevel, Gradient Glow, Gradient Bevel, Adjust Color; presets; filter tweening | Yes | All seven, presets, tweening (and motion-tween tracks); SVG export of blur/shadow/glow; HTML5 renders gradient filters per pixel (exact); plus Motion Blur and ~200 EffectCraft effects (beyond Animate) | ✅ | |
| **Blend modes** (Normal, Layer, Darken, Multiply, Lighten, Screen, Overlay, Hard Light, Add, Subtract, Difference, Invert, Alpha, Erase) | Yes | All 14 | ✅ | |
| Render: Original / Cache as Bitmap / Export as Bitmap; Visible flag | Yes | — | ❌ | P2 |
| 9-slice scaling guides | Yes | — | ❌ | P2 |
| Registration point choice on convert; transformation point vs registration | Yes | 3 × 3 registration grid in Convert to Symbol (remembered), folder choice; `selection.to_symbol registration:` | ✅ | |
| Edit in Place / Edit Symbols across the library; Update library item | Yes | Edit in place and isolated edit | 🟡 | P3 (Update library item) |
| Linkage (class name, export for ActionScript/runtime) | Yes | Library item Properties › Export for runtime + class name; read and written in .fla | ✅ | |
| Symbol folders in Library | Yes | Yes (see Library) | ✅ | |

## 6. Animation

| Feature | Animate | Flare | Status | Pri |
|---|---|---|---|---|
| Classic tween (scale, skew, rotate ±turns, colour, ease, Sync, Snap, Orient to path) | Yes | All of these | ✅ | |
| Classic tween **custom ease** (Edit easing curve per property, presets) | Yes | Curve editor + 30 presets per span, or per property (position, rotation, scale, colour, filters) | ✅ | |
| Shape tween + **shape hints** | Yes | Yes (first shape on the layer) | ✅ | |
| **Motion tweens** (object-based, property keyframes, auto-span, motion path you can edit with Selection/Subselection, Orient to path) | Yes | Position/rotation/skew/scale/alpha/brightness, 3D rotation/depth, tint, every filter parameter | ✅ | |
| **Motion Editor** (per-property graphs, Bezier eases, add anchor, presets, Copy/paste curves) | Yes | Graphs, keys, eases, Bezier handles, copy/paste curves | ✅ | |
| Motion presets panel (bounce, fly-in…, save your own) | Yes | — | ❌ | P2 |
| Copy / Paste Motion, Copy Motion as JS/XML | Yes | — | ❌ | P2 |
| Convert to Frame-by-Frame Animation, Convert to Keyframes | Yes | Yes | ✅ | |
| Bones: IK poses on keyframes, joint rotation/translation constraints, **springiness / damping**, bone ease, runtime vs authoring | Yes | Poses + damped-least-squares IK (OpenToonz); joint rotation enable + min/max limits; pinned joints (object and shape bones) | 🟡 | P2 (translation constraints, springs) |
| Asset Warp: pins, mesh density, tween warped poses, **Create Warped Asset / Reset** | Yes | Pins (rigid MLS) on shapes, bitmaps, symbols, groups; Reset | 🟡 | P2 (tween warped poses of bitmaps/symbols) |
| Camera: tween, colour effects, attach layers, layer depth | Yes | Tweenable pan/zoom/rotate, brightness/tint/alpha/blur, VCam follow + shake (beyond Animate) | 🟡 | P2 (attach layers, depth) |
| **Auto lip-sync** (map visemes from audio) | Yes | Properties ▸ Lip Syncing with a Mouth-shapes picker; Rhubarb Lip Sync (speech-based, external program) when installed, FFT-band heuristic otherwise; `lipsync.apply` | ✅ | |
| **Rig mapping** (map a rig onto symbols) | Yes | — | ❌ | P3 |
| Frame-by-frame workflow helpers (onion skin keyframes-only, Ctrl+Alt+L loop) | Yes | Onion skin Keyframes Only, Next/Previous Keyframe (Shift+. / Shift+,), Edit Multiple Frames, Shift & Trace, Loop Range (Ctrl+Alt+L) | ✅ | |

## 7. Text

| Feature | Animate | Flare | Status | Pri |
|---|---|---|---|---|
| Static text, outlined at render | Yes | Yes (2 bundled fonts) | 🟡 | |
| **System and web fonts**: font family/style picker, Typekit/Adobe Fonts | Yes | Installed fonts, searchable picker, styles/weights (simulated when missing) | 🟡 | P3 (web fonts) |
| Character: size, letter spacing, colour, auto-kern, anti-alias, selectable, link, superscript/subscript | Yes | Size, spacing, colour, auto-kern, selectable, link (opens in published movies), superscript/subscript | ✅ | P3 (anti-alias modes) |
| Paragraph: align, justify, indent, line spacing, margins, wrap, fixed-width boxes | Yes | All: left/centre/right/justify, indent, line spacing, left/right margins, fixed-width boxes with word wrap | ✅ | |
| Vertical text, orientation | Yes | — | ❌ | P3 |
| Rich text runs (mixed styles in one field) | Yes | — | ❌ | P2 |
| Dynamic and Input text (variable, max chars, embed glyphs) | Yes | Dynamic (scripts set `.text`, editor playback and HTML5), Input (HTML5 text field: max chars, password, border, multiline), instance name as the variable; .fla read/write | ✅ | P3 (embed glyphs) |
| Spell checking, Find and Replace | Yes | — | ❌ | P3 |

## 8. Media: images, audio, video

| Feature | Animate | Flare | Status | Pri |
|---|---|---|---|---|
| Import bitmaps (PNG, JPG, GIF, BMP, TIFF) to stage / library | Yes | PNG, JPG, GIF, BMP, TGA, TIFF, WebP, OpenEXR (library + stage, smoothing) | ✅ | |
| Bitmap properties (smoothing, compression, quality), Trace Bitmap | Yes | Smoothing; Trace Bitmap (threshold, minimum area, curve fit, corners) + Centerline mode (OpenToonz) | 🟡 | P2 (compression settings) |
| Import image sequences | Yes | Yes | ✅ | |
| Import SVG, Illustrator (.ai), Photoshop (.psd) with layer options | Yes | SVG (shapes, gradients, text as outlines); PSD (bitmap per layer, visibility, opacity) | 🟡 | P2 (.ai; PSD editable text/vector options) |
| Import audio (WAV, MP3, AIFF), sound properties, edit envelope | Yes | WAV, MP3, OGG Vorbis, FLAC, AIFF (Symphonia); library preview with waveform; no envelope editor | 🟡 | P3 |
| Import video (embedded or FLVPlayback), video layers | Yes | — | ❌ | P3 |
| Import animated GIF as frames | Yes | Yes | ✅ | |

## 9. Scripting

| Feature | Animate | Flare | Status | Pri |
|---|---|---|---|---|
| ActionScript 3 (frame scripts, classes, Flash Player/AIR) | Yes | Not planned: dead runtime | — | P3 |
| HTML5 Canvas frame scripts (JavaScript/CreateJS) | Yes | Sandboxed QuickJS in the editor; the same API in published pages, plus `addEventListener` click/mouseover/mouseout | 🟡 | P2 (rest of the CreateJS display API) |
| Code Snippets panel | Yes | Actions snippets | 🟡 | P2 |
| **JSFL** (authoring automation, Commands menu, `fl.getDocumentDOM()`) | Yes | JSFL shim (`fl`, Document, Timeline, Layer, Frame, Library, Element, FLfile, copy/cut/paste frames) on the command registry; `.jsfl` in the Script Console and CLI | 🟡 | P3 (the rest of the API) |
| Run Command, saved Commands, Manage Saved Commands | Yes | Run Script / Run Example | 🟡 | P2 |
| History → Save as Command | Yes | — | ❌ | P2 |
| Debugger, Compiler Errors | Yes | Output panel | 🟡 | P3 |
| Extensions (CEP panels, extension manager) | Yes | — | ❌ | P3 (plug-ins in M7) |

## 10. Publish and export

| Feature | Animate | Flare | Status | Pri |
|---|---|---|---|---|
| Publish Settings dialog (targets, per-target options, profiles) | Yes | Scale, frame range, transparency, GIF loop/quality, PNG prefix, ffmpeg | 🟡 | P2 (profiles) |
| **HTML5 Canvas** publish (JS + HTML, CreateJS runtime, sprite sheets for bitmaps) | Yes | Own runtime: per-frame display lists over shared tables (exact match with the stage), masks, gradients, bitmaps, filters/blend via canvas filters, frame scripts, buttons with Over/Down/Hit, sounds (Web Audio); folder or one self-contained .html | ✅ | (movie clips follow the main clock, as on our stage) |
| Export Video / Media (MP4/H.264 via Adobe Media Encoder, MOV) | Yes | MP4 (H.264), WebM (VP9, alpha), MOV (ProRes 4444) via ffmpeg, background job with progress | ✅ | |
| Export Animated GIF (with settings: size, colours, loop, dither) | Yes (Save for Web) | Size, range, loop, quality, transparency | ✅ | |
| Export Image (PNG/JPG/GIF/SVG with options, transparency) | Yes | PNG (transparency), JPEG (quality), GIF (palette, transparency) at any size; SVG | ✅ | |
| PNG sequence | Yes | Yes | ✅ | |
| Generate Sprite Sheet / Texture Atlas | Yes | PNG + JSON hash / JSON array / Starling XML; trim, padding, duplicate detection, shelf packing | ✅ | (MaxRects, rotation) |
| Animated SVG (SMIL) export | Yes | Frame groups with discrete SMIL timing; identical frames shared | ✅ | |
| WebGL glTF (standard/extended), VR 360/Panorama (deprecated) | Yes | — | ❌ | P3 |
| SWF / Flash Player, AIR desktop/mobile | Yes | — | ❌ | P3 (SWF via Ruffle crates maybe) |
| Publish to YouTube / social presets (post-2020) | Yes | — | ❌ | P3 |
| Lottie | No (third-party plug-in) | Shape layers per distinct frame (beziers, fills, strokes/dashes, gradients), embedded bitmaps; verified in lottie-web; no masks/filters | ✅ | (keyframed transforms instead of per-frame layers) |
| Test Movie (Ctrl+Enter) in a player window, Test Scene | Yes | Control › Test Movie (Ctrl+Enter): publishes one HTML page and opens it in the browser (scripts, buttons, sounds) | 🟡 | P3 (own player window, Test Scene) |

## 11. Files and interchange

| Feature | Animate | Flare | Status | Pri |
|---|---|---|---|---|
| **.fla read** (ZIP of XFL: `DOMDocument.xml`, `LIBRARY/*.xml`, `bin/*.dat` bitmaps, media) | Yes | `ac-xfl`: scenes, layers, frames (classic/shape tweens, custom and per-property eases, label types, scripts, sounds), motion-object tweens (experimental), raw shapes, drawing objects, primitives, instances, groups, static text (faces → weight/italic), bitmaps, sounds, all filters; warnings for the rest | 🟡 | P1 (real-file corpus) |
| **.fla / XFL write** | Yes | Zipped .fla and XFL folders; shared edges; round-trip tested; motion tweens baked frame by frame | 🟡 | P2 (motion objects) |
| Legacy binary .fla (CS4 and earlier) | Yes | — | ❌ | P3 |
| Save As .xfl (uncompressed folder) | Yes | Yes (`name.xfl` → folder with the marker file) | ✅ | |
| Import SWF | Old versions | — | ❌ | P3 |
| Copy/paste with Illustrator/Photoshop (clipboard formats) | Yes | Internal clipboard | ❌ | P3 |
| Exposure sheets (XDTS: OpenToonz, Clip Studio Paint, TVPaint) | No | Export, and import onto layers (re-time same-named layers, load numbered images, or labelled blank keyframes) | ✅ | |
| Native format | .fla | `.acx` package (ZIP: manifest, document, one JSON per scene and symbol, PNG/audio files, thumbnail; versioned, atomic save); `.acx.json` still supported | ✅ | |

## 12. Accessibility, collaboration, misc

| Feature | Animate | Flare | Status | Pri |
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
- From OpenToonz: drawing assistants (perspective, fisheye, symmetry, affine, jitter), deformation tools,
  centerline trace, MyPaint raster brushes, curve expressions, XDTS timing sheets.
- Particle emitters (dots, stars, symbols, bitmaps) that every export draws.

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

### Phase 4: Interchange and publishing — ✅ done (Oct 2026)
1. **.fla / XFL import** with a test corpus (shapes, symbols, tweens, motion XML, bitmaps, sounds)
2. XFL export (round-trip tests)
3. **HTML5 Canvas publish** (own runtime or a CreateJS-compatible output)
4. Sprite sheets / texture atlas; animated SVG; Lottie
5. Audio layers (waveform, sync modes), auto lip-sync
6. JSFL compatibility shim for existing Animate scripts

### Gap closing for Phases 1–4 — ✅ done (Oct 2026)
The partial items left in Phases 1–4 were closed in four batches (see the devlog):
A (Phase 4) sounds inside symbols, sound effects, AIFF, viseme picker, movie clips playing while
the main timeline is held, shared XFL edges, motion-object import, JSFL frames/FLfile, Lottie mattes;
B (Phase 3) gradient flow, ragged/stipple/hatched strokes, bitmap fills on merge shapes, Break Apart
text/bitmaps, Magic Wand, brush shapes, Ctrl-drag corners, width point delete + profile copy/paste;
C (Phase 2) per-property classic eases, Sync/Snap, 3D/tint/filter motion tracks, Motion Editor
Bezier handles + curve copy/paste, reverse loops, label types, any-span end drag, Parenting View;
D (Phase 1) edit in place (symbols, groups), Gradient Glow/Bevel + filter presets, Export Image
PNG/JPEG/GIF, font weights/italic, Edit Guides, Snap to Pixels.
Then the last three: saving while a group is open in place, the Width tool on merge-shape lines,
exact Gradient Glow/Bevel in HTML5; plus the Free camera (VCam) mode.

### Gaps to close
Recorded Oct 15 2026. Closed the same day with the referenced sources: **.fla gradient
focal points** (tiny-skia two-point conical gradients; SVG `fx`, HTML5 two-circle gradients,
Lottie highlight; read and written in .fla), **.fla art brushes** (the file's own
`PaintBrushDefinitions.xml` SVG artwork laid along the stroke), **per-document and gradient
swatches** (.fla `swatchLists`/`extendedSwatchLists`; Photoshop `.grd` through PhotoCraft),
plus the History panel, Convert Lines to Fills / Expand Fill / Soften Fill Edges (i_overlay)
and Test Movie (our HTML5 runtime). Still open:
1. **Floating panels and tab groups**: any panel can float, dock into any side, or join another
   panel as a tab; panels collapse to icons; named workspaces saved and switched. Needs the
   egui upgrade (0.29 → 0.36) first; PhotoCraft's `panel_docking.rs` (egui 0.36, MIT/Apache)
   is the reference once upgraded, and our grips and drop targets (`dock.rs`) become that
   tree's drag source.
2. **Pattern brushes** (.fla `PatternBrush` definitions, drawn as a solid line with a note),
   drawing with art brushes in the editor, a brush library panel, and writing paint brushes
   back to .fla.
3. **Animate `.clr` colour sets**: an undocumented binary format; no source or sample to work from.

### P2 in groups of five
Started Oct 15 2026 after the P1 gaps; each group is built, tested and checked in the app
before the next.
1. **Timeline** — ✅ loop range, time display, frame widths / timeline zoom, Synchronize
   Symbols, Duplicate Symbol and swap.
2. Symbols & motion: Copy/Paste Motion, motion presets, Cache as Bitmap / Visible flag,
   9-slice scaling, layer effects.
3. Drawing & rigging: hairlines and stroke scaling, Auto Keyframe toggle, 3D global/local,
   Asset Warp mesh options, bone springiness.
4. Commands: Code Snippets, saved Commands, History replay / save as command, Find and
   Replace, Publish profiles.
5. Documents & media: multiple documents, rich text runs, bitmap compression, .ai import,
   motion objects in .fla export.
Last: the egui-upgrade items (named workspaces / floating panels, shortcut editor) and tablet pressure.

### Phase 5: Workspace polish
Docking/floating panels and workspaces, preferences, a shortcut editor, the History panel,
Swatches/Color panels, Find and Replace, multiple documents, themes, accessibility; plus
timeline polish from the Flare review and better lip sync. Slice order 5.0–5.10 (egui upgrade +
UI test harness first) is in `docs/FLARE-REVIEW-PHASE5.md`.

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
