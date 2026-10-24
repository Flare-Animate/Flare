# Flare full QA report (Rust-Rewrite, tip cf36dee)

Scope: real app under Xvfb :77, isolated HOME and stuff/ copy per run. Binary rebuilt from tip (local_build.sh, linked 19:54 UTC 2026-10-24; the build log's final "RUNNING" line was a pgrep self-match, binary was fresh). Harness: /tmp/qa/ (not in repo).

## Summary (honest)

| Bucket | Count |
|---|---|
| Menu command IDs in menu XML (checklist) | 478 |
| Command IDs individually triggered | 0 (not done, see Not tested) |
| Themes launched and screenshotted | 10 of 10 (all alive) |
| Themes visually read | 5 (AnimateDark, Blue, Light, LiquidGlass, Synthwave) |
| Layout profiles launched | 5 of 5 (Animate, AnimateLegacy, OpenToonz, FlipaClip, StudioGhibli) |
| Room tabs clicked and verified | 3 (Rigging, Sound, Draw/Animate main); others not verified |
| Top-level menus opened | 9 of 9 + All Commands (screenshots taken, only All Commands read) |
| Crashes | 0 observed in ~25 app launches |
| Broken layout / clipped text defects | 6 |
| Tools drawn on canvas | 0 (not tested) |

Verdict: app launches and survives every launch, theme and profile tested. Layout and text-clipping defects found in several themes and profiles. Command-level coverage is NOT done; this report does not claim it.

## Themes (Preferences > Interface, via CurrentStyleSheetName)

| Theme | Launch | Result | Evidence |
|---|---|---|---|
| AnimateDark | OK | Chrome dark, canvas white. "My Favorites" overlaps "Texture" tab (defect D1) | /home/user/Flare/docs/images/qa/theme-AnimateDark.jpg |
| Blue | OK | Visually near-identical to AnimateDark; theme difference hard to see | /tmp/qa/shots/theme-Blue.png |
| Clay | OK | Launched, not read | /tmp/qa/shots/theme-Clay.png |
| Dark | OK | Launched, not read | /tmp/qa/shots/theme-Dark.png |
| Default | OK | Launched, not read | /tmp/qa/shots/theme-Default.png |
| Default-Green | OK | Launched, not read | /tmp/qa/shots/theme-Default-Green.png |
| Light | OK | Readable; top toolbar icons low contrast; D1 overlap present | /home/user/Flare/docs/images/qa/theme-Light.jpg |
| LiquidGlass | OK | Clipped text: "Reset Positior", "Texture" tab, startup bar labels cut (D2, D3) | /home/user/Flare/docs/images/qa/theme-LiquidGlass.jpg |
| Neutral | OK | Launched, not read | /tmp/qa/shots/theme-Neutral.png |
| Synthwave | OK | Readable, close to Dark; D1 overlap present | /tmp/qa/shots/theme-Synthwave.png |

## Layout profiles

| Profile | Result | Evidence |
|---|---|---|
| Animate (8 rooms) | Room tabs Draw, Animate, Timing, Rigging, Compose, Cleanup, Sound, Library present. Rigging = Fx Schematic with XSheet and Output nodes, renders OK. Sound = File Browser, renders OK. Compose verify failed (startup popup still modal; see T1). | /tmp/qa/shots/deep-room-Rigging.png, /tmp/qa/shots/deep-after-windows-item.png |
| AnimateLegacy | Launches. Rooms Drawing, Animation, Rigging, Compositing. Viewer header "Zoom" text clipped by toolbar icons; timeline behind startup popup has stale paint (D5). | /home/user/Flare/docs/images/qa/layout-AnimateLegacy-main.jpg |
| OpenToonz (9 rooms) | Launches. Rooms Drawing, Timeline, Cleanup, Palette, InknPaint, Composite, Xsheet, Tasks, Browser. Layout clean, xsheet and level strip render. Rooms not clicked. | /home/user/Flare/docs/images/qa/layout-OpenToonz-main.jpg |
| FlipaClip | Single room tab. Panels collapsed and overlapping: viewer about 200 px, Level Strip floats over viewer, empty bottom-right area (D4). | /home/user/Flare/docs/images/qa/layout-FlipaClip-main.jpg |
| StudioGhibli (6 rooms) | Launches to Cleanup room. Left column labels clipped ("ch", "els", "PI", "ocessing", "ntialias", "harpness", "speckling") (D6). Other rooms not clicked. | /home/user/Flare/docs/images/qa/layout-StudioGhibli-main.jpg |

## Menus

| Menu | Result |
|---|---|
| File, Edit, Draw, Timeline, Scene, View, Tools, Windows, Help | Opened without crash. Screenshots at /tmp/qa/shots/menu-<Name>.png. Items not read individually. |
| All Commands | OK. Category submenus present: Windows and Panels, Guided Drawing and Tweening, Tool Modes, Stop Motion/Scan/Cleanup, Scene/Rigging/Sound, Columns, Keyframes, Frames and Timing, Files and Project, Preview and Render, Adjust and Convert, Palettes and Styles, Levels, Edit and Selection, View and Display. |
| All Commands > Windows and Panels | Shows Cleanup Settings, Palette Gizmo. Activating with Return did not visibly open a panel. Unverified (keyboard harness limit, not confirmed defect). |

## Defect list (ranked)

| # | Defect | Where seen | Likely source |
|---|---|---|---|
| D1 | Style editor: "My Favorites" checkbox text overlaps "Texture" tab | all themes, Animate and OpenToonz | flare/sources/flareqt/styleeditor.cpp (label at ~6178; tab/header layout) |
| D2 | "Reset Position" button text clipped ("Reset Positior") | LiquidGlass | stuff/config/qss/LiquidGlass/LiquidGlass.qss (button padding/min width); Reset Position button in flare/sources/flare/mainwindow.cpp or flare/sources/tnztools/tooloptions.cpp (grep hit) |
| D3 | Startup dialog bottom bar labels clipped ("Automatically Save Every", "Show this at startup") | LiquidGlass | stuff/config/qss/LiquidGlass/LiquidGlass.qss; flare/sources/flare/startuppopup.cpp:132 |
| D4 | FlipaClip room panels overlap and viewer collapses | FlipaClip | stuff/profiles/layouts/rooms/FlipaClip/room1.ini (panel geometry); docking in flare/sources/flareqt |
| D5 | Viewer header "Zoom" label overlapped by toolbar icons; stale paint under startup popup | AnimateLegacy | viewer title bar in flare/sources/flareqt (viewer header); repaint after modal dialog |
| D6 | Cleanup Settings labels clipped at left edge | StudioGhibli room 1 | stuff/profiles/layouts/rooms/StudioGhibli/room1.ini; cleanup settings widget in flare/sources/flareqt |
| D7 | Blue and Dark/AnimateDark look near-identical; theme differences weak | Blue, AnimateDark | stuff/config/qss/Blue, stuff/config/qss/AnimateDark |
| D8 | Top toolbar icons low contrast on Light theme | Light | stuff/config/qss/Light/Light.qss (icon tint) |
| D9 | Room tabs ignore clicks while startup popup is open | Animate | startup dialog modality (flare/sources/flareqt); needs Escape first. Confirmed by Compose tab not switching in first run, switching after Escape in second run |
| D10 | Windows and Panels submenu item activation unverified | All Commands | flare/sources/flare/menubarcommandids.h; command handlers unverified |

Note: D9 is intended modal behaviour per design, listed for awareness only.

## WIP and stubs (from docs/STATUS.md and docs/KNOWN_ISSUES.md, not runtime-verified)

- flashimport.cpp: vector shape import, shape text, static/dynamic text, morph shapes, sounds, video, components, SWF vector art, legacy binary FLA timeline not implemented (STATUS.md). Not exercised: no FLA import run.
- castviewer.cpp: frame count not shown for vector-only FLA (known issue).
- tools/flash/ext2flash compile: mxmlc missing, stub.
- KNOWN_ISSUES #47: Flash import is WIP.

## Not tested (reasons)

- Individual command IDs (478): time budget. Menu XML checklist at /tmp/qa/checklist.md; no per-command trigger run.
- Tools (toolbar, canvas drawing with xdotool drag and pixel compare): not done.
- Preferences dialog tabs, Shortcuts popup, Command Bar editor: not done.
- Import (FLA/XFL/SWF/PSD), Export and Render dialogs, Save/Load round-trip, undo/redo, playback, xsheet cell edits, Fx add/remove, Flash Instance panel, Moho/Next2Flash/AS3, JSFL console, Rigging IK tools, cleanup, sound: not done.
- Stop-motion and camera capture: hardware absent, not testable headless.
- Remaining room tabs (Timing, Compose, Cleanup, Library in Animate; most OpenToonz and StudioGhibli rooms): not verified.
- Remaining theme screenshots (Clay, Dark, Default, Default-Green, Neutral): launched, not read.

## Screenshots (repo copies, <=100 KB, under docs/images/qa/)

- docs/images/qa/theme-LiquidGlass.jpg
- docs/images/qa/theme-Light.jpg
- docs/images/qa/theme-AnimateDark.jpg
- docs/images/qa/layout-FlipaClip-main.jpg
- docs/images/qa/layout-StudioGhibli-main.jpg
- docs/images/qa/layout-AnimateLegacy-main.jpg
- docs/images/qa/layout-OpenToonz-main.jpg
- docs/images/qa/deep-room-Rigging.jpg
- docs/images/qa/menu-AllCommands-open.jpg
- docs/images/qa/deep-after-windows-item.jpg

## Recommended next steps

1. Full per-command pass with a driver that records dialog/no-op/error per MI_ id (checklist at /tmp/qa/checklist.md).
2. Tool-by-tool canvas draw test with pixel compare.
3. Fix D1-D6 (layout and clipping) in QSS and panel sizing, then re-run theme and profile sweep.
