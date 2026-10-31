# Changelog

All notable changes are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project follows
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added
- SWF publishing writes linear gradients as gradient fills (matrix placing the first color at the start and the last at the end) and masks of one shape as clip layers; masks of several shapes and masks inside masks are reported and left out.
- SWF publishing writes sounds: each cued sound is defined once as uncompressed 16-bit audio at the nearest SWF sample rate at or above its own, and started at every cue, with scenes offsetting the frames. The total is bounded.

## [0.1.0] - not yet released

The first release of the Rust rewrite. An early alpha: see docs/releases/v0.1.0.md for what it does not do yet.

### Added
- A document model with scenes, layers (including mask, folder and motion guide layers), frame
  spans, shapes with strokes and gradients, symbols, and sounds. Every change is a validated
  command batch and one undo step.
- Drawing tools: selection, subselection, free transform, lasso, pen, text, line, rectangle,
  ellipse, polystar, pencil, brush, paint bucket, ink bottle, eyedropper, eraser, hand, zoom,
  ruler (measures distance and angle) and mirror (sets the axes for symmetric drawing).
- Shape tweens (Insert > Create Shape Tween) turn one shape into the next, and both tween kinds have Animate's Ease (-100 to 100) in the Properties panel. Tweened spans show an arrow and a color of their own in the timeline. XFL's `tweenType="shape"` and `acceleration` are imported and exported, and `flare-cli frame` has `motion-tween`, `shape-tween`, `remove-tween` and `set-ease`.
- A light theme, chosen in Preferences > General beside Dark, which stays the default.
- The interface can be shown in English, Chinese (Simplified), German, Japanese, Korean, Portuguese (Brazil) or Spanish, chosen in Preferences > General (System locale by default); Preferences and Publish are translated so far, with the rest to follow.
- Publish opens a popup with the formats (Flash, HTML5, animated GIF, and MP4 video with its sound, written by Flare itself in pure Rust) and a Twitter button that opens the post already filled in, and YouTube, Newgrounds and Facebook shown dimmed until they can be filled in too.
- File > Publish writes an HTML5 Canvas page and a Flash movie, as chosen in File > Publish Settings (Loop timeline, Center stage, Make responsive), with sounds started at their cues; `flare-cli publish-html5` does the same from the command line.
- Symbols have a type (Graphic, Movie Clip or Button), chosen when making one or changed in the Library, saved in the project and exchanged through XFL. A Button shows its Up state in the stage and in every export, instead of looping through its frames, and a Graphic instance has Animate's Looping options (Loop, Play Once, Single Frame, and the first frame) in the Object tab, kept in the project and in XFL.
- A Home page at launch: new-project sizes, Open, Import and recent projects, each with when the file was last changed and its size, and a search box.
- The Brush has a nib (Round or Square, with an angle and a flatness) that shapes the stroke; the
  nib and size are kept between sessions. New, choose and Delete keep named custom brushes, and the Brush shows its nib as an outline at the pointer. A brush stroke drawn with smoothing at 0 no longer crosses itself in slivers, so it no longer jumps when the pointer is released, and a Pencil or Brush stroke no longer blinks out for a frame at release.
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

[Unreleased]: https://github.com/Flare-Animate/Flare/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/Flare-Animate/Flare/releases/tag/v0.1.0
