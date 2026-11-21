# Industry formats: what Flare reads, and what it sends you back to

The formats animators meet outside Flash, in the tools studios use beside Animate: Toon Boom
Harmony, After Effects, TVPaint, Clip Studio Paint, Blender, Illustrator and Character Animator.
[FLASH-FORMATS.md](FLASH-FORMATS.md) covers the Flash family; [MOHO.md](MOHO.md) covers Moho and
BVH; [SURFACES.md](SURFACES.md) says which surface can do what.

Checked on 2026-11-20. "Verified" means the reader ran on a real file of that kind. "Refused by
name" means Flare knows the extension and answers with a message saying what to do instead;
nothing is guessed or half-imported.

## Read

| Format | Tool | Reader | Status | What is read |
|---|---|---|---|---|
| `.xstage` | Toon Boom Harmony | `flare-import` `harmony.rs` | Verified against the Harmony 21 scene layout `harmonyparser` documents; synthetic fixtures in `crates/io/flare-import/tests/harmony.rs` | The stage resolution (`<resolution size="W,H">`), the frame rate (`<framerate val>`), the scene name and its frame count (`nbframes`, else `stopFrame - startFrame + 1`), and the scene's columns as layers in `displayOrder`. Each column becomes a layer holding one empty frame. |
| `.fla`, `.xfl` | Adobe Animate | `flare-import` `xfl/`, `source.rs` | Verified | See [FLASH-FORMATS.md](FLASH-FORMATS.md). |
| `.moho`, `.anme` | Moho | `flare-moho` | Verified | See [MOHO.md](MOHO.md). |
| `.psd` | Photoshop | `flare-import` `psd.rs` | Verified | One bitmap per layer, visibility and opacity. |
| `.svg` | Any vector tool | `flare-import` `svg.rs` | Verified | Shapes, gradients and text as outlines (usvg). |
| `.png`, `.tga`, `.tiff` (and JPEG, GIF, BMP, WebP, EXR, …) | Any | `flare-import` `bitmap.rs` | Verified | A picture as a one-frame bitmap; numbered sequences as an animation. |
| `.mp4`, `.mov` (and AVI, MKV, WebM, MPEG, FLV, F4V, F4P) | Any | `flare-import` `video.rs` through `flare-ffmpeg` | Built | One bitmap keyframe per picture at the movie's rate; the sound is not imported. |
| `.wav`, `.mp3` (and Ogg, FLAC) | Any | `flare-runtime` `sound_layer` | Built | Sound import, not project import: a layer with one cue (`flare sound-import`, Import > Sound). |

### What a Harmony scene leaves behind

The `.xstage` file is the scene's XML index; the drawings (`.tvg` vector, `.pli` raster), palettes
and sounds live beside it in the scene's folder, and Flare does not read them yet. The import says
so in a note. Open the scene in Harmony and export a PNG or PSD sequence, or an image sequence, to
bring the artwork over today.

## Refused by name

Each of these is in `flare_import::file_kinds::UNSUPPORTED`, so every surface — File > Open, the
browser picker, `flare import`, `import_project` — gives the same answer instead of a generic
"not a kind Flare opens".

| Format | Tool | Why it is refused, and what to do instead |
|---|---|---|
| `.ai` | Illustrator | Adobe's PDF-based artwork; Flare has no PDF reader. In Illustrator use **Save As → SVG**, which Flare imports. |
| `.aep` | After Effects | Adobe's binary project. Render a PNG sequence or a movie from After Effects and import that. |
| `.tvpp`, `.tvp` | TVPaint | TVPaint's binary project. Export a PNG or PSD sequence, or JSON with its images, from TVPaint and import that. |
| `.clip` | Clip Studio Paint | A SQLite database of proprietary drawing chunks. Export a PSD or a PNG sequence from Clip Studio Paint and import that. |
| `.blend` | Blender | A 3D scene. Render a PNG sequence, or export glTF (Flare does not read glTF yet). |
| `.puppet`, `.chproj` | Character Animator | Adobe's self-contained puppet and scene files. Export a PNG sequence from Character Animator, or import the artwork's PSD or SVG. |
| `.vbd` | Harmony | The compact Harmony scene (version 25 and later). Open it in Harmony and save a regular `.xstage`. |
| `.xstage~` | Harmony | Harmony's backup of a scene; open the `.xstage` beside it. |

## Sources and licences

| Source | Licence | Use |
|---|---|---|
| `PoloB/harmonyparser` | `MIT` (`LICENSE`, Copyright 2024 Paul Boiste) | Format facts for the Harmony 21 `.xstage` layout (element names, `displayOrder`, `nbframes`, `size`, `framerate`). No code is copied; the reader is new Rust. Its sample scene was read in place, not committed. |

The Adobe, TVPaint, Blender and Clip Studio formats above were not read from a published
specification: their refusals state what the tools are and the export route, which is verifiable
from each tool's own documentation. When a reader for one is started, its sources and licence go
in [BACKLOG.md](BACKLOG.md) first.

## Next

1. Harmony drawings: `.tvg` vector levels beside the `.xstage`, after the level readers Flare
   already has for OpenToonz (`.pli`, `.tlv`).
2. Illustrator `.ai` when saved with PDF compatibility: a bounded PDF content-stream reader for
   paths, which would also open PDF artwork generally.
3. Blender and other 3D: glTF first (it is the interchange format Blender writes), then a Blender
   file reader if it is still wanted.
