# What the old code had, and what Flare does with it

A review of `original/` (the archived C++ project; never a dependency, never copied from). Each
idea is either already in Flare, planned, or deliberately left out. Flash test fixtures were the
first thing taken over and are in the importer and Flash adapter tests.

| Idea in `original/` | Flare |
| ------------------- | ----- |
| Flash/XFL/FLA import, real-file fixtures | Done: `flare-import`, fixtures under its tests |
| SWF publish | Done: `flare-publish` |
| FLV/F4V/SWF detection | Done: `flare-flash` |
| `.sol` (shared object) container and AMF0 reading | Planned: belongs in `flare-flash`, with its own fixtures |
| Merge-drawing model (Flash planar-map shapes: edge bending, boolean cuts) | Planned and the largest gap against Animate's drawing model; today shapes are separate objects |
| AS3 decompile / ActionScript editing | Not planned for 0.1; ActionScript is preserved as data, never run |
| Moho projects, rig rendering | Not planned; import is a later milestone after the Animate workflow |
| FlipaClip layout profile, custom room layouts | Flare has dockable panels and "Reset layout"; named workspace presets are a dimmed menu entry |
| Auto-update with SHA-256 verification | Planned after the first release exists to update from |
| Windows installer (NSIS), Chocolatey | Planned; see [release.md](release.md) |
| Android build, P2P remote control, game ECS | Not planned |
| Issue-to-fix map and regression script | Replaced by tests that name the behaviour they protect |

## Questions people ask

- **`.fla` files:** modern ones (CS5 and later, which are zipped XFL) and XFL folders import and
  export. Older binary `.fla` files (CS4 and before) are recognised and refused with a reason; open
  them in Animate and save as a modern `.fla` or XFL. Reading the old binary format is not planned for 0.1.
- **Pen pressure:** the Brush's width follows pressure (the Pencil's does not), for tablets that report force through the
  window system. It is untested on real hardware.
- **Symbols:** Graphic, Movie Clip and Button exist as types (see [architecture.md](architecture.md)).
  Only the type is carried; a movie clip's own clock and a button's states are not run yet.
- **ActionScript:** not in 0.1. It is kept as data in imported files, never run or edited.
- **HTML5:** File > Publish writes an HTML5 Canvas page (one self-contained `.html` file) as well as a SWF, chosen in File > Publish Settings, where Animate has them: Loop timeline, Center stage and Make responsive. The page is baked like the SWF (tweens and symbols flattened, scenes played one after another) and has masks, gradients and strokes, with sounds embedded and started at their cues (the page asks for a click first, because browsers keep a page silent until then), but no scripting or interactivity, and it is not a CreateJS document. WebGL is not planned for 0.1.

## Gaps against Animate that remain

The outline column's color picker (the chip cycles a palette for now), classic tweens as their own kind (a motion tween on an instance is the nearest), shape hints, tweens of color effects, text menu
commands, command scripting, keyboard shortcut editor, browser sound and pen input, autosave for
every tab. These appear as dimmed entries in the menus until built ([ui.md](ui.md)).
