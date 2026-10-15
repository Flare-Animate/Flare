# Scripting Flare

Flare has two kinds of script. They look similar but have very
different jobs and safety rules, so they are kept apart on purpose.

| | Frame scripts | Authoring scripts |
|---|---|---|
| Animate equivalent | Actions panel code (ActionScript / HTML5 Canvas JS) | JSFL commands |
| Lives in | the document, on a keyframe | a file on your computer, or the Script Console |
| Language | JavaScript (CreateJS-style API) | JavaScript (built in), Python, Node.js, Ruby, or any language |
| Runs | during playback only | when *you* run it |
| Can do | `stop`, `play`, `gotoAndPlay/Stop`, `trace` | everything the menus can do: draw, edit, timeline, library, export, save |
| Safety | sandboxed: no files, network or editor; 100 ms + 32 MB per frame | external languages run with your permissions; the app asks first |

## Frame scripts (Window ▸ Actions, F9)

Select a keyframe, press **F9**, write code, **Apply** (Ctrl+Enter). The
**Insert snippet** menu has the common ones. Control ▸ **Enable Frame Scripts**
turns them on or off during playback.

```js
// `this` is the timeline, as in Animate's HTML5 Canvas documents.
this.stop();
this.gotoAndPlay("start");          // frame label…
this.gotoAndStop(0);                // …or 0-based frame number (HTML5 Canvas numbering)
trace("frame", this.currentFrame, "of", this.totalFrames);
this.loops = (this.loops || 0) + 1; // `this.x` persists between frames
```

Frame scripts never run when a document is opened, cannot read or write
files, cannot reach the network, and cannot change the document. A runaway
loop is stopped after 100 ms and memory is capped at 32 MB. That is what makes
it safe to open files other people send you.

### Instances in frame scripts

Give a symbol instance (or text, bitmap or group) an **instance name** in
Properties, and frame scripts can read and change it while the movie plays,
CreateJS-style:

```js
this.ball.x += 10;
this.getChildByName("ball").alpha = 0.5;
if (this.ball.x > 500) this.ball.visible = false;
```

Supported properties: `x`, `y`, `rotation` (degrees), `scaleX`, `scaleY`,
`alpha`, `visible`, and `text` on text fields (`this.score.text = "Score: " + n;`
for Dynamic text; Input text holds what the viewer typed). Changes last until
playback stops; the document itself is never modified.

In a published **HTML5 Canvas** page the same scripts run in the browser,
and named instances (buttons included) also take mouse events:

```js
this.go.addEventListener("click", () => this.gotoAndPlay("start"));
this.go.on("mouseover", (e) => { e.target.alpha = 0.7; });
```

## Authoring scripts (Commands ▸ Script Console)

Pick a language, write or load a script, press **Run**. A whole run is **one
undo step**. Every language gets the same commands (Commands ▸ Command
Reference lists them all):

```js
// JavaScript — built in, nothing to install
ac.layer.add({ name: "Stars" });
ac.draw.polystar({ cx: 100, cy: 100, r: 30, star: true, fill: "#FFC41F" });
ac.select.layer({ layer: "Stars" });
ac.selection.rotate({ degrees: 30 });
ac.export.gif({ path: "out.gif" });
```

```python
# Python — uses the Python installed on your computer
import flare as ac
ac.layer.add(name="Stars")
ac.draw.polystar(cx=100, cy=100, r=30, star=True, fill="#FFC41F")
info = ac.doc.info()
print(info["frames"], "frames")
```

```js
// Node.js (.mjs)
import ac from "./flare.mjs";
ac.draw.rect({ x: 10, y: 10, w: 80, h: 40, radius: 8 });
```

```ruby
# Ruby
require "flare"
Flare.call("draw.text", x: 20, y: 20, text: "Hello", size: 32)
```

Run a file from the console's **Script file** box, or headless:

```sh
cargo run -p ac-cli -- run examples/scripts/stars.py --gif stars.gif
cargo run -p ac-cli -- run examples/scripts/grid.js --doc demo --save out.acx.json
cargo run -p ac-cli -- commands          # list every command + the protocol
```

Conventions: frames and layers are numbered from 1 (as in the timeline);
colours are `"#RRGGBB"` or `"#RRGGBBAA"`, `null` means none; relative paths
are relative to the script's folder.

