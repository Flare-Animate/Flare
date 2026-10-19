# Flash / Adobe Animate (FLA/XFL) import

This module imports Adobe Flash / Animate documents into OpenToonz as an
ordinary, editable OpenToonz scene: PLI vector levels, raster levels,
palettes, Xsheet columns, sub-xsheets, stage keyframes and FX. No Adobe
software is needed. Scripts in the document are never executed, and archive
members are read in memory without being extracted.

Import is **best effort**: everything with a native mapping is translated, and
anything without one is approximated or left out. Every approximation and
omission is listed, with its source location, in the import dialog and in the
bundle's `import-report.json`. Only structural problems (an unreadable
package, a missing document timeline, symbol cycles, resource budgets) block
an import. Library callers can request a strict import that rejects on the
first unmapped detail (`ImportOptions::bestEffort = false`).

## Supported input

| Input | Status |
| --- | --- |
| CS5 and later compressed `.fla` (ZIP + XFL XML) | Supported |
| Uncompressed XFL project (`.xfl` marker or `DOMDocument.xml` folder) | Supported |
| Compressed interchange `.xfl` (ZIP with an `.xfl` suffix) | Supported |
| CS4 and earlier binary `.fla` (OLE/CFB records) | Not supported; rejected with a clear message |
| `.swf` | Not supported (a published movie, not an authoring document) |

## How Flash concepts become OpenToonz data

