# Flash / Adobe file support test report

Branch: Rust-Rewrite. Samples: `/tmp/flash_samples/` (manifest `/tmp/flash_samples/MANIFEST.txt`).
Source for SWF/PSD/AS/ASE/XFL samples is noted per row. SWF source is ruffle-rs/ruffle at commit `153f940eaa32679de2e321459e14cb2ccad0deb4`, path `tests/tests/swfs/...`.

## 1. Sample files

| Type | File(s) | Source | Size | Tool | Result | First error / note |
|------|---------|--------|------|------|--------|--------------------|
| SWF | 22 files (AS2 + AS3, audio, text, fonts, blend, stage3d, timeline, malformed length) | ruffle `tests/tests/swfs/...` | 56 B – 70 KB | import_container.py | 20 pass, 2 edge cases "pass" wrongly | `swf_length_zero` (56 B) and `swf_length_too_long` report 5 tags and frame_count 1 with no problem flagged |
| SWF | same 22 | same | same | flare-cli info/convert | 0 / 22 fail | `stream did not contain valid UTF-8` (flare-cli does not read SWF for info/convert) |
| SWF | `visual_blend_modes_multiply_test.swf` | same | 2565 B | flare-cli swf (in.swf -> out.swf) | pass | output 2523 B, re-imports with import_container |
| XFL | `dmt_MovieClipExample.xfl` (zip of DOMDocument.xml + 3 LIBRARY files) | XTDStudios/DMT `Samples/fla/MovieClipExampleAssets` | 5479 B | import_container.py | pass | exported 4 files |
| XFL | same, zipped | same | same | flare-cli info/convert | fail | `has no DOMDocument.xml` (zip not opened; message names parent dir) |
| XFL | same, unzipped folder | same | same | flare-cli info/convert/png/svg/html5/sprites | pass | motion-tween "experimental" note only |
| PSD | `gray0.psd`, `hidden-layer.psd`, `stroke-center.psd` | psd-tools/psd-tools `tests/psd_files/` | 29–114 KB | import_container.py | 3 / 3 pass | exported 1 file each |
| AS | `hello.as`, `kingdom_Sky.as`, `AutoAni_stage.as` | open-source-flash/open-source-flash, noio/kingdom, LePtC/AutoAni | 0.4–10 KB | import_container.py | 3 / 3 pass | copied only (per docs) |
| ASE | `mc2_hub.ase` | alariq/mc2srcdata `tgl/hub.ase` | 50 KB | import_container.py | fail | `Unsupported file type: .ase`. This is 3DS ASCII Scene Export text, not Aseprite. Aseprite binary not tested |
| FLA | none usable | GitHub `.fla` hits (qqpet, rCubed, hqp, Prime-World) are Git LFS pointers | - | - | not tested | no real CFBF/ZIP FLA obtained online |
| FLV | none usable | G2FUZZ / milw0rm hits were text or HTML | - | - | not tested | local `tests/flash_fixtures/sample.flv` passes import (control) |
| F4V | none usable | cinelerra hits were text, not ISO-BMFF | - | - | not tested | local `sample.f4v` passes import (control) |
| JSFL | none online | - | - | - | not tested | local `sample.jsfl` passes import (control) |
| AIR | none Flash | Ikemen/MUGEN `.air` is animation data | - | - | not tested | local `sample.air` passes import (control, 3 files) |
| ACO | none Flash | hits are unrelated (ESRI/GAP) | - | - | not tested | - |
| ASE (Aseprite) | 1 seen in aseprite repo, not copied | aseprite/aseprite `tests/sprites/` | - | - | not tested | - |

Totals (online samples, 30 files): import_container 30 runs, 28 pass, 1 fail (.ase), and 2 SWF edge cases pass that should fail (length-zero and length-too-long). flare-cli: 22 SWF fail, 1 zipped XFL fails, 1 XFL folder passes all subcommands tried.

## 2. Real app (`/home/user/bld/bin/Flare`, not rebuilt)

| Run | Result | Evidence |
|-----|--------|----------|
| main window, swf sample | FAIL, exit 127 | `symbol lookup error: undefined symbol: _Z9toPixel32RK12TPixelRGBM64` |
| load xfl sample | FAIL, same | same log |
| load psd sample | FAIL, same | same log |
| File > Import menus (xdotool) | not reached | binary never got a window |

Screenshots (black frame, no Flare window): `/tmp/flash_gui/swf/fail.png`, `/tmp/flash_gui/xfl/fail.png`, `/tmp/flash_gui/psd/fail.png`. Logs: `/tmp/flash_gui/<type>/run.log`.