### Transforms, 3D, gradients, bones and the camera

The tools added in M0.3 are scriptable too (coordinates are stage pixels,
frames are 1-based):

```js
ac.selection.skew({ h: 20, v: 0 });                    // degrees, about the selection centre
ac.selection.distort({ corners: [[0,0],[120,10],[110,90],[0,100]] }); // TL, TR, BR, BL (shapes)
ac.selection.set_3d({ rx: 0, ry: 45, rz: 0, z: 80 });  // 3D rotation + depth
ac.doc.set({ perspective_angle: 70, vanishing_x: 275, vanishing_y: 200 });
ac.selection.set_gradient({ kind: "radial", colors: ["#FFFFFF", "#2D8CEB"] }); // or "linear" / "none"
ac.bone.add({ x1: 100, y1: 100, x2: 180, y2: 100 });   // from the object at (x1,y1) to the one at (x2,y2)
ac.bone.drag({ x: 260, y: 100, to_x: 220, to_y: 180 }); // pose with inverse kinematics
ac.camera.set({ x: 275, y: 200, zoom: 1.5, rotation: 10, frame: 24 });
ac.camera.mode({ mode: "free" });                        // "stage" (Animate's camera) or "free" (VCam)
ac.camera.options({ follow: "hero", follow_lag: 4, offset_x: 40, shake: 6, shake_speed: 10, frame: 1 });
ac.camera.effect({ brightness: 0.3, tint: "#FF8800", tint_amount: 0.2, alpha: 1, blur: 4, frame: 24 });
```

Camera options belong to the camera span they're set on (a new camera keyframe
inside the span inherits them). `follow` names a top-level instance; the camera
centres on it, averaged over `follow_lag` frames (≤24) and shifted by the
offset. Shake is deterministic, so every export shows the same motion.

### Layers, groups, alignment, filters, import

```js
ac.layer.wrap({ kind: "mask", layer: "Photo" });       // a mask above "Photo"
ac.layer.wrap({ kind: "guide", layer: "Ball" });       // classic motion guide
ac.layer.set({ layer: "Sun", parent: "Sky" });          // put a layer in a folder
ac.timeline.set_tween({ kind: "classic", orient: true, layer: "Ball" });
ac.select.all();
ac.selection.group();                                   // ungroup: selection.ungroup
ac.selection.align({ op: "hcenter", to_stage: true });
ac.selection.add_filter({ type: "drop_shadow", distance: 8, color: "#203040" });
ac.selection.set_blend({ mode: "multiply" });
ac.import.image({ path: "badge.png", x: 20, y: 20 });  // PNG/JPEG/GIF/BMP/TGA/TIFF/WebP/EXR into the library + stage
ac.draw.text({ x: 40, y: 40, text: "Hi", font: "DejaVu Serif" }); // any installed font
```

### Motion blur, EffectCraft effects, Trace Bitmap, layer clipboard

```js
ac.selection.add_filter({ type: "motion_blur", shutter: 180, samples: 12 }); // blur along the real movement
// any EffectCraft effect by id (`flare-cli effects blur` lists them with their parameters);
// numbers, booleans, [x, y] points (fractions of the object's bounds) and "#RRGGBBAA" colours
ac.selection.add_filter({ type: "effect", id: "ec.blur.radial", params: { amount: 30, type: 1, center: [0.5, 0.5] } });
ac.select.all();
ac.bitmap.trace({ color_threshold: 100, min_area: 8, curve_fit: "smooth", corners: "normal" }); // → fills
ac.timeline.paste_frames({ frame: 10 });                  // later frames move along
ac.timeline.paste_frames({ frame: 10, overwrite: true }); // Paste and Overwrite Frames
ac.layer.copy({ layer: "Rig" });                          // with everything inside the folder
ac.layer.paste({ layer: "Background" });                  // pasted above it
```

Timeline semantics: by default splitting a tween works as in Animate. A host can
switch on Flare's curve-retaining behaviour (`frames::set_retain_tween_curves`,
Modify › Timeline › Keep Tween Curves When Splitting); scripts run on the same thread
as the app, so they follow that setting.

### Eases, motion tweens, scenes and buttons