| Flash / Animate | OpenToonz | Notes |
| --- | --- | --- |
| Stage size, frame rate, background | Camera resolution, scene frame rate, background color | |
| Several scenes | One sub-xsheet per scene, played back to back in a "Flash scenes" column | |
| Layer | One column per part track; simultaneous elements in one layer get separate columns | Uniquely named instances keep their column across keyframes |
| Folder layer | Flattened; lock is inherited by the child columns | |
| Hidden layers and folders (eye off) | Visible columns when the document publishes hidden layers (*Include hidden layers*, Flash's default); hidden columns otherwise | Guides keep their authored eye state |
| Guide layer | `[guide]` column hidden from render; visible in the editor on the main timeline, hidden inside symbols | The import option *Show guide layers inside symbols* shows them everywhere |
| Mask layer | One native mask column right below the masked layers' columns, clipping all of them (the new column *mask scope*, shown in the column settings popup), plus an `In` FX matte, so the viewer, playback and render clip alike | The mask shows the silhouette Flash clips with: the filled area of the mask artwork, fully opaque, without strokes. A symbol instance on a mask layer shows an opaque fill-only copy of its symbol, *&lt;symbol&gt; (Flash mask)*. Classic and shape tweens of the mask are kept. Rows where the mask has no artwork let everything through, as in Flash |
| Keyframe spans | Cell exposures; blank spans stay empty | |
| Shapes, drawing objects, rectangle/oval primitives | PLI vector drawings with editable strokes and filled regions | Source edges are converted directly, not traced. As in Animate's published SWFs (even-odd rule), an open fill outline is closed, contours of one paint that overlap fill even-odd, and where paints overlap the topmost opaque one shows |
| Solid fills and strokes | Palette styles shared per drawing level | |
| Linear and radial gradient fills and gradient strokes | `TFlashGradientStyle` palette styles (native, editable, saved in PLI) | Full paint matrix, ordered stops with alpha, spread, interpolation and focal point. A singular matrix fills with the first stop's color, as the published SWF is drawn |
| Bitmap fills (`BitmapFill`) of shapes and strokes | `TFlashBitmapStyle` palette styles (native, editable; the bitmap is kept in the PLI palette) | Clipped or repeating, nearest or smoothed, as authored |
| Graphic symbol | Shared sub-xsheet; loop, play once, single frame, reverse modes and first/last frame become cell exposures | Retime later in **Windows > Flash Instance** |
| Movie clip | Shared sub-xsheet with its own clock from the moment it appears; continuous across keyframes that keep the same clip | Nested clips keep their own clocks too. Where native cells cannot express a nested clock, the placement plays a *clock replay* of the symbol, a sub-xsheet named `<symbol> [clock]` built from the same source frames |
| Frame scripts (AS2 and AS3) | The timeline's timing; the script text stays in an Xsheet note on its frame | Scripts are never executed. `stop()`, `play()`, `gotoAndStop(...)`, `gotoAndPlay(...)`, `nextFrame()`, `prevFrame()` on the own timeline or the root are simulated tick by tick; other statements are reported by kind |
| Group | Untransformed sub-xsheet of the members | |
| Instance transform | Column stage keys: position, rotation, scale, shear, stacking order | Held between keyframes |
| Classic motion tween | Interpolated stage keys around the source transformation point | Linear, Flash's quadratic ease or custom ease curves; auto, clockwise and counter-clockwise rotation with extra turns |
| Motion guide | Native motion path: the guided column follows the guide stroke as its spline (*Path*, or *Path + Aim* with orient to path), `T_Path` keyed with the tween's ease | |
| Motion tween (CS4+ motion object) | Native stage keys from the tween's property curves; tween color curves become color effect keys | Eases (Simple and Stop and Start presets) are applied; a channel that follows one curve keeps native segments, others are keyed per frame |
| Layer parenting (Animate 2019+) | Native column parents; the child keeps editable local keys | Chains compose and stacking order is kept |
| Camera layer (Animate 2017+) | The scene camera's stage keys (position, rotation, scale), tweens included; layers attached to the camera become its children | |
| Shape tween, also inside symbols and on mask layers | One editable PLI drawing per in-between row in the layer's drawing level | Follows the correspondence Animate saves (`MorphShape`, shape hints included); fills, strokes and widths are interpolated with the tween's ease |
| Symbol alpha | Column opacity, also applied in final renders | Changing alpha is keyed on the column's new animatable *Opacity* curve (Function Editor, under the column) |
| Color effects: tint, brightness, advanced | Tint and brightness: the stock *RGB Fade* FX; advanced multipliers and offsets: the new *Color Matrix* FX | Keyed across classic tweens |
| Filters on movie clips and buttons, in order | Blur: the stock *Blur* FX; drop shadow and glow: the new *Drop Shadow* FX; bevel: the new *Bevel* FX; adjust color: *Color Matrix* with Animate's matrix | *Drop Shadow* and *Bevel* compute as the Flash Player does (box blur passes, distance, angle, strength, inner, knockout, hide object) |
| Blend modes: multiply, screen, lighten, darken, add, subtract; layer | OpenToonz's layer blending FX on the column's output, blending with the columns below through the Xsheet port; a locked *Flash stage background* color card lies below everything on the document timeline | Editable in the FX schematic |
| Embedded PNG/JPEG and lossless bitmap data | Shared raster levels | |
| Frame labels and comments | Xsheet frame labels | Anchor labels are dropped and reported |
| Static, dynamic and input text | Editable Text FX columns (*Text Iwa* with its new *Transform with Column* option and layout options), placed by the element's stage keys; the glyph outlines stay in a `[text outlines]` reference column shown in the editor and left out of renders | Mixed fonts or colors get one FX per run piece of each line. Flash's paragraph layout maps to the new FX options *Kerning*, *Letter Spacing*, *Line Spacing*, *Indent* and *Shrink to Fit*. The editor viewer draws Text FX only in Preview mode; after editing a Text FX, hide its outlines column |
| Sound on a document timeline or in a movie clip: PCM, MP3 and SWF ADPCM | Sound column per source layer, playing a WAV sound level in `levels/`; movie clip sounds play from top-level columns named `<symbol> / <layer> sound` | Stream, event, start and stop sounds, repeat, loop and in/out points. Envelopes are baked into the WAV samples. MP3 needs FFmpeg |
| Embedded video (VP6 / VP6 alpha) | Raster level of PNG frames in `levels/`, one column per instance; an audible PCM sound track becomes a sound column | Needs FFmpeg |

### Approximated (kept, with a reported difference)

- Gradients that fail validation, have a nearly singular matrix or a focal
  ratio of +/-1, and gradients on rectangle/oval primitives: solid fill in
  the mean color of the stops. Bitmap fills whose bitmap cannot be decoded:
  a solid fill.
- Fill regions the exact topology check cannot certify: matched by sampling
  one interior point per face.
- Dashed, dotted and hairline strokes and stroke scale modes: solid strokes.
  Pixel hinting is not applied.
- Classic tweens with per-property custom ease curves: one curve for all
  properties.
- Motion tweens: Bounce, Spring, wave, random and custom eases are applied
  linearly; filter curves and unknown properties are left out; 3D curves and
  3D instances are reported (the 2D matrix is drawn).
- Layer parenting whose parent changes between keyframes, or whose parent has
  no single symbol instance: keyed on every frame without a native parent.
- Cameras inside symbols or in scenes played from a sub-xsheet, camera color
  effects and filters, and layer depth are not mapped.
- Shape tweens without a matching `MorphShape`: interpolated point by point
  when the keyframes share their edge structure, otherwise by contour
  resampling.
- Motion guides that cannot share one native path with the in-betweens are
  keyed frame by frame; branching guides leave the tween straight.
- Filters: Flash's box blur is drawn by the Blur FX with the same spread;
  gradient glows and bevels use the gradient's end colors; filter lengths
  inside symbols scale with the placement.
- Blend modes difference, invert, overlay, hardlight, alpha and erase are
  drawn in normal mode; antialiased edges blend with OpenToonz's formulas;
  inside graphic symbols and groups blending is an approximation.
- Masks: bitmaps and videos in mask artwork clip by their alpha.
- Cubic (Bezier) editing data: the equivalent quadratic outline that Flash
  stores is used.
- Frame script statements other than the playhead commands are not run;
  movie clips on a scripted main timeline follow its frames.
- Sounds: event sounds playing past the end of their timeline end at its
  last frame; an MP3 decoded to another length is padded or cut.
- Text: a missing font is substituted; vertical text, sub/superscript, HTML,
  borders, hyperlinks and text filters are drawn without that detail. Text
  whose Text FX layout would differ from Flash's by more than half a pixel
  is reported; the outlines column keeps the exact Flash layout.
- Video sound tracks inside symbols, and tracks that are not raw PCM: the
  video is imported without its sound.

### Omitted (not imported, listed in the report)

- Hidden instances and instances with a degenerate (zero-scale) matrix, which
  Flash does not draw either.
- Unknown layer types and elements whose vector conversion fails.
- Videos without embedded data or with a codec other than VP6, and all
  videos when FFmpeg is missing or fails.
- Sounds in graphic symbols (Flash never plays them), in buttons, and movie
  clip sounds no clip on stage reaches.
- Sounds without decodable embedded samples (external sources, Nellymoser,
  Speex, unknown formats), and MP3 when FFmpeg is missing or fails.
- Sound tracks of videos on guide layers, and on hidden layers when hidden
  layers are not published.

Button instances are imported showing their Up frame; their other states are
kept in the shared symbol.

## Using it

Choose **File > Import > Import Flash / Adobe Animate...**, pick the
document, a parent folder and a new folder name, then **Inspect** and
**Import and Open**. A progress bar shows the current step, percent done and
the estimated time left. Options in the dialog:

- **Include unused library symbols**: also import symbols that are not on
  any timeline.
- **Show guide layers inside symbols**: show guide layers everywhere, not
  only on the main timeline.

The import is written to a new folder (existing folders are never replaced)
containing:

- `scene.tnz` and `levels/` (the native scene),
- `source/` (a byte-for-byte copy of the original document, with hashes),
- `import-report.json` (source info, every approximation and omission),
- `field-accounting.json` / `field-bindings.json` (source-to-native bindings).

The scene is an ordinary OpenToonz scene: saving, Save As, editing and undo
work as usual. If the `source/` copy is moved or deleted the scene still
saves; it just loses its link to the original. Symbol instances can be
retimed later in **Windows > Flash Instance**.

Embedded video and MP3 sound are decoded with FFmpeg: the one set in
*Preferences > Import/Export*, else found on the `PATH` or beside the
application. Without it, videos and MP3 sounds are omitted and reported.

## Compatibility

Stock scenes, palettes and drawings are unchanged and keep the stock formats.

- Scenes with Flash instance controls are saved as scene version 71.2 or
  later and need this build to keep those controls.
- Drawings imported with exact source topology are saved as PLI version 151
  frames, which stock OpenToonz cannot read.
- A mask column clipping more than one column and an animated column opacity
  are saved as the column tags `mask_scope` and `opacity_curve`, which older
  builds reject ("unknown tag") rather than misread.
- The Text FX options (*Transform with Column*, *Kerning*, *Letter Spacing*,
  *Line Spacing*, *Indent*, *Shrink to Fit*) are ordinary FX parameters; older
  builds skip them and draw the stock layout. At their defaults the FX draws
  exactly as before.
- *Color Matrix* (`STD_colorMatrixFx`, Image Adjust), *Drop Shadow*
  (`STD_dropShadowFx`, Light) and *Bevel* (`STD_bevelFx`, Light) are ordinary
  stdfx FX available in **Insert FX** for any scene; older builds cannot open
  a scene that uses them.
- Column opacity inside a sub-xsheet now multiplies with the parent column's
  opacity in the viewer.

## OpenToonz extensions added for the import

- Column *mask scope*: a mask column can clip several columns above it.
- Animatable column opacity (an *Opacity* curve under each column).
- Text FX (*Text Iwa*): *Transform with Column* and layout options.
- New FX: *Color Matrix*, *Drop Shadow*, *Bevel*.
- Palette styles `TFlashGradientStyle` (Flash gradients) and
  `TFlashBitmapStyle` (bitmap fills).
- Source-preserving region finding for vector images, stored with a PLI
  region-mode tag; used only for imported drawings.
- Thread-safety fixes in vector region and stroke code, so shapes convert on
  all CPU cores.

## Code map

| Path | Role |
| --- | --- |
| `archive.*` | Bounded ZIP / XFL folder reader (`Flash::Package`): size, CRC, path and budget checks |
| `document.*` | Document index: stage, timelines, library, references, symbol graph and depth checks |
| `model.*` | Parsed animation model: timelines, layers, frames, elements, paints, shapes |
| `geometry.*`, `fill.*`, `gradient.*` | XFL edge decoding, fill evaluation, gradient validation |
| `motion.*` | Motion-object (CS4+) tweens: curves, eases and poses |
| `shapetween.*` | Shape tweens: Animate's saved morph, fallback correspondences, in-between shapes |
| `timing.*`, `placement.*`, `parttracks.h` | Symbol playback clocks, frame script simulation, matrix decomposition, part-track planning |
| `media.*` | Embedded bitmap and sound decoding (PCM, MP3 via FFmpeg, SWF ADPCM), envelope baking |
| `video.*` | Embedded video container parser and FFmpeg frame decoding |
| `effects.*` | Color effects, filters and blend modes: parsing and tween interpolation |
| `fieldaccounting.*` | Inventory of every XML field for provenance |
| `../toonzlib/flashpreflight.*` | Compatibility guards and issue handling (blocking / approximated / omitted) |
| `../toonzlib/flashscene.*`, `flashvector.*`, `flashbitmap.*` | Native scene, vector and raster construction |
| `../toonzlib/flasheffects.*` | Native FX chains and keys for color effects, filters and blend modes |
| `../toonzlib/flashtext.*` | Text: glyph outlines and Text FX parameters |
| `../toonzlib/flashimport.*`, `flashbundle.*`, `flashprovenance.*`, `flashfieldbindings.*`, `flashhandoff.*` | Transactional import bundle, report, provenance and Save As handling |
| `../toonzlib/flashinstance*`, `../toonz/flashinstance*` | Per-instance playback data, undoable edits and the Flash Instance panel |
| `../toonz/flashimportpopup.*` | Import dialog |
| `../stdfx/colormatrixfx.cpp`, `../stdfx/dropshadowfx.cpp` | Color Matrix, Drop Shadow and Bevel FX |
| `../stdfx/iwa_textfx.*`, `../stdfx/iwa_textfxlayout.h` | Text FX *Transform with Column* and layout options; the layout (header only) is shared with the importer |
| `../common/tvrender/tflashgradientstyle.*` | `TFlashGradientStyle` and `TFlashBitmapStyle` palette styles |
| `../common/tvectorimage/tcomputeregions.cpp`, `tregion.cpp` | Source-preserving region finding |
