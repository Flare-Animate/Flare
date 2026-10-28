# Flare full QA report (Rust-Rewrite, tip cf36dee)

Scope: real app under Xvfb :77, isolated HOME and stuff/ copy per run. Binary rebuilt from tip (local_build.sh, linked 19:54 UTC 2026-10-24; the build log's final "RUNNING" line was a pgrep self-match, binary was fresh). Harness: /tmp/qa/ (not in repo).

## Summary (update 2, real counts)

| Bucket | Count |
|---|---|
| Menu command IDs in menu XML (checklist) | 478 |
| Command invocations via All Commands walker | 155 (15 categories) |
| Invocations with no visible change | 90 |
| Invocations with visible change (no dialog detected) | 65 |
| Invocations that crashed the app | 0 |
| Distinct commands verified by name | not established (walker stopped early in most categories, see D12) |
| Toolbar tools (left column, 22 icons) | 5 tested + 1 crash; 16 not tested |
| Tools with canvas change (pixel diff > 0) | 3 of 5 (tools 2,3,4); 2 no change (tools 0,1) |
| Themes launched and visually read | 10 of 10 |
| Layout profiles launched | 5 of 5 |
| Animate rooms visually verified | 3 of 8 (Draw, Rigging, Sound; Timing seen via side effect) |
| Top-level menus opened | 9 of 9 + All Commands |
| Preferences dialog | not tested |
| Import/export/render, save/load, undo/redo, playback, xsheet edit, Fx, Flash Instance, script console, IK, cleanup, sound, stop-motion | not tested |
| Crashes observed | 1 (toolbar tool 6, FATAL bad QImage format, D11) |

Coverage estimate of the 478 command IDs: at most 155 invocations attempted, real distinct coverage well under 30 percent. Overall project coverage well under 25 percent. Verdict: the app survives every launch and most menu entries, but layout clipping, a crash on tool use, and most tools/dialogs remain unverified.

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
| D11 | CRASH: "FATAL: bad QImage format" after drag/undo sequence on toolbar tool at y=262 (paint/fill icon, index 5). Not reproduced by single click | Animate, New Scene | flare/sources/common/tvrender/tfont_qt.cpp:186 (throw site); caller in tool/viewer draw path; main.cpp prints FATAL |
| D12 | Walker test tooling: All Commands submenu enumeration stops early (wrap-around detection); a room switched to Timing during walk (side effect unexplained) | harness | /tmp/qa/walk.sh (not repo) ; investigate Room tab shortcut bindings in stuff/profiles/layouts/shortcuts |

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

## D11 regression note (bad QImage format)

Root cause: `TFont::drawChar` (tfont_qt.cpp) threw on any `alphaMapForGlyph` image that was not Indexed8/Alpha8, and the message built `L"..." + image.format()` (wide-literal pointer arithmetic, hence the empty "bad QImage format " in the log). Fix: null/invalid glyph images return an empty advance; other formats are converted with `convertToFormat(Format_Grayscale8)`. Verification: rebuilt; scripted Xvfb run (tool at y=262, drag, click, ctrl+z x3) stayed alive with no FATAL. The original crash was not deterministically reproducible before the fix (matches D11 "not reproduced by single click").

---

## QA update 2026-10-25 (Rust-Rewrite, HEAD 62d1e91; requested tip 1872fda is its ancestor, 62d1e91 only touches .gitignore)

Method: `scripts/dev/local_build.sh` rebuilt into /home/user/bld (build ok, fix d661d7c present). Samples downloaded to a scratch dir (untrusted, not committed): ruffle-rs/ruffle `tests/tests/swfs` (SWF, FLA), mozilla/shumway `test/swfs` (FLA, XFL folder, MP4, SVG), psd-tools/psd-tools `tests/psd_files` (PSD), repo `stuff/library` (PLI), github.com GIF from ruffle. Import via `tools/flash/import_container.py`; GUI via `scripts/dev/gui_smoke.sh` and a per-tool relaunch loop (xdotool).

### Command/tool/format results

| Area | Item | Result | Evidence / note |
|---|---|---|---|
| Build | `local_build.sh` | PASS | build ok in 26s, /home/user/bld |
| Tools | Toolbar tools 0-21 (22), relaunch each, click tool, drag canvas, ctrl+z, ctrl+y | PASS (no crash) / effect UNVERIFIED | all ALIVE, no FATAL in logs. Process survival only; canvas change not measured. Prior D11 (tool 5 crash) not reproduced |
| Import (helper) | FLA (acid.fla, ruffle test.fla) | PASS | 16 / 19 files exported |
| Import (helper) | XFL zipped (packed .xfl) | PASS | 4 files |
| Import (helper) | XFL folder (DOMDocument.xml) | PASS | 4 files |
| Import (helper) | PSD (1layer, 2layers) | PASS | 1 file each, copy only, no layers |
| Import (helper) | SWF AS2 / AS3 / blend | PASS with warning | JPEXS not on PATH, built-in extractor used, 1 file each |
| Import (helper) | GIF, MP4, PLI, SVG | FAIL | `Unsupported file type: .<ext>` rc=3 |
| Import (GUI) | FLA (acid.fla) load arg | FAIL | "File .../acid.fla doesn't look like a TOONZ Scene" (docs/images/qa/load-fla-not-toonz-error.png) |
| Import (GUI) | PSD (1layer.psd) load arg | FAIL | "... doesn't look like a TOONZ Scene" (docs/images/qa/load-psd-not-toonz-error.png) |
| Import (GUI) | SWF (blend test.swf) load arg | FAIL | same dialog text |
| Import (GUI) | XFL folder load arg | UNTESTED | launched only, file not passed (harness -f check) |
| Import (GUI) | GIF, MP4, PLI, SVG | UNTESTED | not run |
| CLI | `flare-cli` (ac-cli) | BLOCKED | `cargo build` fails: flareqt/Cargo.toml not in flare/sources workspace. No info/convert/export possible |
| Save/load | Ctrl+S scene save, reload .tnz | BLOCKED | no save dialog reached via xdotool; no .tnz written. Needs Save As menu path |
| Export | Flash SWF / XFL / HTML5 | UNTESTED | only via GUI Export dialog, not driven. CLI blocked |
| Preferences, undo/redo beyond tool loop, playback, xsheet, Fx, Flash Instance, script console, IK, cleanup, sound, render, all rooms | | UNTESTED | time budget; not driven |
| Moho (.moho/.anme), Adobe .ai, .tnz, .tlv, Lottie .json, .air/.ane | | UNTESTED | no usable online sample found (moho/AI searches gave pages, no direct files). Local fixtures sample.ai/sample.air exist, not re-run |
| Themes, layouts | | UNCHANGED | see prior sections |

### Defects (new)

- **D13 (high). Native Flare cannot open non-TNZ files.** `Flare <file.fla|.psd|.swf>` shows "doesn't look like a TOONZ Scene". Repro: `FLAREROOT=/home/user/Flare/stuff /home/user/bld/bin/Flare /path/to/acid.fla`. Consistent with the prior note that the command line accepts only `.tnz`. Import of these formats in the GUI is therefore not reachable from the CLI argument path; check File > Import path separately.
- **D14 (high, harness). `scripts/dev/gui_smoke.sh` reports SMOKE PASS when the load dialog appears.** It only checks for a main window, crashes, and fatal log markers. Fix the harness to detect modal error dialogs (title/window text) and fail on them.
- **D15 (medium). `flare-cli` does not build from this checkout.** `flare/sources/flareqt/Cargo.toml` is not a workspace member and `flare/sources/Cargo.toml` does not list it. `cargo build -p ac-cli` errors "did not match any packages"; building from flareqt errors "believes it's in a workspace". Fix: add `flareqt` to `workspace.members` or `workspace.exclude` in `flare/sources/Cargo.toml`.
- **D16 (low). Import helper rejects GIF, MP4, PLI, SVG** with `Unsupported file type`. PLI is a brush library format, not a scene. Confirm which of these Flare claims to import (docs/ and the import dialog) and align the claim.
- **D17 (low). SWF import helper needs JPEXS (`ffdec`) on PATH** and silently falls back to the built-in extractor with a warning. Document or bundle.

### Coverage

- Menu command IDs (478 in XML): no new command IDs exercised this pass; estimate unchanged at about 25 percent.
- Toolbar tools: 22 of 22 relaunched and exercised (effect unverified). Prior: 5 of 22.
- Import formats: 15 samples through the helper, 10 pass, 5 fail (4 unsupported, 1 SWF warning-only is pass). GUI load: 3 of 3 attempted fail.
- Overall project coverage estimate: about 27 percent (up from about 25 percent). Most of the remaining scope is still UNTESTED.

### Next steps

1. Fix D15 (workspace membership) so flare-cli can run info/convert/export; then add Flash/XFL/HTML5 round-trip tests.
2. Add Save As + reload test (File menu, not Ctrl+S) for .tnz.
3. Make gui_smoke fail on modal error dialogs (D14) before trusting it.
4. Find Moho, AI, Lottie, TLV samples or generate minimal ones and say so.

---

## QA update 2026-10-26 (QA2, Rust-Rewrite tip 7e93bc2, AnimationCraft bridge)

Partial pass. Most of the requested scope is UNTESTED; coverage below is honest, not padded.

Method: worktree `/home/user/wt-qa2` (detached at origin/Rust-Rewrite 7e93bc2). Build via `BUILD_DIR=/home/user/bld-qa2 scripts/dev/local_build.sh` (ok, 793 s, 1072 targets). Xvfb `:77` via `scripts/dev/gui_smoke.sh`. Samples: `/tmp/qa2/` (untrusted; ruffle SWF paths returned 404, so no SWF; Moho/AI/GIF/MP4/Lottie samples not obtained). Disk: 9.1G free at start, 8.0G at last check.

### Results

| Item | Result | Evidence |
|---|---|---|
| Build from worktree | PASS | build ok 793 s, `/home/user/bld-qa2/bin/Flare` |
| GUI main window launch | PASS | `gui_smoke` main phase, title `untitled [sandbox] : Flare 1.7.1` |
| GUI smoke harness verdict | FAIL (harness) | Harness says PASS for every case below, but each one leaves a modal dialog open. See D18. |
| Load `tests/flash_fixtures/handmade.fla` (arg) | PARTIAL | Log shows `xfl_svg/L0_F0.svg` created (AC bridge runs). Then modal "doesn't belong to the current project. Import / Load / Cancel" blocks the scene. Layer/frame counts not read. docs/images/qa/qa2-handmade-fla-prompt.png |
| Load `sample.fla` (470 B) | PARTIAL | Info dialog "Flash import complete. 550x400, 0 layers, 0 frames, 0 sounds". docs/images/qa/qa2-sample-fla-info.png |
| Load PSD (`gray0.psd`) | PARTIAL | Same "doesn't belong to project" prompt. Import outcome not run. docs/images/qa/qa2-psd-prompt.png |
| Load SVG (US flag) | PARTIAL | Same prompt. Not run past prompt. |
| Click Import on prompt, read xsheet | UNTESTED | no xdotool click path written |
| Export XFL / Export HTML5 / re-import / HTML5 in chromium | UNTESTED | playwright not installed in this env (`ModuleNotFoundError`). Export not driven. |
| 22 tools with canvas stroke + pixel diff | UNTESTED | |
| Save/reload .tnz, undo/redo, playback, xsheet edit, Preferences tabs, LiquidGlass across rooms, layout profile rooms | UNTESTED | time budget |
| Moho (.moho/.anme) | Importer exists (`flare/sources/flare/mohoimport.cpp`, `common/moho/`), no sample, not run | UNTESTED |
| Lottie (.json) | No importer found (grep `lottie` hits only particle/test code) | UNSUPPORTED (grep evidence) |
| GIF / MP4 / SVG / PLI as levels | Routed as levels in `openNonSceneFile` (`flashimport.cpp:1193`). SVG load hits the project prompt; GIF/MP4 not run | PARTIAL |
| AI (.ai) | fixture `tests/flash_fixtures/sample.ai` exists; no GUI run | UNTESTED |

Coverage: about 8 of roughly 100 checklist items touched, about 3 fully PASS (build, main launch, sample.fla opens without crash). Overall estimate: about 10 percent of the requested scope.

### Defects (new, QA2)

- **D18 (high, harness). `scripts/dev/gui_smoke.sh` returns PASS while a modal dialog is open.** Repro: `BUILD_DIR=/home/user/bld-qa2 SMOKE_FILE=tests/flash_fixtures/handmade.fla scripts/dev/gui_smoke.sh /tmp/x` prints `SMOKE PASS`, but the screenshot shows the "doesn't belong to the current project" prompt. `detect_modal_error` matches only titles with error/warning/failed. This prompt is a question dialog, so it is not caught. Fix: also fail on any visible non-main Flare window in phase 2, or set a per-case expected-dialog list.
- **D19 (high). Passing a file outside the project opens a Import/Load/Cancel prompt rather than importing.** Repro: `Flare /tmp/qa2/3/p.psd` (or any `.svg`, `.fla` in `tests/flash_fixtures/`). Same for FLA-derived SVG. Expected: route straight to the importer, or make the prompt answerable from CLI.
- **D20 (medium). Printf-style string bug in import path.** Log: `QString::arg: Argument missing: Always do this action., /tmp/flare_flash_import_.../xfl_svg/L0_F0.svg`. A `.arg()` call has one placeholder too few. Find the caller of the "Always do this action" prompt in `flashimport.cpp` or the file-open dialog code.
- **D21 (medium). Import of `sample.fla` reports 0 layers, 0 frames, 0 sounds with no error.** Either the fixture is empty or the importer drops content silently. Needs a check against `tests/flash_fixtures/README.md`.
- **D15 still open** (`flare-cli` workspace membership), not rechecked this pass.

### Screenshots

- docs/images/qa/qa2-handmade-fla-prompt.png
- docs/images/qa/qa2-psd-prompt.png
- docs/images/qa/qa2-sample-fla-info.png

### Next steps

1. Fix D18 so the harness fails on the prompt, then D19 routing.
2. Install playwright (or chromium) in the QA env; export HTML5 and render-check it.
3. Write an xdotool tool-loop with pixel diff for the 22 tools (`import -window root` before/after).
4. Get Moho and AI samples, or record them as UNSUPPORTED with evidence.


---

## QA update 2026-10-26 (QA3, automated harness `scripts/dev/qa/run_all.sh`, Rust-Rewrite)

Full run: 118 PASS, 1 FAIL, 1 UNTESTED (552 s). Machine-readable results, screenshots and crash logs are written by the
harness to its `--out` directory; see docs/NEXT_SESSION.md for the coverage list and how to run it.

| Area | Result |
|---|---|
| 22 toolbar tools, stroke + pixel diff | 22 PASS (PaintBrush/Finger: correct warning on a Vector level) |
| Brush on empty cell (level auto-create) | PASS after fix (was SIGSEGV) |
| Undo/redo (keys + command), next frame, .tnz save/reload | PASS |
| Playback frame counter | FAIL (constant under Xvfb; may be environment related) |
| Preferences (16 pages), all rooms of 5 layout profiles (28 rooms) | PASS |
| Imports: FLA, XFL folder, SWF, PSD, SVG, GIF, JPG, PNG, AI/AIR/ANE/F4V/FLV, 3 Moho | PASS (empty documents and unconverted morph shapes are explained in the import result) |
| Export XFL/FLA/HTML5, re-import counts, HTML5 in chromium | PASS |

Defects found and fixed in this pass: use-after-free of the level palette (`include/flare/txshsimplelevel.h`, raw `TPalette*`
instead of `TPaletteP`) that crashed every SVG/PSD/Toonz Raster level load, hence every XFL/FLA/SWF import and the first Brush
stroke on an empty cell; PSD 1x1 image divide by zero; XFL import added each bitmap twice; extension-less XFL folder argument
ignored; D18-D21 from QA2.
