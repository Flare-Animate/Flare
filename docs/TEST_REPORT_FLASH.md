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

---

## 8. QA pass 2026-10-25 (Rust-Rewrite, binary /home/user/bld/bin/Flare 1.7.1 built 2026-10-22, not rebuilt)

Samples: 26 archive.org files in 26 fresh dirs under `/tmp/flash-samples/<n>/` (untrusted, not committed), plus 400 files extracted from ruffle-rs/ruffle source zip (`tests/tests/swfs`, 121 SWF incl. v1-v5 and ZWS LZMA, 107 FLA). Ground truth = own stdlib SWF header parser (`swfinfo.py`, run with `python3 -I`). Import = `tools/flash/import_container.py` (olefile installed to a private dir, not system). GUI = Xvfb per file, process alive check, window-title list, screenshot. No JPEXS on PATH.

### 8.1 Sample matrix (PASS/FAIL per step; `GUI` = CLI-arg open, the only open path reachable without xdotool)

Legend: IMP = import helper rc/message. HDR = import metadata vs header truth (frames, fps, version, AS3 flag). GUI = app alive + modal. RT = export round trip. Source URLs are `https://archive.org/details/<item>` (file in parens).

| # | Sample (source) | Feature | IMP | HDR | GUI | RT |
|---|---|---|---|---|---|---|
| 1 | Strong_Bad_Email_001 (`Strong_Bad_Email_001_Some_Kinda_Robot`, .swf CWS v6) | AS2, 556 fr, 12 fps, stream sound, 5 buttons | PASS (warn JPEXS) | PASS | FAIL: "doesn't look like a TOONZ Scene" (D13) | FAIL (SWF->XFL 2 frames, F5) |
| 2 | magneto3 (`Magneto3.swf`, CWS v8) | AS2, 16 bitmaps, 8 buttons, 46 sprites | PASS | PASS | FAIL (D13) | FAIL (F5) |
| 3 | ants (witch-hut, CWS v11) | AS3, 17 DoABC, 960x600 | PASS, only swf_info.json, no ABC export (F3) | PASS | FAIL (D13) | not run |
| 4 | box-monster (witch-hut, CWS v11) | AS3, 162 DoABC | PASS (no ABC export, F3) | PASS | FAIL (D13) | not run |
| 5 | clean-teeth (witch-hut, CWS v11) | AS3, 22 DoABC | PASS (F3) | PASS | FAIL (D13) | not run |
| 6 | bicycle (witch-hut, CWS v11) | AS3, 18 DoABC | PASS (F3) | PASS | FAIL (D13) | not run |
| 7 | tedi-ytp (`tedi-ytp.swf`, 27 MB) | video | FAIL: HTTP 500 on download | - | - | - |
| 8 | johnny test (`johnny test.fla`, CFBF CS3 binary) | FLA binary, 783 streams | PASS msg but F2 (raw OLE dump) | n/a | FAIL (D13) | not run |
| 9 | just say here bro (johnny-test, CWS v8) | AS2, 455 fr | PASS | PASS | FAIL (D13) | not run |
| 10 | Prep 3 unit 10a (`Prep 3 unit 10a.swf`, CWS v9) | AS3 (2 DoABC), 2 fr, 44 sprites | PASS (F3) | PASS | FAIL (D13) | not run; truncated variant passes (F4) |
| 11 | mw_ecard_01 (nick, CWS v7) | AS1, 110 fr, 30 fps, 11 sounds | PASS | PASS | FAIL (D13) | not run |
| 12 | Bomber (bomber_202503, FWS v7) | AS2, 65 fr, 10 DoAction | PASS | PASS | FAIL (D13) | not run |
| 13 | isaac demo (`isaac demo.swf`, CWS v8, 9 MB) | 282 sounds, 99 bitmaps, 68 buttons, 1436 sprites | PASS | PASS | FAIL (D13) | not run |
| 14 | intro2 (intro2_202509, FWS v5) | AS1, 389 fr | PASS | PASS | FAIL (D13) | not run |
| 15 | flash_picosschool (FWS v8) | 84 buttons, 22 fonts, 15 sounds | PASS | PASS | FAIL (D13) | not run |
| 16 | mpf0003-1 (`mpf0003-1.fla`, ZIP = CS5+ XFL-in-FLA) | 1920x1080 25 fps, 1 timeline, 4 layers, 11 sounds, 1 bitmap, 145 symbols | PASS (150 files) | PASS (DOMDocument: 1920x1080 25fps) | FAIL (D13 "not TOONZ") | PASS: XFL read (`xfl_handler --read`, 145 symbols) and reimport of XFL folder (300 files; subfolders kept) |
| 17 | mpf0003-1 (`mpf0003-1.swf`, CWS v15) | 1550 fr, 1920x1080 | PASS | PASS | FAIL (D13) | not run |
| 18 | asset pack (`asset pack.fla`, CFBF) | binary FLA | PASS msg, F2 (56 raw streams) | n/a | FAIL (D13) | not run |
| 19 | asset pack (`asset pack.swf`, CWS v8) | AS2, 801 fr | PASS | PASS | FAIL (D13) | not run |
| 20 | asset pack (`another test.swf`, CWS v8) | AS2, 801 fr | PASS | PASS | FAIL (D13) | not run |
| 21 | rig_scratchcat (`rig_scratchcat.fla`, CFBF) | binary FLA | PASS msg (olefile needed, F2) | n/a | FAIL (D13) | not run |
| 22 | rig_scratchcat (`rig_scratchcat.swf`, CWS v8) | 15 sprites, 1 fr | PASS | PASS | FAIL (D13) | not run |
| 23 | rig_scratchcat (`rig_scratchcat.zip`) | zipped XFL/SWF bundle | PASS (4 files) | n/a | FAIL (D13) | not run |
| 24 | hyy_rigs (`hyy_rigs.fla`, CFBF) | binary FLA | PASS msg, F2 (1111 raw streams) | n/a | FAIL (D13) | not run |
| 25 | demo reel 2010 (`demo_reel_2010_by_zeurel_d2ou9a5.swf`, CWS v8, 1938 fr) | long timeline | PASS | PASS | FAIL (D13) | not run |
| 26 | chileno aldea (`flash-chileno-aldeaflash-lost.swf`, CWS v7) | 2128 fr, 22 sounds | PASS | PASS | FAIL (D13) | not run |
| 27 | ruffle `tests/tests/swfs/lzma.swf` (ZWS, from ruffle source zip, `ruffle-rs-ruffle-2024-03-15`) | LZMA-compressed SWF | not run individually (batch in 29/) | - | - | - |
| 28 | ruffle `DefineBits-JpegTables-MX.swf` (FWS v1) | old SWF v1 | not run individually | - | - | - |
| 29 | ruffle `avm1/add_swf4/test.swf` (FWS v4), `add_swf5` (FWS v5) | old AVM1 | not run individually | - | - | - |
| 30 | ruffle source zip (non-Flash control, 98 MB) | - | FAIL: no result within 120 s (F9) | - | - | - |

