# Open work for later sessions

Written 2026-10-24 so nothing is lost between sessions. Rules for agents: every sub-agent prompt starts with
"First invoke Skill anthropic-skills:caveman and anthropic-skills:strict"; cheapest viable model; independent
reviewer reads from git (never the working tree) before a push; push once per round after local build + CI.

## Needs the owner
- PR #151 (`Rust-Rewrite` -> `master`) has conflicts with `master`; merge `master` into `Rust-Rewrite` only with a go-ahead.
- Repo settings: Pages source = GitHub Actions; Actions > "Allow GitHub Actions to create and approve pull requests"
  (fixes `sync_upstream` and `cpp2rust-draft` PR opening); a security-scan workflow errors `JF_URL must be provided`.
- Code scanning alerts are unreadable with the session token (403, needs security_events); paste or grant to fix them.
- `nflare`: unidentified (only `sunflare` shaders match). Needs a repo URL to chop up like Next2Flash.
- Issues: close #112, #98 once Linux+macOS+Windows builds are green on the tip; #66/#71 need a macOS launch proof;
  #116/#114/#110/#117 need a regression test or guard before closing. Do not close feature requests.

## GUI QA harness (scripts/dev/qa/, added in QA3)
`BUILD_DIR=<build> scripts/dev/qa/run_all.sh [--tools|--import|--export|--prefs|--rooms|--all] [--dir DIR] [--out OUT]`
runs the app under Xvfb with a fresh HOME and `stuff/` copy per launch and writes `OUT/results.{json,md}`, `shots/`, `logs/`,
`crash/` (backtraces), `artifacts/` (exports); exit 1 on any FAIL. About 9 minutes for `--all`. Haiku just runs it.
- It drives the app with xdotool; widget rectangles come from the app itself through the QA hook (`FLARE_QA_HOOK=<dir>`, only
  active when set; `flare/sources/flare/qahook.cpp`: save/load/new/frame/cmd/geom/rooms/prefs/export/moho/button commands).
- Covers: 22 tools (stroke + pixel diff, mid-drag shot for selection tools), Brush auto-create on an empty cell, undo/redo
  (Ctrl+Z/Y and MI_Undo), next-frame, playback, .tnz save/reload, Preferences pages, all rooms of all 5 layout profiles,
  imports of `tests/flash_fixtures/*`, 3 Moho samples (`tests/flash_fixtures/moho_samples`) and synthetic PNG/JPG/GIF/SVG,
  XFL/FLA/HTML5 export + re-import count check, HTML5 in headless chromium (`html5_check.js`, no console errors, non-blank).
- `FLARE_QUIET_IMPORT=1` logs the import result instead of showing the modal summary (set by gui_smoke and the harness).
- `gui_smoke.sh` now fails (rc 4) when ANY dialog/modal window of the app is open, except the "Flare Startup" popup (`qa/dialogs.sh`).
- Last run (QA3): 118 PASS, 1 FAIL, 1 UNTESTED. Open: playback (MI_Play / the P shortcut leaves the frame counter
  constant under Xvfb; the Pause button stays active; may be environment related, check on a real display),
  `T_PaintBrush`/`T_Finger` only checked for the correct "raster levels only" warning on a Vector level (not draw-tested on a Toonz
  Raster level), `MI_NextFrame` via the command path needs viewer focus (works from the keyboard).
- Known app behaviours the harness records but does not call defects: XFL export `X.xfl` writes folder `X/` containing `X.xfl`;
  `ruffle_DefineShape4.swf` reports 0x0 px / 0 fps (header parse of that compressed SWF v12) and "1 morph shape(s) not converted".
- Fixed in QA3 (all were crashes or blockers found by the harness): `TXshSimpleLevel::m_palette` was a raw pointer (use after free in
  `load()`: SVG/PSD/Toonz Raster levels, every XFL/FLA/SWF import and Brush auto-create crashed); PSD 1-pixel image SIGFPE
  (`common/psdlib/psd.cpp`); StyleEditor null-style guard; XFL import duplicated bitmap layers; extension-less XFL folder
  argument; D19 (no Import/Load prompt for importer/CLI loads, PSD options popup skipped for them), D20 (QString::arg without
  placeholder), D21 (empty-layer FLA now says so), D18 (smoke fails on any dialog).