```js
// eases: a preset, a custom curve [x, y, in_dx, in_dy, out_dx, out_dy], or "none"
ac.timeline.set_ease({ ease: "Bounce Out", layer: "Ball", frame: 1 });
ac.timeline.set_ease({ ease: [[0, 0, 0, 0, 0.4, 0], [1, 1, -0.1, 0, 0, 0]] });

// motion tween: one object on a span, property keyframes (frames are 1-based)
ac.timeline.set_tween({ kind: "motion", layer: "Ball", frame: 1, rotate: 1 });
ac.motion.set_key({ prop: "x", frame: 24, value: 400 });
ac.motion.set_key({ prop: "rotation", frame: 12, value: 90 });
ac.motion.set_key({ prop: "alpha", frame: 24, value: 0.2 });   // scale/alpha 0..1
ac.motion.set_ease({ prop: "position", ease: "Sine In Out" });
ac.motion.info({ layer: "Ball", frame: 1 });                    // keys per property

ac.timeline.reverse_frames({ layer: 1, frame: 1, to_frame: 24 });
ac.timeline.to_frame_by_frame({ layer: 1, frame: 1, to_frame: 24 });
ac.timeline.set_layer_end({ layer: "Ball", end: 48 });          // motion keys stretch
ac.shape_hint.add({ x: 0, y: 0, to_x: 100, to_y: 0 });          // on a shape tween's first keyframe

ac.layer.set_parent({ layer: "Hat", parent: "Head" });          // layer parenting (0 = none)
ac.scene.add({ name: "Credits" });                              // scene.list / goto / rename / move / delete
ac.selection.swap_symbol({ symbol: "Ball 2" });
ac.selection.color_effect({ style: "advanced", mul: [100, 60, 60, 100], add: [40, 0, 0, 0] });
ac.selection.color_effect({ style: "brightness", amount: -30 });
ac.selection.set_looping({ mode: "loop_reverse", first_frame: 5 });   // loop | play_once | single_frame | play_reverse_once
ac.timeline.set_label({ label: "todo", type: "comment" });           // name | comment | anchor
ac.timeline.set_tween_options({ sync: true, snap: false });          // classic tween Sync / Snap
ac.timeline.set_property_ease({ property: "rotation", ease: "Back Out" }); // position | rotation | scale | color | filters
ac.timeline.set_span_end({ layer: 1, frame: 3, end: 20 });           // any span; later keyframes move
ac.motion.set_key({ prop: "rotation_y", frame: 24, value: 60 });     // also rotation_x, depth, tint
ac.motion.set_key({ prop: "filter1.blur_x", frame: 24, value: 20 }); // filterN.blur_x|blur_y|strength|angle|distance|…
ac.timeline.copy_frames({ layer: 1, frame: 1, to_frame: 12 });       // cut_frames / paste_frames
ac.group.edit();                                                    // edit the selected group; group.exit closes it
```

Exports (`export.gif`, `export.video`, …) play every scene in order; `doc.info`
reports `movie_frames` and `scenes` next to the current timeline's `frames`.

### Merge drawing, primitives, strokes, shape bones

```js
ac.drawing.mode({ merge: true });                 // draw.* now cut and merge into the layer's shape
ac.draw.rect({ x: 20, y: 20, w: 120, h: 100, fill: "#3080E0", stroke: "#101010" });
ac.draw.oval({ x: 90, y: 60, w: 110, h: 110, fill: "#F0B020" });
ac.merge.select({ x: 160, y: 140 });              // pick a fill (connected: true = double-click)
ac.selection.move({ dx: 40, dy: 40 });
ac.merge.deselect();                              // it merges where it was dropped
ac.merge.erase({ x: 30, y: 30, w: 30, h: 30, mode: "fills" });
ac.merge.bucket({ x: 60, y: 60, color: "#FF0000", gap: 6 });
ac.drawing.mode({ merge: false });

ac.draw.rect_primitive({ x: 260, y: 30, w: 120, h: 80, radii: [30, 0, -20, 10] });
ac.selection.stroke_style({ dash: "dashed", cap: "none", join: "miter" });
ac.selection.bitmap_fill({ bitmap: "texture.png" });
ac.selection.combine({ op: "punch" });            // union / intersect / punch / crop
ac.selection.shape({ op: "smooth" });             // smooth / straighten / optimize
ac.selection.stroke_style({ dash: "hatched" });   // also ragged / stipple
ac.selection.set_gradient({ kind: "linear", colors: ["#000000", "#FFFFFF"], flow: "reflect" }); // extend | reflect | repeat
ac.selection.break_apart();                       // text → letters → shapes; bitmap → bitmap-filled shape
ac.select.magic_wand({ x: 120, y: 80, threshold: 20 }); // on a broken-apart bitmap
ac.selection.add_filter({ type: "gradient_glow", kind: "outer", stops: [[0, "#FFFFFF00"], [1, "#FF8000FF"]] });
ac.draw.text({ x: 40, y: 140, text: "Title", font: "DejaVu Sans", weight: 700, italic: true });

ac.shape_bone.add({ x1: 385, y1: 172, x2: 460, y2: 172 });  // inside a drawing object
ac.shape_bone.pose({ x: 460, y: 172, to_x: 430, to_y: 240 });
```