Ground-truth result: 14 of 14 SWFs with a readable header in the helper's `swf_info.json` match header truth for frame count, fps, SWF version and AS3 flag (PASS). Width/height are not written to `swf_info.json` at all (F3), so dimensions are UNVERIFIED for import.

Totals: 26 archive samples + 121 ruffle SWF + 107 ruffle FLA downloaded. Import helper: 25 of 25 downloaded archive files rc=0 (CFBF FLAs need olefile, F12); 2 downloads failed (tedi-ytp HTTP 500, Super Mario 63 HTTP 503); ruffle source zip control timed out (F9). GUI: 23 of 23 CLI-arg opens ALIVE but each raises the D13 modal (window title `Flare 1.7 - Critical`, screenshots below). XFL folder as CLI arg: ALIVE, no dialog, nothing opened (F10). File > Import through xdotool was NOT driven for all samples (time); the D13 fix agent should re-run it.

### 8.2 Export round trips

| Step | Result | Evidence |
|---|---|---|
| XFL read (`xfl_handler.py --read`) of mpf0003-1 XFL folder | PASS | 145 symbols, 1920x1080 @25 |
| XFL write then reimport (`--write sample`, import helper) | PASS, but trivial | creates DOMDocument + LIBRARY + PublishSettings; reimport "Exported 2 files" |
| XFL folder reimport (mpf) vs original | PASS with note | 300 files exported, subfolders `Duplicate Items Folder/`, `chara/` preserved; orig LIBRARY top-level 130 files, all names present under subfolders |
| SWF -> XFL (`--convert-swf`, JPEXS absent) | FAIL (F5) | rc 0 and "Converted"; DOMDocument has 2 DOMFrame for 556-frame SWF 1 and 2 for 6-frame magneto3; no shapes |
| SWF -> HTML5 (`game_export.py scene.json`) | UNTESTED for real samples (F11) | no importer produces scene.json; minimal scene `{...}` renders 3215-byte HTML (PASS); malformed JSON gives raw JSONDecodeError traceback |
| SWF / FLA export via GUI Export dialog | UNTESTED | not driven |
| flare-cli info/convert | UNTESTED | D15 workspace issue not re-checked this pass |

