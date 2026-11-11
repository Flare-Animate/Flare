# What the old code had, and what Flare does with it

A review of `original/` (the archived C++ project; never a dependency, never copied from). Each
idea is either already in Flare, planned, or deliberately left out. Flash test fixtures were the
first thing taken over and are in the importer and Flash adapter tests.

| Idea in `original/` | Flare |
| ------------------- | ----- |
| Flash/XFL/FLA import, real-file fixtures | Done: `flare-import`, fixtures under its tests |
| SWF publish | Done: `flare-publish`; sounds are written as uncompressed 16-bit audio at their cues, linear gradients as gradient fills, and masks of one shape as clip layers (masks of several shapes or inside masks are reported and left out) |
| FLV/F4V/SWF detection | Done: `flare-flash` |
| `.sol` (shared object) container and AMF0 reading | Planned: belongs in `flare-flash`, with its own fixtures |
| Everything drawn on a layer stays on that layer (many elements in a keyframe) | Done: new shapes join the active layer's keyframe at the playhead, selection and every edit name one element, the exporters and `flare draw-rect` / `draw_rect` follow (docs/decisions/ELEMENTS.md) |
| Merge-drawing model (Flash planar-map shapes: edge bending, boolean cuts) | Planned and the largest gap against Animate's drawing model; today every shape is a separate drawing object (Object Drawing), shown as a dimmed switch in the tool options |
| AS3 decompile / ActionScript editing | Not planned for 0.1; ActionScript is preserved as data, never run |
| Moho projects, rig rendering | Not planned; import is a later milestone after the Animate workflow |
| FlipaClip layout profile, custom room layouts | Flare has dockable panels and "Reset layout"; switching between whole interfaces (Animate's workspace switcher) is a dimmed button at the top right and a dimmed Window > Interfaces entry |
| Auto-update with SHA-256 verification | Planned after the first release exists to update from |
| Windows installer (NSIS), Chocolatey | Planned; see [RELEASE.md](RELEASE.md) |
| Android build, P2P remote control | Not planned |
| Game ECS and game creation | Reversed: planned and in progress as `flare-game`; see [GAME.md](GAME.md). The C++ scaffold's systems were mostly empty, so the port defines their small behaviour and lists what is still open. Scripts stay unrun |
| Adobe/Harman AIR and Flex SDK targets (a Flash-era app toolchain) | Not planned: Flare publishes SWF and HTML5, and ActionScript is not run |
| OpenToonz's raster painting, FX schematic and xsheet | Not planned: they belong to a different kind of tool than Animate |
| Trace Bitmap (threshold, minimum area, curve fit, corners; a centerline mode) | Planned, high priority (drawing quality) |
| Motion Editor: per-property graphs, key drag and retime, per-property eases | Planned, high priority |
| Import SVG and Photoshop (a bitmap per layer, visibility, opacity); Illustrator later | Planned: SVG and PSD high priority, `.ai` lower |
| Find and Replace, shared libraries and the Assets panel, History replay and Save as Command | Planned, lower priority |
| Layer depth (2.5D with the camera) | Planned, lower priority, with the camera follow-ups |
| Legacy binary `.fla` (CS4 and before) | Partly built: the stage, frame rate, background and library names and types are read into a skeleton project with a warning; drawings, timelines, text and media are not (checked only on Ruffle's small MX 2004/8-era test files) |
| Contrast check for themes (`scripts/dev/contrast_check.py`) | Planned with custom themes |
| Pre-commit hook that kept the atlas of the code honest | Planned as a smaller hook: format check only |
| `cairn` (a log of sessions and lessons) | Not needed: `.codesight/` maps the code ([ENGINEERING.md](ENGINEERING.md)) and the git log is the history |
| Fixture generators and sample `.psd`, `.ai`, `.air` files | Ideas only; Flare writes its own fixtures when a feature needs them |
| Issue-to-fix map and regression script | Replaced by tests that name the behaviour they protect |

## Questions people ask

- **`.fla` files:** modern ones (CS5 and later, which are zipped XFL) and XFL folders import and
  export. Older binary `.fla` files (CS4 and before) are recognised and refused with a reason; open
  them in Animate and save as a modern `.fla` or XFL. Reading the old binary format is not planned for 0.1.
- **Saving in Animate's formats:** Save As offers `.fla` (the default), `.xfl` and `.flare`, and Save writes a document
  back to the `.fla` or `.xfl` it was opened from, as Animate does, with no `.flare` made. Not Animate's: no "Save as Copy", no
  compatibility choice of older Animate versions, and a pre-CS5 `.fla` cannot be written.
- **Opening and saving a file without changing it:** an imported `.fla`, XFL folder or `.swf` is kept whole with the
  project, so saving or exporting it while unchanged writes the original back byte for byte, and an edited export keeps every
  file the edit did not touch (bitmaps, sounds, settings). Opening never modifies the source. A legacy binary `.fla`
  is only a skeleton and keeps no source. See [decisions/LOSSLESS.md](decisions/LOSSLESS.md).
- **Pen pressure:** the Brush's width follows pressure (the Pencil's does not), for tablets that report force through the
  window system (Windows does). On macOS winit drops a tablet's pressure, so `flare-tablet` reads it from AppKit: a local event monitor
  sees the program's pointer events before the window does, takes the pressure of those a tablet made (the tablet's own events, and mouse events with a tablet subtype; a mouse or a Force Touch trackpad is not a pen), and passes every event on unchanged. On Linux winit passes a pen on as a mouse, so `flare-tablet` reads the pen from the X server
  (XInput2 raw events on the root window, which still arrive while winit holds the pointer during a drag; the "Abs Pressure" axis) and the window opens through X11 (XWayland under Wayland) when such a pen is
  plugged in; `FLARE_TABLET=off` keeps the native window system, and on macOS leaves the pen a mouse. Not built: Wayland's own tablet protocol, tilt, and
  the eraser end of a pen. It is untested on real hardware.