- Still to do: raster-level drawing tests (PaintBrush, Finger), Fx schematic, xsheet cell edits, Flash Instance panel, script console,
  IK tools, cleanup, sound, render; remaining style defects D1-D8 from the earlier report (layout clipping, theme contrast).

## Code scanning (readable via check-run annotations: `gh api repos/Flare-Animate/Flare/check-runs/<CodeQL run id>/annotations`)
Found 2026-10-24 on PR #151 (check run 120971320646, 29 high + 2 medium):
- 29x "Access of invalid pointer" in `flare/sources/image/tzp/avl.rs` (raw pointer derefs in the faithful AVL port;
  add null/bounds guards or safe wrappers at the C ABI edge), 1x `flarelib/sandor_fxs/PatternPosition.rs:422`,
  1x `common/flash/FSWFStream.rs:664`.
- 2x "Workflow does not contain permissions" in `.github/workflows/rust-tests.yml` (lines 27, 53): add `permissions: contents: read`.

## Known defects (from docs/TEST_REPORT_FLASH.md)
- AS3-only SWF import reports 0x0 px / 0 frames (header parse).
- PSD import loads the flattened image as a level (layer/group options use the PSD popup defaults for CLI loads).
- Unzipped XFL folder cannot be chosen in the import dialog; typed filename in the file browser is ignored.
- Command line / open-file: non-.tnz files are routed (magic bytes first) to the importers via `openNonSceneFile` (flashimport.cpp): FLA/XFL/SWF/SWC/AI etc. use the Flash import; PSD/GIF/MP4/SVG/PLI/raster load as levels. Verify GIF/MP4 decoding in a real session.
- `flare-cli`: `cd flare/sources && cargo build -p ac-cli`; needs network for the effectcraft git deps (pinned in flare/sources/Cargo.toml). Binary: flare/sources/target/debug/flare-cli.
- `tools/flash/import_container.py` (D16) intentionally handles Flash containers/media only; GIF/MP4/PLI/SVG are not Flash containers and are opened by the app itself (File > Load Level, or as an argument), so the helper still reports "Unsupported file type" for them.
- About dialog loads `:Resources/tahoma2d_about.png` which does not exist (blank logo).
- Packaging: `.deb` rename in `ci-scripts/linux/flare-buildpkg.sh` is a guess; macOS bundle must provide `Flare.real`.
- Animate layout: empty black Flash Instance strip in room 2; existing users keep old personal layouts until
  Windows > Workspace > Reset Room Layout; shared `settings/commandbar.xml` is still the OpenToonz default.

## AnimationCraft wiring (see "AnimationCraft wiring status" in docs/REWRITE_BACKLOG.md)
- Verify in a real GUI session (scripts/dev/gui_smoke.sh): Import Flash on `tests/flash_fixtures/handmade.fla`, File > Export > Export as XFL / Export HTML5.
- Fix `flare/sources/flare/Cargo.toml` (missing `photocraft-psd` workspace dep, egui 0.29 vs 0.36 pin) and add ac-app as an optional workspace member, then add "Open in Flare Animate" that launches it on an exported XFL.
- Export Toonz raster/vector levels and sound columns (render to PNG/WAV in C++ before the manifest), keep symbols as symbols.
- Check Windows + macOS CI for the new `flare_formats` deps.

## Rewrite (docs/REWRITE_BACKLOG.md, docs/PORTING.md)
- ~1100 C/C++ files remain; most are Qt/OpenGL bound. Qt-free logic files are nearly exhausted; next: Qt UI strategy
  (egui crates) rather than file-by-file ports. The owner's own agent ports `common/trop/*` and `common/tcore/*`.
- Next2Flash stages 2+: swf_to_n2d/compile_n2d (replace with native SWF/HTML5 export), shape_converter, fla_xfl
  vs ac_xfl, bitmap_converter, cycle_detector, mxmlc recompile workflow (docs/NEXT2FLASH_INTEGRATION.md).
- Ported-but-unverified: Windows/MSVC and macOS behaviour of recent ports (avl.rs 64-bit keys, tcolor inline
  forwarders, SWF writer headers); `igs_line_blur` assumes Release/NDEBUG.
- cpp2rust draft workflow works but only drafts tiny class-free files; use it for leaf files only.
- Guards in CI: check_cmake_sources.py, check_header_defs.py, check_preferences_ids.py, check_layout_profiles.py.