### 8.3 Fuzz (5 base samples; 60 s timeout each; `/tmp/flash-fuzz/`)

Bases: S1 (Strong Bad, CWS), S2 (magneto3, CWS), S10 (Prep 3, CWS AS3), S14 (intro2, FWS), S16 XFL zip (mpf0003-1.fla). Variants: truncated (half), 64 random bit flips (seeded), zero-length, bad compression (zlib/LZMA junk after header), huge RECT (2^30 twips, FWS only, FileLength fixed), XFL zip-bomb (200 MB deflate of zeros inside DOMDocument.xml, 1.1 MB archive), XFL zip-slip (`../../escape.txt`).

| Variant | Result |
|---|---|
| S1/S2/S10 trunc | S1, S2 rc 5 "Error -5 incomplete or truncated stream" (clean). S10 PASS with no problem flagged (F4) |
| S1/S2/S10 bitflip | rc 5 "incorrect data check" / "invalid code lengths" (clean, no crash) |
| S1/S2/S10/S14 zero-length | rc 5 "Unexpected error: index out of range" (F6) |
| S1/S2/S10 badcomp | rc 5 zlib errors (clean) |
| S14 trunc / bitflip | rc 4 "malformed SWF: header FileLength" / "tag runs past end of file" (clean, correct) |
| S14 hugedim (2^30 twips RECT, FileLength fixed) | PASS rc 0 (accepted silently, F7) |
| XFL bomb (200 MB inflate) | PASS: 1.1 s, "Exported 1 files", no memory blow-up observed |
| XFL zip-slip | PASS: nothing written outside output dir (`/tmp/escape.txt` absent) |
| hang / crash in any variant | NONE (0 of 20 runs; all < 1.1 s except the 98 MB control at >120 s, F9) |

### 8.4 Screenshots of failures

`docs/images/qa/flash_swf_as2_dialog.png` (D13, SWF), `flash_swf_as3_dialog.png` (D13, SWF AS3), `flash_fla_cfbf_dialog.png` (D13, CS3 FLA), `flash_fla_xfl_zip_dialog.png` (D13, FLA zip), `flash_xfl_folder_arg.png` (XFL folder arg, silent).

### 8.5 Defects (new this pass; D13-D17 from earlier still open)