Root cause: the binary links a symbol that the tree no longer defines. `grep` finds no declaration of `toPixel32(const TPixelRGBM64&)` under `flare/sources`. The binary is stale relative to source. Fix: rebuild `/home/user/bld` (not done, per instruction).

## 3. Rust and tool-level results

| Check | Result |
|-------|--------|
| `cargo test -p ac-xfl --offline` | 60 pass (42+5+4+9) |
| `cargo test -p ac-package --offline` | 3 pass |
| `python -m pytest tools/flash/tests -q` | 46 pass, 1 skip |
| `tools/flash/lint_repo_scripts.py` | 26 scripts, 0 problems |
| `scripts/build-html5-runtime.sh --check` | up to date |
| `tests/check_issue_fixes.py` | SKIP (its `gh` call fails; `gh api` works in this shell) |
| flare-cli png / svg / html5 / sprites on DMT XFL folder | pass |
| `tools/flash/import_container.py` on local fixtures (sample.air, .jsfl, .flv, .f4v, .fla) | all run; sample.fla is a ZIP and imports |
| local `sample_xfl/sample.xfl` (plain XML file) | misleading message `Unable to parse binary FLA file` |

## 4. Features and issues (headless)

| Item | How | Result |
|------|-----|--------|
| FEATURES.md file-type table (XFL, SWF, FLV/F4V, AS, PSD, AIR) | importer on real samples, section 1 | pass for XFL, SWF, PSD, AS. FLV/F4V/FLA not verified on online files |
| FEATURES.md relative links (9) | `[ -e ]` per link | all exist |
| FEATURES.md "Running the tests" commands | ran pytest, cargo test -p ac-xfl, check_issue_fixes | pytest and cargo pass; issue checker skipped |
| Moho, Next2Flash, AS3 decompiler (FEATURES rows) | not run (no samples, no mxmlc) | not tested |
| #47 Opening FLA files | importer only | not verified: no real FLA. GUI blocked |
| #117 Crash importing FLA and SWF | importer only, 22 SWF + XFL | no crash in importer. GUI crash path not reproduced (binary never starts) |
| #116 Crash by starting, #114 AppImage crash, #112 AppImage, #110 Win11 crash, #98 macOS FFmpeg, #71 macOS | GUI / platform | not tested (binary does not launch; other platforms) |
| #52 JSFL / ZXP / MXP | importer on local sample.jsfl, sample.air | partial: jsfl and air pass, no online ZXP/MXP |
| #123, #125, #66, #55, #53, #49, #50, #51, #56, #25, #15, #11 | feature/UI/process or docs | not tested headless |

## 5. Top failures (ranked)

1. **Flare binary does not launch**: `undefined symbol toPixel32(TPixelRGBM64)`. Stale build. Start at `/home/user/bld/bin/Flare`; source side `flare/sources/common/tparam/tpixelparam.cpp`. Impact: all GUI tests blocked, including #116.
2. **flare-cli rejects every SWF for info/convert**: `stream did not contain valid UTF-8`. Path dispatch in `flare/sources/flareqt/ac_cli_main.rs` (`info`, `convert`) treats input as JSON or XFL and never routes `.swf`. Only `flare-cli swf in.swf out.swf` works.
3. **Malformed SWF reported as imported**: `swf_length_zero` (56 B) and `swf_length_too_long` (334 B) return "Import complete", tags `{0,1,9,12,69}`, frame_count 1, `problems: {}`. Likely the built-in fallback extractor in `tools/flash/import_container.py` (around line 383) fabricates info. Should flag a problem.
4. **Zipped XFL unsupported in flare-cli**: `has no DOMDocument.xml`. Opened directory-only; error names the parent dir. Start at `flare/sources/flareqt/ac_cli_main.rs` (line ~32) and `ac_xfl::open` in `flare/sources/common/flash/`. Python importer handles zip fine.
5. **JPEXS missing, SWF fallback only**: every SWF logs `JPEXS unavailable` and only gets metadata, no SVG/PNG shape export. Environment, not code. `tools/flash/decompile_flash.py`.

Minor: `.ase` returns "Unsupported file type" (`tools/flash/import_container.py` line ~506). Local `sample_xfl/sample.xfl` gets a binary-FLA message.

## 6. Not tested

