# Wick Editor projects

Wick Editor (wickeditor.com, Wicklets LLC, GPL-3.0) saves a project as a `.wick` file. Flare imports
it through `flare-import` (`crates/io/flare-import/src/wick/`), by the extension, on every surface:
File > Open on the desktop and in the browser, `flare import` and `import_project` (MCP). Flare does
not write `.wick` files.

## Licence and provenance

Wick Editor is GPL-3.0. Under the reference policy in [ENGINEERING.md](ENGINEERING.md) its code was
read for the file layout and for behaviour only: how each object is serialised, how frames and tweens
are numbered and interpolated, and where a clip's transform is applied. No code was copied, and the
importer is written from those facts. The sources read are listed under "Third-party format references
used" in ENGINEERING.md.

The samples are the 81 example projects in the Wick Editor repository (`public/examples`, all
written by Wick 2020.x). They are GPL-licensed, so none is committed: the real-sample test reads the
files named by `WICK_SAMPLES`. Web searches for other `.wick` files found none that could be
downloaded. The importer's own tests (`src/wick/tests.rs`, and the fixture
`tests/fixtures/wick/synthetic.wick`, which Flare's authors wrote) use synthetic projects in the same
layout.

## Container

A `.wick` file is a zip archive. Wick writes it with a `project.json` at the root and an
`assets/<uuid>.<ext>` file for each image, sound and font.