- **Brush nib:** the Brush paints with a Round or Square nib set in Properties to an angle (0 to 359 degrees) and a flatness (1 to 100 percent), as in Animate's Nib Options. Size, nib and angle are remembered between sessions. Custom brushes: **New** keeps the nib in hand under a name (up to 32), choosing one puts its nib in hand, changing the nib while one is chosen edits it, **Delete** removes it, and they are kept with the preferences. "Zoom size with stage" is always on: the brush is in stage units, so it scales with the zoom, and there is no switch to turn it off. Not built yet: renaming brushes and the Nib Options dialog. The Brush shows its nib as an outline at the pointer. Sync settings with Eraser is not built: the Eraser has a size of its own and a round tip only. The live preview while dragging is the finished stroke's own outline, ends and nib shape included.
- **Eraser:** as Animate's Erase Normal, dragging rubs out the fills and lines it passes over while it moves, on every shown, unlocked layer at the current frame, as one undo step; a click rubs out a dot. Its size is set in Properties (1 to 400 px) and its round outline follows the pointer. **Faucet** turns a click into deleting the whole shape under it. A filled, outlined shape comes apart into its fill and its line, and the cut edge of a fill gets no outline, as in Animate. Symbols, bitmaps and text are not erased (text is broken apart first in Animate too); curves are cut as fine straight pieces. Not built: the Erase Fills, Lines, Selected Fills and Inside modes, square eraser shapes, and the eraser end of a pen.
- **Symbols:** Graphic, Movie Clip and Button exist as types (see [ARCHITECTURE.md](ARCHITECTURE.md)).
  Only the type is carried; a movie clip's own clock and a button's states are not run yet.
- **ActionScript:** not in 0.1. It is kept as data in imported files, never run or edited.
- **HTML5:** File > Publish writes an HTML5 Canvas page (one self-contained `.html` file) as well as a SWF, chosen in File > Publish Settings, where Animate has them: Loop timeline, Center stage and Make responsive. The page is baked like the SWF (tweens and symbols flattened, scenes played one after another) and has masks, gradients and strokes, with sounds embedded and started at their cues (the page asks for a click first, because browsers keep a page silent until then), but no scripting or interactivity, and it is not a CreateJS document. WebGL is not planned for 0.1.

## Gaps against Animate that remain

The outline column's color picker (the chip cycles a palette for now), classic tweens as their own kind (a motion tween on an instance is the nearest), shape hints, tweens of color effects, text menu
commands, command scripting, keyboard shortcut editor, browser sound and pen input, autosave for
every tab. These appear as dimmed entries in the menus until built ([UI.md](UI.md)).

## Lessons from the old project's log

- Check `git status --ignored` before a push: an ignore pattern (`build*`) once hid build scripts
  that were needed, and CI failed with exit 127.
- A push cancels CI runs in progress, so commits are batched into one push per cycle.
- A change is read by someone other than its author before it is pushed.