- Real FLA (CFBF and ZIP) and real FLV/F4V from online sources (no usable files found)
- Aseprite `.ase`/`.aseprite` and `.aco` colour swatches
- All GUI paths: Import FLA/XFL, Import Flash Vector, rooms, menus (binary blocked)
- JPEXS export (decompiler not installed)
- Native C++ suite `tests/native/run_tests.py`, full `tests/run_all.py`, full `cargo test --workspace`
- Moho, Next2Flash, AS3 decompiler, game-export runtime, Android, Windows installer, auto-update
- Issue checker `tests/check_issue_fixes.py` (skipped)

## 7. Reproduction

- Samples: `/tmp/flash_samples/` (not in repo)
- CLI: `flare/sources/target/debug/flare-cli` (built from `ac-cli`, then cleaned)
- Logs and outputs: `/tmp/flash_out/`, `/tmp/flash_gui/`

## GUI at tip

Build: /home/user/bld/bin/Flare links cleanly at tip (ae4974c + 2 source fixes below). Root cause of the earlier
exit 127 (`_Z9toPixel32RK12TPixelRGBM64`): stale/raced incremental build (the symbol is no longer referenced;
a completed ninja run relinked everything). Two real link/compile breaks found at tip and fixed:
- `TRop::expandPaint(TRasterCM32P)` undefined in libflarelib: b8280f4 deleted trop/tdistancetransform.cpp but left
  no C++ forwarder for Rust `flare_expand_paint`. Added in `flare/sources/common/trop/tropcm.cpp` (wrap passed in bytes).
- `flare_shrink` in `flare/sources/common/trop/rust/trop_utils.rs` called the parameter `shrink` (i32) instead of
  the function (E0618); now `self::shrink(...)` (from ae4974c).

Smoke (`scripts/dev/gui_smoke.sh /tmp/smoke_tip`): PASS. Main window up; sample.fla as CLI arg gives modal
"File .../sample.fla doesn't look like a TOONZ Scene" (CLI args are treated as .tnz scenes only). Screenshots:
/tmp/smoke_tip/main.png, /tmp/smoke_tip/load_sample.png.

CLI file argument (Xvfb, isolated HOME, 40 s): every file -> app ALIVE, no crash/hang, but the same modal
"File <path> doesn't look like a TOONZ Scene" (swf shape, swf AS3, swf bitmap, xfl zip, xfl DOMDocument.xml, psd).
Not an import path; use File > Import. Shots: /tmp/flash_gui2/{swfshape,swfas3,swfbitmap,xfl_zip,xfl_dir,psd}.9.png

File > Import (menu has one flash entry "Import Flash (FLA / XFL / SWF / SWC / FLV / F4V / AS)..."; no separate
"Import FLA/XFL as Editable Scene", "Import Flash Vector" or PSD item; PSD goes through the same dialog).
Driven with xdotool (startup popup dismissed by click+Esc; file browser: typed path in File name field is ignored and
the importer receives the browsed folder -> "Failed to read XFL: DOMDocument.xml not found in XFL directory:
.../sandbox/scenes"; select by clicking the list row instead). Results, all processes ALIVE, no log errors:
- SWF shapes (gnash shape_test): imports; prompt "File /tmp/flare_flash_import_*/shape0001_id4.svg doesn't belong to
  the current project. Import / Load / Cancel" (OK flow). imp_swfshape_3later.png
- SWF AS3 (avm2_array_indexof_test): dialog "Flash import complete. SWF v38 | 0 x 0 px | 0 fps | 0 frame(s)
  [compressed]; Not converted to levels: 1 named symbol(s); 1 ActionScript 3 block(s); 1 class decompiled; 2 files
  exported". Header shows 0x0/0 fps/0 frames for this SWF (dimension/frames parse suspect). imp_swfas3_3later.png
- SWF bitmaps (LoadBitmapTest): prompt for bitmap_.jpg import/load. imp_swfbitmap_3later.png
- XFL zip (dmt_MovieClipExample.xfl): prompt for xfl_svg/L0_.svg. imp_xflzip_3later.png
- XFL unzipped dir: folder cannot be chosen in the file browser (only the .xfl file type is listed; inside the dir it
  shows LIBRARY with no entries) -> not importable from GUI. imp_xfldir_1c_sel.png
- PSD (gray0.psd selected): "Flash import complete. PSD file copied for reference. 1 file(s) exported" (PSD is only
  copied, no layers imported). imp_psd_3later.png
No crashes, hangs or native error dialogs other than the ones quoted. Screenshots in /tmp/flash_gui2/.