| Entry | Content |
| --- | --- |
| `project.json` | `{ "project": {...}, "objects": {...} }`: the project's fields, and every object by its `uuid` |
| `assets/<uuid>.png` | an image (every sample's image is PNG) |
| `assets/<uuid>.ogg`, `.wav`, `.mp3` | a sound |
| `assets/<uuid>.ttf` | a font |

Refused, each by name: a gzip file (`1f 8b`; Wick does not write gzip), plain JSON starting with `{`
(the layout of Wick projects before 1.0.9, which is not read), a file that is not a zip, a zip with
no `project.json`, an entry name that would leave the project (absolute, `..`, backslash, drive, or
empty part), and an entry listed twice. A `.wick` that is a zip with `project.json` is read whatever its
name, through `import_bytes`.

## Objects

Each object has a `classname`, a `uuid` and, for a container, a `children` list of uuids. The tree:

- `Project` (`project`): `name`, `width`, `height`, `backgroundColor` (`rgb(35,35,35)`), `framerate`,
  `focus` (the root clip's uuid), `children` (the root clip).
- `Clip` (a movie clip placed in a frame): `identifier`, `transformation` (`x`, `y`, `scaleX`,
  `scaleY`, `rotation` in degrees, `opacity`), `animationType` (`loop`, `playOnce` or `single` with
  `singleFrameNumber`), and `children` holding one `Timeline`. `scripts` is a list of `{name, src}`.
- `Button`: as a clip, with one `Timeline`; its states are the frames of that timeline.
- `Timeline`: `children` are the layers, index 0 on top.
- `Layer`: `name`, `hidden`, `locked`, `children` are the frames.
- `Frame`: `start` and `end`, 1-based and inclusive; `children` are the paths, clips, buttons and
  tweens in the frame; `scripts`, and optionally `name` (a label) and `sound`.
- `Path`: `json` is a Paper.js item, `["Path", props]`, `["CompoundPath", props]` (with inline
  `children`), `["Raster", props]` (`source` is `asset:<uuid>`) or `["PointText", props]`.
- `Tween`: `playheadPosition` (1-based, within the frame), `transformation`, `fullRotations`,
  `easingType` (`none`, `in`, `out`, `in-out`).
- `ImageAsset`, `SoundAsset`, `FontAsset`: `name`, `filename`, `MIMEType`, `fileExtension`.
- `Selection`: editor state; ignored.

Paper.js paths: a segment is a point `[x, y]`, or `[point, handleIn, handleOut]` with the handles
relative to the point. `applyMatrix` true means the points are already in the parent's space; false
means a `matrix` `[a, b, c, d, e, f]` (when present) maps them. Colours are arrays of red, green, blue
and an optional alpha from 0 to 1. `strokeWidth`, `strokeCap` and `strokeJoin` are set only with a
stroke.

## What is imported

| Wick | Flare |
| --- | --- |
| project size and background | the stage (background from `rgb(...)`, white if unreadable) |
| `framerate` | the frame rate |
| the root clip's timeline | the one scene; its name is the project name |
| layers, `hidden`, `locked` | layers, visibility and lock |
| frame `start`..`end` | a span from `start - 1` for `end - start + 1` frames (0-based) |
| an empty frame | an empty keyframe |
| a closed or open path, a compound path | a shape with its solid fill and stroke; cap and join kept |
| a clip or button | a symbol: a Movie Clip for `loop`, a Graphic with Play Once or Single Frame for the others, a Button for a button |
| a clip in a frame | an instance with the clip's transform |
| a PNG raster | a bitmap in the library, shared when identical |
| a tween | a span whose Flare tween moves to the next span, with the easing (see below) |
| a frame's scripts | kept as attachments, each `(not run)` |
| a frame's label | kept as an attachment |

### Tweens

Wick interpolates a clip's transform between its tweens; the earlier tween's easing shapes the motion.
A frame with tweens becomes one span per tween, each holding the clip at that tween's transform, and
the span's Flare tween moves to the next. `none` is linear motion; `in`, `out` and `in-out` map to
Flare's quadratic family with the same name, which is exactly Wick's `Quadratic` easing. A tween
after the last holds to the end of the frame.

Paths and pictures are not tweened in Wick, so they repeat in every span of a tweened run. Only the
first clip of a frame moves, as Flare moves the first element of a span; further clips are held at
their stored transform, and this is reported.

### Clip placement (spec-only)

Wick positions a clip by the centre of its content's bounds: Paper.js rotates and scales a group
about its bounds centre. The importer measures that centre from the clip's own paths, on all its
frames, and places the clip so that `x`, `y` is the centre and rotation and scale turn about it. Wick
measures the bounds of what is drawn at each moment, so a clip whose drawing changes shape can be
placed slightly differently here. This is read from the engine source and has not been compared with
a rendered Wick project.

### Pictures (spec-only)

A raster's `matrix` places its centre, since Paper.js draws a raster about its own centre. The
bitmap's top left is therefore the centre minus half its size, under the matrix. Read from the engine
source; not compared with a rendered project.

## What is not imported

Each is reported as a diagnostic with its count.

- Sounds (`SoundAsset`, a frame's `sound`): no sound importer exists for ogg, wav or mp3.
- Text (`PointText`): not outlined, as XFL text is not; the words and font are lost.
- Fonts (`FontAsset`): not read.
- Gradient fills: not imported; the shape is drawn in its first colour or not at all.
- Opacity of a clip or a tween: Flare instances have no alpha. Reported with its count.
- Full rotations of a tween: the angle keeps only the turn within a circle.
- Button states and events: a button keeps its type; its states and scripts are not run.
- Scripts: kept as text with `(not run)`; Wick runs them and Flare does not.
- Clip assets (a nested `.wick` in `ClipAsset`): not read.
- Pictures that are PNG only: other formats and pictures stored inside the project are not imported.
- Fill rules: Wick's even-odd fill is not carried; shapes fill as Flare fills them.
- Selection, onion skin, cursor and playhead positions: editor state, ignored.

## Limits

Each bound is checked before the work it guards (`src/wick/limits.rs`): 4,096 zip entries; 32 MiB per
entry; 64 MiB for `project.json`; 256 MiB of entries together; 200,000 objects; 100,000 children per
object; 64 levels of clips in clips; 10,000 symbols; 1,024 layers per timeline; frames up to 100,000;
120 frames a second; 512 MiB of decoded pictures. A bad name, a bad frame range, an overlap, an unknown
easing or animation type, a clip that contains itself, and a segment that is not a point are refused by
name, and a refused file changes nothing.

## Verified against

- The 81 real projects in the Wick Editor repository, all 2020.x. Each imports (see the table below),
  run by `WICK_SAMPLES=<folder> cargo test -p flare-import --test wick -- --ignored --nocapture`.
- The object layout above was checked against the samples: every class, field and encoding named here
  appears in them, and the samples contain no other class.
- Synthetic projects in the same layout, with each refusal and each diagnostic, are unit tests in
  `src/wick/tests.rs`. The CLI and MCP tests import `tests/fixtures/wick/synthetic.wick`.

## Real samples

The 81 example projects of the Wick Editor repository, imported by `flare import <file> <out.flare>` on the
current build (rebuilt for this change). Every file imports (exit 0; 0 refused). Columns: the clips and
buttons made into symbols, the scenes, the diagnostics of kind "not imported" (unsupported), of kind
"not carried" (warning), and the scripts kept as text. A non-empty `unsupported` is text, sounds or pictures that
are not imported; `warnings` are buttons keeping only their type, opacity and similar. Totals over the 81 files: 284 symbols, 280 scripts kept as text, 36 unsupported notes, 75 warning notes. The original `.wick` is not kept inside the Flare project (`original_kept` is false): the project is an import, not a round trip.

| Sample | Symbols | Scenes | Unsupported | Warnings | Scripts kept | CLI |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| DJSpookyman | 17 | 1 | 1 | 1 | 10 | ok |
| WellDressedGhost | 16 | 1 | 1 | 1 | 16 | ok |
| biscuitDestroyer | 1 | 1 | 1 | 1 | 5 | ok |
| dragAndDrop | 1 | 1 | 1 | 0 | 1 | ok |
| followMouse | 4 | 1 | 1 | 2 | 1 | ok |
| hauntedHouse | 84 | 1 | 2 | 3 | 54 | ok |
| multipleRooms | 26 | 1 | 1 | 3 | 8 | ok |
| playingClips | 24 | 1 | 1 | 3 | 7 | ok |
| playingTheTimeline | 5 | 1 | 1 | 3 | 2 | ok |
| popups | 7 | 1 | 1 | 3 | 4 | ok |
| reference-Default-script | 0 | 1 | 1 | 0 | 1 | ok |
| reference-Keydown-script | 1 | 1 | 0 | 1 | 2 | ok |
| reference-Keypressed-script | 1 | 1 | 0 | 1 | 2 | ok |
| reference-Keyreleased-script | 1 | 1 | 0 | 1 | 2 | ok |
| reference-Load-script | 1 | 1 | 1 | 1 | 5 | ok |
| reference-Mouseclick-script | 1 | 1 | 0 | 1 | 2 | ok |
| reference-Mousedown-script | 1 | 1 | 0 | 1 | 2 | ok |
| reference-Mousedrag-script | 1 | 1 | 0 | 1 | 2 | ok |
| reference-Mouseenter-script | 1 | 1 | 0 | 1 | 2 | ok |
| reference-Mousehover-script | 1 | 1 | 0 | 1 | 2 | ok |
| reference-Mouseleave-script | 1 | 1 | 0 | 1 | 2 | ok |
| reference-Mousepressed-script | 1 | 1 | 0 | 1 | 2 | ok |
| reference-Mousereleased-script | 1 | 1 | 0 | 1 | 2 | ok |
| reference-Update-script | 1 | 1 | 0 | 1 | 2 | ok |
| reference-clone | 2 | 1 | 1 | 1 | 2 | ok |
| reference-clones | 2 | 1 | 1 | 1 | 2 | ok |
| reference-currentFrameName | 4 | 1 | 1 | 1 | 5 | ok |
| reference-currentFrameNumber | 4 | 1 | 1 | 1 | 5 | ok |
| reference-gotoAndPlay | 1 | 1 | 1 | 1 | 2 | ok |
| reference-gotoAndStop | 1 | 1 | 1 | 1 | 2 | ok |
| reference-gotoNextFrame | 2 | 1 | 1 | 1 | 3 | ok |
| reference-gotoPrevFrame | 2 | 1 | 1 | 1 | 3 | ok |
| reference-height | 1 | 1 | 0 | 1 | 2 | ok |
| reference-hideCursor | 1 | 1 | 0 | 0 | 3 | ok |
| reference-hitTest | 2 | 1 | 0 | 1 | 3 | ok |
| reference-hits | 19 | 1 | 0 | 1 | 16 | ok |
| reference-ifhitTest | 2 | 1 | 0 | 1 | 3 | ok |
| reference-ifkey | 1 | 1 | 0 | 0 | 2 | ok |
| reference-isKeyDown | 1 | 1 | 0 | 0 | 2 | ok |
| reference-isKeyJustPressed | 1 | 1 | 0 | 0 | 2 | ok |
| reference-isMouseDown | 1 | 1 | 0 | 0 | 2 | ok |
| reference-key | 0 | 1 | 1 | 0 | 2 | ok |
| reference-keydown | 1 | 1 | 0 | 1 | 2 | ok |
| reference-keypressed | 1 | 1 | 0 | 1 | 2 | ok |
| reference-keyreleased | 1 | 1 | 0 | 1 | 2 | ok |
| reference-keys | 0 | 1 | 1 | 0 | 2 | ok |
| reference-load | 1 | 1 | 1 | 1 | 4 | ok |
| reference-mouseMoveX | 1 | 1 | 0 | 1 | 2 | ok |
| reference-mouseMoveY | 1 | 1 | 0 | 1 | 2 | ok |
| reference-mouseX | 1 | 1 | 0 | 1 | 2 | ok |
| reference-mouseY | 1 | 1 | 0 | 1 | 2 | ok |
| reference-mouseclick | 1 | 1 | 0 | 1 | 2 | ok |
| reference-mousedown | 1 | 1 | 0 | 1 | 2 | ok |
| reference-mousedrag | 1 | 1 | 0 | 1 | 2 | ok |
| referenc

## Real samples

The 81 example projects of the Wick Editor repository, imported by `flare import <file> <out.flare>`.
Every file imports (exit 0, none refused). Columns: the clips and buttons made into symbols, the scenes,
the diagnostics of kind "not imported" (unsupported) and "not carried" (warning), and the scripts kept as
text. Unsupported is text, sounds or pictures not imported; warnings are buttons keeping only their type,
opacity and similar. Totals over the 81 files: 284 symbols, 280 scripts kept as text, 36 unsupported notes, 75 warning notes.
The original `.wick` is not kept inside the Flare project (the report's `original_kept` is false): the
project is an import, not a round trip.

| Sample | Symbols | Scenes | Unsupported | Warnings | Scripts kept | CLI |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| DJSpookyman | 17 | 1 | 1 | 1 | 10 | ok |
| WellDressedGhost | 16 | 1 | 1 | 1 | 16 | ok |
| biscuitDestroyer | 1 | 1 | 1 | 1 | 5 | ok |
| dragAndDrop | 1 | 1 | 1 | 0 | 1 | ok |
| followMouse | 4 | 1 | 1 | 2 | 1 | ok |
| hauntedHouse | 84 | 1 | 2 | 3 | 54 | ok |
| multipleRooms | 26 | 1 | 1 | 3 | 8 | ok |
| playingClips | 24 | 1 | 1 | 3 | 7 | ok |
| playingTheTimeline | 5 | 1 | 1 | 3 | 2 | ok |
| popups | 7 | 1 | 1 | 3 | 4 | ok |
| reference-Default-script | 0 | 1 | 1 | 0 | 1 | ok |
| reference-Keydown-script | 1 | 1 | 0 | 1 | 2 | ok |
| reference-Keypressed-script | 1 | 1 | 0 | 1 | 2 | ok |
| reference-Keyreleased-script | 1 | 1 | 0 | 1 | 2 | ok |
| reference-Load-script | 1 | 1 | 1 | 1 | 5 | ok |
| reference-Mouseclick-script | 1 | 1 | 0 | 1 | 2 | ok |
| reference-Mousedown-script | 1 | 1 | 0 | 1 | 2 | ok |
| reference-Mousedrag-script | 1 | 1 | 0 | 1 | 2 | ok |
| reference-Mouseenter-script | 1 | 1 | 0 | 1 | 2 | ok |
| reference-Mousehover-script | 1 | 1 | 0 | 1 | 2 | ok |
| reference-Mouseleave-script | 1 | 1 | 0 | 1 | 2 | ok |
| reference-Mousepressed-script | 1 | 1 | 0 | 1 | 2 | ok |
| reference-Mousereleased-script | 1 | 1 | 0 | 1 | 2 | ok |
| reference-Update-script | 1 | 1 | 0 | 1 | 2 | ok |
| reference-clone | 2 | 1 | 1 | 1 | 2 | ok |
| reference-clones | 2 | 1 | 1 | 1 | 2 | ok |
| reference-currentFrameName | 4 | 1 | 1 | 1 | 5 | ok |
| reference-currentFrameNumber | 4 | 1 | 1 | 1 | 5 | ok |
| reference-gotoAndPlay | 1 | 1 | 1 | 1 | 2 | ok |
| reference-gotoAndStop | 1 | 1 | 1 | 1 | 2 | ok |
| reference-gotoNextFrame | 2 | 1 | 1 | 1 | 3 | ok |
| reference-gotoPrevFrame | 2 | 1 | 1 | 1 | 3 | ok |
| reference-height | 1 | 1 | 0 | 1 | 2 | ok |
| reference-hideCursor | 1 | 1 | 0 | 0 | 3 | ok |
| reference-hitTest | 2 | 1 | 0 | 1 | 3 | ok |
| reference-hits | 19 | 1 | 0 | 1 | 16 | ok |
| reference-ifhitTest | 2 | 1 | 0 | 1 | 3 | ok |
| reference-ifkey | 1 | 1 | 0 | 0 | 2 | ok |
| reference-isKeyDown | 1 | 1 | 0 | 0 | 2 | ok |
| reference-isKeyJustPressed | 1 | 1 | 0 | 0 | 2 | ok |
| reference-isMouseDown | 1 | 1 | 0 | 0 | 2 | ok |
| reference-key | 0 | 1 | 1 | 0 | 2 | ok |
| reference-keydown | 1 | 1 | 0 | 1 | 2 | ok |
| reference-keypressed | 1 | 1 | 0 | 1 | 2 | ok |
| reference-keyreleased | 1 | 1 | 0 | 1 | 2 | ok |
| reference-keys | 0 | 1 | 1 | 0 | 2 | ok |
| reference-load | 1 | 1 | 1 | 1 | 4 | ok |
| reference-mouseMoveX | 1 | 1 | 0 | 1 | 2 | ok |
| reference-mouseMoveY | 1 | 1 | 0 | 1 | 2 | ok |
| reference-mouseX | 1 | 1 | 0 | 1 | 2 | ok |
| reference-mouseY | 1 | 1 | 0 | 1 | 2 | ok |
| reference-mouseclick | 1 | 1 | 0 | 1 | 2 | ok |
| reference-mousedown | 1 | 1 | 0 | 1 | 2 | ok |
| reference-mousedrag | 1 | 1 | 0 | 1 | 2 | ok |
| reference-mouseenter | 1 | 1 | 0 | 1 | 2 | ok |
| reference-mousehover | 1 | 1 | 0 | 1 | 2 | ok |
| reference-mouseleave | 1 | 1 | 0 | 1 | 2 | ok |
| reference-mousepressed | 1 | 1 | 0 | 1 | 2 | ok |
| reference-mousereleased | 1 | 1 | 0 | 1 | 2 | ok |
| reference-opacity | 1 | 1 | 1 | 1 | 2 | ok |
| reference-parent | 2 | 1 | 0 | 1 | 2 | ok |
| reference-play | 0 | 1 | 1 | 0 | 1 | ok |
| reference-playSound | 2 | 1 | 1 | 1 | 3 | ok |
| reference-project.framerate | 1 | 1 | 1 | 0 | 2 | ok |
| reference-project.height | 1 | 1 | 0 | 0 | 2 | ok |
| reference-project.width | 1 | 1 | 0 | 0 | 2 | ok |
| reference-random.choice | 1 | 1 | 1 | 1 | 2 | ok |
| reference-random.float | 1 | 1 | 0 | 0 | 2 | ok |
| reference-random.integer | 1 | 1 | 0 | 0 | 2 | ok |
| reference-remove | 1 | 1 | 1 | 1 | 2 | ok |
| reference-rotation | 1 | 1 | 1 | 1 | 2 | ok |
| reference-scaleX | 1 | 1 | 1 | 1 | 2 | ok |
| reference-scaleY | 1 | 1 | 1 | 1 | 2 | ok |
| reference-setText | 1 | 1 | 1 | 1 | 2 | ok |
| reference-showCursor | 1 | 1 | 0 | 0 | 3 | ok |
| reference-stop | 0 | 1 | 1 | 0 | 1 | ok |
| reference-stopAllSounds | 2 | 1 | 1 | 1 | 3 | ok |
| reference-update | 1 | 1 | 0 | 1 | 2 | ok |
| reference-width | 1 | 1 | 0 | 1 | 2 | ok |
| reference-x | 1 | 1 | 0 | 1 | 2 | ok |
| reference-y | 1 | 1 | 0 | 1 | 2 | ok |