- **F1 (high, = D13, reconfirmed on 23 of 23 samples).** Repro: `FLAREROOT=/home/user/Flare/stuff /home/user/bld/bin/Flare /tmp/flash-samples/1/Strong_Bad_Email_001_Some_Kinda_Robot.swf`. Modal `File ... doesn't look like a TOONZ Scene`, window title `Flare 1.7 - Critical`. Process stays alive. Also for `.fla` (CFBF and zip), `.psd`, `.swf`, and XFL zip.
- **F2 (high).** CS3 binary FLA is "imported" as raw OLE streams. Repro: `python3 -I tools/flash/import_container.py -i "/tmp/flash-samples/8/johnny test.fla" -o /tmp/x` gives 783 files named `Symbol N` (hyy_rigs gives 1111, asset pack 56, rig 21), no DOMDocument, no timelines or library. Message is "Import complete". Should be a clear "partially supported" warning or a real CFBF-to-XFL decode.
- **F3 (high).** Without JPEXS, SWF import is metadata only. `swf_info.json` has no width/height keys (dimensions unknown to user; GUI showed 0x0 earlier). AS3 ABC blocks (ants: 17 DoABC) are not exported to any `.as` file. Only a warning says so; rc 0. Repro: `import_container.py -i /tmp/flash-samples/3/ants.swf -o /tmp/x`, then `ls /tmp/x` = `swf_info.json` only.
- **F4 (high).** Truncated CWS SWF accepted with no problem. Repro: `/tmp/flash-fuzz/S10_trunc/S10_trunc.swf` (half of Prep 3, 4.25 MB) gives rc 0 "Import complete" and `frame_count 2`, `problems: {}`. The uncompressed FWS truncated sample is correctly rejected (rc 4), so zlib path is inconsistent.
- **F5 (medium).** `xfl_handler.py --convert-swf` returns success but drops timeline content. Repro: `python3 -I tools/flash/xfl_handler.py --convert-swf "/tmp/flash-samples/1/Strong_Bad_Email_001_Some_Kinda_Robot.swf" --output /tmp/x` gives DOMFrame count 2 for 556 frames (magneto3: 2 for 6). Should fail loudly without JPEXS.
- **F6 (medium-low).** Zero-length and some corrupt inputs give raw Python text: `Unexpected error: index out of range` (rc 5). Repro: `: > /tmp/z.swf; import_container.py -i /tmp/z.swf -o /tmp/x`. No crash; message should name the file and "empty file".
- **F7 (low).** Absurd dimensions (RECT 2^30 twips, ~53686 px) accepted silently, nothing reports dimensions. Repro: `/tmp/flash-fuzz/S14_hugedim/S14_hugedim.swf`.
- **F8 (low, harness).** `scripts/dev/gui_smoke.sh` still false-passes on the D13 modal (D14 unchanged). New `/tmp/flash-scripts/gui_one.sh` detects it by window title `Critical` (and would also catch `Error`/`Warning` titles); suggest moving that into gui_smoke.sh.
- **F9 (medium).** Non-Flash 98 MB zip (ruffle source zip) did not return within 120 s in the importer. Repro: `import_container.py -i "/tmp/flash-samples/28/ruffle Source code 2024-03-15.zip" -o /tmp/x`. Should reject unknown zips quickly or stream them. Fuzz 200 MB inflate bomb did finish in 1.1 s, so the slow path is likely the unknown-zip extraction.
- **F10 (low).** XFL folder passed on command line: app alive, nothing opened, no message (silent no-op). Repro: `Flare /tmp/flash-rt/xfl_mpf_src` (screenshot `flash_xfl_folder_arg.png`).
- **F11 (low).** No importer output produces the scene.json that `game_export.py` needs, so HTML5 export is not reachable from an imported Flash file. Malformed scene JSON gives a raw `JSONDecodeError` traceback.
- **F12 (environment).** `olefile` is not installed system-wide; the CFBF FLA path fails with rc 4 "Legacy binary FLA needs 'olefile'" until installed. Add to requirements or bundle.

### 8.6 Not tested (this pass)

File > Import via xdotool for every sample; tedi-ytp and Super Mario 63 (download HTTP errors); individual ruffle ZWS/old-version SWFs through the import path (they are in `/tmp/flash-samples/29/`, only header parsed); PSD, GIF, MP4, SVG, SWC, AS; HTML5 and SWF export from real samples; flare-cli (D15); sound/bitmap pixel comparison; Moho, Lottie, AI, TLV.

### 8.7 Repro harness

Downloads and fuzz are untracked: `/tmp/flash-samples/`, `/tmp/flash-fuzz/`, `/tmp/flash-out/`, `/tmp/flash-gui/`. Scripts: `/tmp/flash-scripts/{fetch.py, swfinfo.py, table.py, compare.py, import_all.py, mkfuzz.py, fuzzrun.py, gui_one.sh, gui_all.sh, runimp.py}`.