### OpenToonz features: styles, deformers, assistants, painting, expressions, IK, timing sheets

```js
// stroke styles with their parameters (names as in Properties; `selection.stroke_style` lists them on error)
ac.selection.stroke_style({ dash: "rope", params: { tilt: 0.8 }, colors: ["#402000"] });
ac.selection.stroke_style({ dash: "zigzag", params: { "min distance": 2, "max distance": 8, thickness: 1.5 } });
// herringbone | chain | circlets | scratchy | rope | tulle | plait | fuzz | bubbles | jagged | zigzag | wave | curl

// deformation tools: one drag from (x, y) to (to_x, to_y); size = radius / reach in px
ac.shape.deform({ tool: "magnet", x: 200, y: 150, to_x: 200, to_y: 110, size: 40 });
ac.shape.deform({ tool: "pinch", x: 300, y: 150, to_x: 300, to_y: 190, size: 30, sharp: true });
ac.shape.deform({ tool: "pump", x: 120, y: 150, to_x: 120, to_y: 110, size: 25 });     // up = thicker
ac.shape.deform({ tool: "iron", x: 100, y: 150, to_x: 300, to_y: 150, size: 30, strength: 0.6 });
// bender: (x, y)–(x2, y2) is the segment across the lines; the drag turns what lies on its side
ac.shape.deform({ tool: "bender", x: 400, y: 120, x2: 400, y2: 180, from_x: 440, from_y: 150, to_x: 400, to_y: 230 });

// drawing assistants (stored in the document); draw.path with assist:true goes through them
ac.assistant.add({ type: "star", x: 275, y: 200, count: 8, mirror: true });
// guides: line | vanishing_point | ellipse | perspective | fisheye; replicators: mirror | star | grid | affine; jitter
ac.assistant.add({ type: "line", x: 100, y: 100, parallel: true, magnetism: 1 });
ac.assistant.add({ type: "perspective", x: 275, y: 200, three_point: true });   // points: vx, vy, vz
ac.assistant.add({ type: "affine", x: 275, y: 200, count: 12 });                 // points: c0, a0, b0 → c1, a1, b1
ac.assistant.add({ type: "jitter", period: 30, amplitude: 6, keep_first: true, keep_last: true });
ac.draw.path({ points: [[275, 150], [290, 120], [300, 90]], smooth: true, assist: true });
ac.assistant.set({ index: 1, enabled: false });      // also points, count(_inv), mirror, parallel, concentric, three_point, count_a/_b(_inv), period, amplitude
ac.assistant.list(); ac.assistant.remove({ index: "all" });

// Raster Brush: MyPaint brushes painting into the bitmap under the stroke (or a new one on this keyframe)
ac.paint.brushes();                                   // ids, names, groups
ac.paint.stroke({ brush: "classic_charcoal", color: "#203040", size: 6, points: [[50, 50], [120, 80, 0.9], [200, 60]] }); // [x, y, pressure?]
ac.paint.stroke({ brush: "classic_pen", eraser: true, size: 10, points: [[100, 40], [100, 90]] });

// Trace Bitmap, centerline mode: line art → lines with the ink's width
ac.bitmap.trace({ mode: "centerline", line_threshold: 128, curve_fit: "smooth" });

// Motion Editor keyframe types (OpenToonz): on the segment that starts at `key` (1-based frame)
ac.motion.set_interp({ prop: "rotation", key: 1, type: "ease_in_out", ease_in: 6, ease_out: 6 });
// constant | linear | ease_in_out | ease_in_out_pct | exponential | expression | similar_shape | auto
ac.motion.set_interp({ prop: "alpha", key: 1, type: "expression", expression: "100 - rotation / 2" });
ac.motion.set_interp({ prop: "scale_x", key: 1, type: "similar_shape", expression: "abs(sin(frame * 20))" }); // not on x/y (the path)

// joint constraints (degrees from the pose when the bone was made)
ac.bone.joint({ x: 250, y: 10, min: -30, max: 45 });   // the object attached at that point
ac.bone.joint({ x: 150, y: 10, rotate: false });       // a locked joint
ac.shape_bone.joint({ x: 195, y: 10, min: -45, max: 45 });
ac.bone.joint({ x: 150, y: 10, pin: true });           // pinned: stays put while IK poses the rest
// IK is damped least squares (OpenToonz): drags pull the chain, pins pull back harder

// Asset Warp on a bitmap, symbol or group: each pin takes the point under (x, y) to (to_x, to_y)
ac.selection.mesh_warp({ pins: [[20, 20, 20, 20], [60, 60, 80, 85]] });
ac.selection.mesh_warp({ reset: true });

// XDTS timing sheets: columns named like layers re-time them (drawing n = the n-th drawing);
// other columns load name_0001.png… next to the sheet, or become labelled blank keyframes
ac.import.xdts({ path: "cut01.xdts" });   // → { summary, retimed, from_images, timing_only, missing }
```

