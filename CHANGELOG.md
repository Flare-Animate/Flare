# Changelog

All notable changes are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project follows
[Semantic Versioning](https://semver.org/).

## [0.1.0] - not yet released

The first release of the Rust rewrite. An early alpha: see docs/releases/v0.1.0.md for what it does not do yet.

### Added
- Test Movie plays in a window of its own, with Ruffle running in the same process on the editor's own GPU device, with sound. The picture fills the window, which opens at the movie's own size when the screen has room; under it a bar has a clock and frame at the left, one centered box (Restart, Pause or Play, Step one frame, a playback speed drop-down of 0.25x to 2x, a loop switch that holds the last frame when off, a sound switch) and Close at the right. Control > Test Movie and a right click on the Play button at the top open it. Building without the `ruffle` feature starts an external Ruffle instead.
- With the Subselection tool, a click on a point of the selected shape chooses it (it is drawn filled), and Delete or Backspace takes it out of the outline (one undo step) instead of deleting the shape.
- Timeline: a right-click menu on frames (insert frame, keyframe or blank keyframe, clear keyframe, remove frames, create a motion or shape tween, cut, copy, paste, select all, reverse), Edit > Timeline > Cut Frames and Select All Frames (Ctrl+Alt+X, Ctrl+Alt+A), and Modify > Timeline > Reverse Frames: the keyframes of the selected frames in the opposite order, a tween going with the pair it joined and its easing reversed. `flare frame remove-frames` and `reverse-frames` (with `--to`) and the MCP operations `remove_frames` and `reverse_frames` do the same from scripts.
- The Add (`=`), Delete (`-`) and Convert (`C`) Anchor Point tools, in the Pen's cell: click an outline to put a point there (a curve keeps its shape), click a point to take it out (the outline joins across the gap), click a curve point to make it a corner, drag from a corner to pull out smooth handles. Each is one undo step.
- The status bar shows the stage size and frame rate; a click on them opens the document settings.
- The scene can be renamed by double-clicking its name in the bar above the stage (Enter or leaving the box keeps it, Escape drops it), with `flare rename-scene` and the MCP tool `rename_scene`; the name is trimmed, 1 to 256 characters, and one undo step.
- `flare open [projects...]` starts the desktop app on the given files. The app's program is `flare-desktop` on Linux (`/usr/bin/flare-desktop`) and Windows, so it does not share the name `flare` with the command line on `PATH`.
- A scene can have a camera layer whose keyframes (position, zoom, rotation) and motion tweens change the picture in every render and export; the SWF and HTML5 pages show it too (real Ruffle draws the same picture); `flare camera` and the MCP tool `set_camera` point it; the Camera tool, its Properties and a preview are in the editor; XFL support is still to come (see docs/ROADMAP.md).
- SWF publishing keeps symbols: each becomes a sprite defined once, each instance a placement of it, with the clock its playback says (a looping Graphic and a Movie Clip loop, a Graphic that plays once ends on `stop()`, a held frame is one frame, a Button shows its Up state, and a new span starts the symbol again). Real Ruffle plays the result on the clocks Flare predicts. Tweens are still baked.
- French is an interface language (Preferences > General > Language, and followed from a French system), and the README is translated into every interface language, with a test that keeps the two lists together.
- `flare export --format gif`, and MCP tools `export_gif`, `publish_swf`, `publish_html5`, `export_xfl` and `import_project`: they write into the export directory only, never overwrite, and report what the format could not hold. `flare export --format mp4` and the MCP tool `export_mp4` write the video too, so the command line and MCP server have every export the desktop app has.
- The Polygon lasso (in the Lasso cell): click the corners of a loop, then double-click, press Enter or click the first corner to select what it encloses; Shift adds to the selection and Escape drops the loop.
- `flare-ruffle`'s `embedded` feature runs Ruffle inside the process on a wgpu device it is given: a published movie plays over time into a texture, with sound through Ruffle's mixer (tested: frames advance with the time given, pausing holds, the sound we publish is heard).
- Test Movie works in the browser: the page runs Ruffle's WebAssembly build (`@ruffle-rs/ruffle` 0.7.1, vendored with its licences, fetched on first use) in an overlay over the editor.
- `scripts/build_ruffle.sh` builds Ruffle's desktop player from the pinned checkout, and Test Movie has been run against it (an ignored test opens a published movie in it and checks it keeps playing).
- Ruffle is pinned as a submodule at third_party/ruffle, and `flare-ruffle`'s `render-check` feature has Ruffle's exporter render published SWFs and compares them with Flare's render (solid shapes, a gradient and a mask agree).
- SWF publishing writes linear gradients as gradient fills (matrix placing the first color at the start and the last at the end) and masks of one shape as clip layers; masks of several shapes and masks inside masks are reported and left out.
- SWF publishing writes sounds: each cued sound is defined once as uncompressed 16-bit audio at the nearest SWF sample rate at or above its own, and started at every cue, with scenes offsetting the frames. The total is bounded.
- A document model with scenes, layers (including mask, folder and motion guide layers), frame
  spans, shapes with strokes and gradients, symbols, and sounds. Every change is a validated
  command batch and one undo step.
- Drawing tools: selection, subselection, free transform, lasso, pen, text, line, rectangle,
  ellipse, polystar, pencil, brush, paint bucket, ink bottle, eyedropper, eraser, hand, zoom,
  ruler (measures distance and angle) and mirror (sets the axes for symmetric drawing).
- Shape tweens (Insert > Create Shape Tween) turn one shape into the next, and both tween kinds have Animate's Ease (-100 to 100) in the Properties panel. Tweened spans show an arrow and a color of their own in the timeline. XFL's `tweenType="shape"` and `acceleration` are imported and exported, and `flare frame` has `motion-tween`, `shape-tween`, `remove-tween` and `set-ease`.
- A light theme, chosen in Preferences > General beside Dark, which stays the default.
- The interface can be shown in English, Chinese (Simplified), German, Japanese, Korean, Portuguese (Brazil) or Spanish, chosen in Preferences > General (System locale by default); Preferences and Publish are translated so far, with the rest to follow.
- Publish opens a popup with the formats (Flash, HTML5, animated GIF, and MP4 video with its sound, written by Flare itself in pure Rust) and a Twitter button that opens the post already filled in, and YouTube, Newgrounds and Facebook shown dimmed until they can be filled in too.
- File > Publish writes an HTML5 Canvas page and a Flash movie, as chosen in File > Publish Settings (Loop timeline, Center stage, Make responsive), with sounds started at their cues; `flare publish-html5` does the same from the command line.
- Symbols have a type (Graphic, Movie Clip or Button), chosen when making one or changed in the Library, saved in the project and exchanged through XFL. A Button shows its Up state in the stage and in every export, instead of looping through its frames, and a Graphic instance has Animate's Looping options (Loop, Play Once, Single Frame, and the first frame) in the Object tab, kept in the project and in XFL.
- A Home page at launch: new-project sizes, Open, Import and recent projects, each with when the file was last changed and its size, and a search box.
- The Brush has a nib (Round or Square, with an angle and a flatness) that shapes the stroke; the
  nib and size are kept between sessions. New, choose and Delete keep named custom brushes, and the Brush shows its nib as an outline at the pointer. A brush stroke drawn with smoothing at 0 does not cross itself in slivers, so it does not jump when the pointer is released. Strokes, lines, rectangles and ovals stay on screen from the moment the pointer is let go until the picture that includes them arrives, so nothing flashes.
- Pencil and Brush can draw a straight line (option, or hold Shift) and mirror each stroke across
  one or two axes; Modify > Transform > Mirror Copy makes a reflected copy of the selection.
- Timeline (Control > Go to End, and the comma and period step a frame as in Animate) with frames, keyframes, layers, folders, onion skin, linear motion tweens and motion
  guides, and playback with sound.
- Panels that dock and move: tools, properties, color, align, transform, library, history, output. The tool strip groups related tools in slots as Animate does (press and hold, or right-click, for the list; Subselection, Oval, PolyStar and Ink Bottle are in their slots' lists), with the tools Flare has not built shown dimmed. It scrolls when it is taller than its panel, so the color box can always be reached, and the black-and-white and swap buttons stay where they were at the corners of the color squares, with No color in each color picker so a shape can have no stroke. The strip is one tool wide by default, runs the full height of the window and is a bar of its own. The stage shows its zoom once, in the stage bar. A dimmed Interface switcher sits at the top right, where Animate has its workspace switcher. A Publish button sits beside Play in the top bar. Tools can be dragged from the dots list into the strip and are kept. A layer is renamed by double clicking its name in the timeline, and a right click on it opens a menu of what can be done to the layer. Undo and redo bring back pictures already drawn without drawing them again.
- Several documents in a window as tabs, Open Recent, Save All, autosave and crash recovery,
  Preferences.
- Files: the `.flare` project container (a SQLite file whose records are compressed binary: about a seventh the size of the text it replaces), XFL/FLA import and export, PNG and GIF export (GIF frames are made on several cores), and SWF
  publish. Flash fixtures are tested against.
- The `flare-cli` command line (inspect, render, convert) and an MCP server for agents.
- Text menu: bold, italic, larger and smaller for new text.
- An Info panel (selection size and position, color under the pointer).
- Guides pulled out of the rulers, with snapping; Modify > Timeline > Convert to Keyframes.
- Layer outline colors (timeline chip, `.flare` and XFL) and View > Outline Mode.
- Window, Help (About, Keyboard Shortcuts) menus that do something; Insert > New Symbol; Edit > Paste in Place (Paste now centers on the stage).
- An experimental web build, which opens and saves `.flare` files (SQLite and zstd as WebAssembly) as well as importing Animate files.
- File > Export > Flash Document writes a `.fla` Animate can open.

[0.1.0]: https://github.com/Flare-Animate/Flare/releases/tag/v0.1.0
