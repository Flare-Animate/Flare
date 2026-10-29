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

## Gaps against Animate that remain

The outline column's color picker (the chip cycles a palette for now), shape tweens, classic tweens with easing, text menu
commands, command scripting, keyboard shortcut editor, browser sound and pen input, autosave for
every tab. These appear as dimmed entries in the menus until built ([ui.md](ui.md)).