Expressions use OpenToonz's syntax: `+ - * / ^ %`, comparisons, `and or not`,
`c ? a : b`; `sin cos tan` (degrees), `atan atan2`, `abs sign sqrt sqr exp log floor
ceil round min max clamp step smoothstep pulse saw wave`, `random(min, max)`,
`random_s(seed, …)`, `random_p(period, …)`, `cycle(period)`. Variables: `t` (0–1
across the segment), `frame` and `rframe` (1-based: in the tween span and in the
segment), `v0`/`v1` (the segment's key values) and the object's other properties
(`x y rotation skew scale_x scale_y alpha brightness rotation_x rotation_y depth
tint`, optionally at another frame: `rotation(frame - 3)`), all in displayed units
(degrees, %, px).

### Particles

```js
// an emitter on the current layer; particles start at its keyframe and every export draws them
ac.draw.particles({ x: 275, y: 380, rate: 60, life: 1.2, speed: 160, direction: -90, spread: 50,
                    gravity: [0, 220], color_start: "#FFD040", color_end: "#FF402080", scale_end: 20 });
ac.draw.particles({ x: 100, y: 100, source: { symbol: "Leaf" }, rate: 4, life: 4, spin: 90, tint: false });
ac.selection.particles({ rate: 120, seed: 3, prewarm: true });   // change selected emitters
// source: dot | square | star | {symbol} | {bitmap}; also life_var, speed_var, drag, spin_var, random_angle,
// scale_start/scale_end (%), scale_var, fade_in/fade_out (fractions of life), area [w, h], duration (s), max
```

### Sound, lip-sync, interchange and publishing

```js
const id = ac.import.sound({ path: "voice.mp3" });                   // WAV / MP3 / OGG / FLAC
ac.timeline.set_sound({ sound: id, sync: "stream", layer: "Voice", frame: 1 });   // event | start | stop | stream
ac.timeline.set_sound({ sound: "music", repeat: 3 });                // or loop: true
ac.timeline.set_sound({ sound: id, effect: "fade in" });            // left channel | right channel | fade to right | fade to left | fade in | fade out
ac.sound.list();                                                     // [{ id, name, bytes, seconds }]
ac.select.element({ layer: "Mouth", index: 0 });                     // the mouth instance (0 = bottom)
ac.lipsync.apply({ audio_layer: "Voice", map: { "O": 5, "MBP": 2 } }); // shapes → symbol frames (1-based)
ac.frame.info({ layer: 1, frame: 1 });                               // span, label, tween, script, sound, objects
ac.symbol.new({ name: "Mouth", kind: "graphic" });                   // Ctrl+F8 (opens it for editing)
```

From the CLI, every document argument can be an `.acx.json`, `.fla`, `.xfl`
or XFL folder, and `--save` writes whichever extension you give:

```sh
flare-cli run build.js --doc old.fla --save new.fla
flare-cli html5 movie.fla site/            # or site.html for one file
flare-cli sprites movie.fla walk.png 0.5 --format starling
flare-cli lottie movie.fla movie.json
flare-cli svg-anim movie.fla movie.svg
flare-cli convert movie.acx.json movie.fla
flare-cli png movie.fla 1 poster.jpg 2 --quality 85   # .png / .jpg / .gif (--colors N, --transparent)
```

### JSFL scripts

`.jsfl` files (Script Console language "JSFL", or `flare-cli run
script.jsfl`) run through a compatibility layer over the same commands. Layers
and frames are 0-based, as in JSFL:

```js
var doc = fl.getDocumentDOM(), tl = doc.getTimeline();
tl.addNewLayer("Boxes");
doc.setFillColor("#33AAFF");
doc.addNewRectangle({ left: 10, top: 10, right: 90, bottom: 60 }, 6);
doc.selectAll();
doc.convertToSymbol("movie clip", "Box", "center");
doc.selection[0].name = "box";
tl.insertKeyframe(23);
tl.setFrameProperty("tweenType", "motion", 0, 1);
tl.layers[tl.currentLayer].frames[0].actionScript = "this.stop();";
fl.trace(doc.library.items.length + " library items");
```

Covered: `fl` (getDocumentDOM, trace, outputPanel), Document (size, frame rate,
colour, selection, add rectangle/oval/line/text/polystar/primitives, fill and
stroke, move/scale/rotate/skew, align/distribute/match, group, break apart,
arrange, convertToSymbol, swapElement, instance alpha/tint/brightness, blend,
filters, import, exportPNG/SVG, saveAsCopy, scenes), Timeline (layers, frames,
current frame/layer, insert/remove/convert/clear frames and keyframes, tweens,
reverse, frame and layer properties), Layer/Frame/Element objects with live
properties, copy/cut/paste frames, `FLfile` (exists, read, write, append,
createFolder, listFolder; no remove), and the Library (items, add to document,
rename, delete, edit, new symbol). JSFL asks before running, like external
languages, because it can write files. Anything else throws "JSFL: … is not supported by Flare yet".

### Any other language

Python, Node and Ruby use tiny helper libraries, but the protocol is simple
enough to use from anything that can print and read lines. The script writes
one line per command to stdout:

    \x1eAC {"id": 1, "cmd": "draw.rect", "args": {"x": 10, "y": 10, "w": 50, "h": 50}}

(`\x1e` is the ASCII record-separator character) and reads one JSON line back
from stdin: `{"id": 1, "result": …}` or `{"id": 1, "error": "…"}`. Anything
else the script prints goes to the Output panel. Add the interpreter for a
file extension in `ScriptOptions::interpreters` (Lua is preconfigured).

## Does supporting several languages cause problems?

Mostly no, because of how it is built:

- **One API.** Languages don't get their own bindings; they all call the same
  command registry (`ac_core::commands`). A new command is instantly available
  everywhere, and behaviour can't drift between languages.
- **No extra dependencies for the app.** Only JavaScript is embedded
  (QuickJS, ~1 MB). Python, Node and Ruby are *not* bundled: they're used if
  installed. If one is missing you get a clear "couldn't start python" message.
- **Isolation.** External scripts are separate processes with a timeout
  (30 s default), a line-size limit and a command limit; a crash or infinite
  loop in a script can't take the editor down.

The real costs to keep in mind:

- **Security.** An external script is a normal program with your
  permissions. That's why the app asks before running one, and why documents
  can never carry them. Treat scripts like any downloaded program.
- **Portability of shared scripts.** A Python script only runs for people who
  have Python. For scripts meant to be shared, built-in JavaScript is the
  safe choice.
- **Exported animations.** Frame scripts stay JavaScript only, because an
  HTML5/web export can run JavaScript natively; Python in a browser would need
  ~10 MB of Pyodide per movie.
