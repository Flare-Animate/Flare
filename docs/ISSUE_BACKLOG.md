# Open issue backlog

Snapshot of all 20 OPEN issues in Flare-Animate/Flare (fetched via GitHub MCP, perPage 50, no further pages). Module dirs were checked with `ls`/`grep` under `flare/sources`. "Addressed" is a quick grep judgment, not a verification; `docs/ISSUE_MAP.md` and `tests/check_issue_fixes.py` remain the authoritative probe.

Legend: addressed = yes / partial / no / unknown.

| # | Title | Labels | Gist | Category | Module dir (verified) | Addressed in repo? (evidence) |
|---|---|---|---|---|---|---|
| 123 | Interactive Project Support | (none) | Want ActionScript 2/3 and JavaScript interactive-project support (e.g. Homestar Runner) | feature | `flare/sources/common/flash/` (As3Bridge.cpp, SWFAssets) | partial: AS3 bridge exists, no AS2/JS runtime |
| 117 | Flare crashes when importing FLA and SWF files (Windows) | bug, help wanted, priority | Crash ~1% into import, FLA and SWF, Win10 | bug | `flare/sources/flare/flashimport.cpp`, `flare/sources/common/flash/` | unknown (no crash-specific fix found) |
| 116 | Crash by starting (Linux, SIGABRT in libtnzcore) | (none) | Crash on launch from AppImage 1.7.1, abort inside libtnzcore | bug | `flare/sources/common/tapptools/` (tenv.cpp), `flare/sources/flare/crashhandler.cpp` | unknown |
| 114 | Crash on opening AppImage | (none) | Same SIGABRT backtrace on KDE neon | bug | as #116 | unknown |
| 112 | AppImage doesn't work even with FLAREROOT "fix" | (none) | FLAREROOT error despite `/usr/share/flare/stuff` set | bug | `flare/sources/common/tapptools/tenv.cpp` (line ~757 fallback) | partial: fallback logic exists, user still fails |
| 110 | Flare crashes on Windows 11 | (none) | WinDbg shows C++ exception at startup; portable build fails with `portablestuff` present | bug | `flare/sources/flare/main.cpp` (line ~373 AA_UseDesktopOpenGL), `flare/sources/flare/` | unknown |
| 98 | Flare 1.7.1 fails to launch on macOS (missing FFmpeg dylib) | bug, help wanted, priority | Hard-coded `/usr/local/Cellar/ffmpeg/9.0.1/lib/libavfilter.12.dylib` reference | bug | `ci-scripts/osx/bundle-homebrew-dylibs.sh`, `.github/workflows/nightly.yml` | partial: bundler rewrites `/usr/local/*.dylib` only; nested Cellar path likely not covered |
| 71 | MacOS error (copied from Discord) | bug, priority | Blocked-icon/Gatekeeper error on M2 Mac; no logs attached | bug | (macOS packaging, `ci-scripts/osx/`) | no: needs reproduction data |
| 66 | App is incomplete | bug, enhancement, good first issue, help wanted, beginner friendly, priority | Nightly macOS build has no unix executable | build | `ci-scripts/osx/`, `.github/workflows/workflow_macos.yml` | unknown (CI bundle workflow exists) |
| 56 | [Linux] Building from source gave me a kernel panic | bug, good first issue, priority | Source build pegged CPU and crashed laptop (4 GB RAM) | build | `doc/how_to_build_linux.md`, `docs/how_to_build_linux.md` | partial: docs exist; no resource-limit fix |
| 55 | Finding more developers | good first issue, help wanted, priority | Recruitment request | docs | none | n/a (not code) |
| 53 | Making Flare Studio Pipeline Ready | good first issue | Rooms, node compositing, collaboration/asset linking | feature | `flare/sources/toonzqt/` | partial: Studio rust module exists; major features unbuilt |
| 52 | JSFL/ZXP/MXP Integration | enhancement, good first issue, priority | Plugin integration for JSFL/ZXP/MXP | feature | `flare/sources/common/expressions/` (jsfl.js), `flare/sources/flare/flashimport.cpp` | partial: jsfl.js and ZXP/MXP dispatch exist; no plugin install |
| 51 | title mix up | good first issue, beginner friendly | Window/title text conflicts with Rive "Flare" and ASUS Strix Flare | ui | `flare/sources/flare/` | no: expected title text not defined in tree |
| 50 | Logo Rebrand | help wanted, beginner friendly, translation | Subjective logo redesign request | ui | `flare/sources/flare/Flare.ico`, `Flare.icns`, `flare/sources/flare/Resources/` | no |
| 49 | OpenToonz Rigging -> Harmony Rigging | enhancement? (none listed) | Unified node-based rigging (IK, constraints, master controllers, auto-patching) | feature | `flare/sources/tnztools/skeletontool.cpp`, `flare/sources/tnztools/plastictool_meshedit.cpp`, `flare/sources/common/moho/` | partial: skeleton and plastic tools exist; no rigging nodes |
| 47 | Opening FLA files doesn't work + Animate room layout | bug, help wanted, priority | FLA open command missing in Animate room | bug | `flare/sources/flare/flashimport.cpp`, `flare/sources/common/flash/XFLReader.cpp`, `stuff/profiles/layouts/rooms/Animate/` | yes per ISSUE_MAP (XFL/FLA import and Animate room exist); layout persistence unverified |
| 25 | Pencil tool -> CSP vector brush | enhancement | Per-node stroke width/opacity metadata with pressure | feature | `flare/sources/flareqt/schematicnode.cpp` | partial per ISSUE_MAP; brush metadata not confirmed |
| 15 | Migrate features from Tahoma | enhancement, beginner friendly | Port Tahoma features and look | feature | none specific | no |
| 11 | Custom toolbar | enhancement, beginner friendly | JSFL-style custom toolbar scripts | ui | `flare/sources/flareqt/docklayout.cpp`, `doc/LAYOUT_CUSTOMIZATION.md` | partial: customizable dock layout exists; scripted actions not |

## Counts by category

- bug: 8 (117, 116, 114, 112, 110, 98, 71, 47)
- feature: 6 (123, 53, 52, 49, 25, 15)
- build: 2 (66, 56)
- ui: 3 (51, 50, 11)
- format: 0
- docs: 1 (55)

Total: 20.

## Highest-value items

1. #117 crash on FLA/SWF import (Windows): blocks core workflow.
2. #110 Windows 11 startup crash: blocks launch.
3. #116 / #114 Linux AppImage SIGABRT on start: blocks launch.
4. #98 macOS FFmpeg dylib path: blocks launch on macOS.
5. #112 FLAREROOT on AppImage: likely shared root cause with the Linux crashes.
6. #47 FLA opening / Animate room: already mostly answered, verify.
7. #71 macOS error: needs logs before it can be acted on.
8. #66 nightly macOS executable: CI packaging.
9. #56 Linux source build resource exhaustion: docs-only today.
10. #52 JSFL/ZXP/MXP integration: partial code exists.
