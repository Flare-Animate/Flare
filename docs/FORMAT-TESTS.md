# Format tests

Which real files Flare opens, how they were found, and what came out. The harness is
[scripts/format_matrix.py](../scripts/format_matrix.py): it samples up to 20 files per type from the
sample folders named in its `ROOTS` table, runs `flare import` on each, then `flare info` and, for an
imported project, `flare export` to a PNG, each with a timeout. A row is `ok` (imported, nothing
reported as not carried over), `partial` (imported, with items the importer reported as not
imported or not decoded), `refused` (a typed error and a message, with the root cause), or `crash`
(a panic, a signal or a timeout). The sample files are read in place and are not committed.

Run: `python3 scripts/format_matrix.py --flare target/debug/flare --work DIR --out-md OUT.md --per-type 20`.
The sample set is seed 7, so two runs of the same binary compare row for row.

## Counts

Before: the `flare` build from `origin/main` at `9ebdfc4f`. After: the same harness on this branch.
"expected met" counts a row that did what its type should do: an opened type imported, a refused
type was refused cleanly. Types with no sample are listed, not hidden.

| type | files | ok | partial | refused | crash | expected met | before: ok/partial/refused |
|---|---|---|---|---|---|---|---|
| fla | 20 | 1 | 19 | 0 | 0 | 20/20 | 1/19/0 |
| xfl | 6 | 1 | 5 | 0 | 0 | 6/6 | 1/5/0 |
| xfl-document | 8 | 1 | 7 | 0 | 0 | 8/8 | 1/7/0 |
| xml | 20 | 0 | 0 | 20 | 0 | 20/20 | 0/0/20 |
| swf | 20 | 0 | 20 | 0 | 0 | 20/20 | 0/20/0 |
| spl | 1 | 1 | 0 | 0 | 0 | 1/1 | 1/0/0 |
| swc | 2 | 0 | 0 | 2 | 0 | 2/2 | 0/0/2 |
| as | 20 | 0 | 0 | 20 | 0 | 20/20 | 0/0/20 |
| as2 | 0 | - | - | - | - | - | no sample on disk or from search |
| as3 | 0 | - | - | - | - | - | no sample on disk or from search |
| jsfl | 1 | 0 | 0 | 1 | 0 | 1/1 | 0/0/1 |
| sol | 7 | 0 | 0 | 7 | 0 | 7/7 | 0/0/7 |
| flv | 20 | 0 | 0 | 20 | 0 | 20/20 | 0/0/20 |
| moho | 4 | 4 | 0 | 0 | 0 | 4/4 | 4/0/0 |
| anme | 0 | - | - | - | - | - | no sample on disk or from search |
| toonz | 20 | 0 | 9 | 11 | 0 | 9/20 | 0/9/11 |
| tif | 20 | 0 | 0 | 20 | 0 | 20/20 | 0/0/20 |
| tnx | 0 | - | - | - | - | - | no sample on disk or from search |
| flipaclip | 2 | 0 | 2 | 0 | 0 | 2/2 | 0/2/0 |
| wick | 20 | 0 | 0 | 20 | 0 | 0/20 | 0/0/20 |
| flipnote | 20 | 2 | 13 | 5 | 0 | 15/20 | 2/13/5 |
| png | 20 | 20 | 0 | 0 | 0 | 20/20 | 20/0/0 |
| gif | 20 | 19 | 0 | 1 | 0 | 19/20 | 19/0/1 |
| jpg | 20 | 0 | 0 | 20 | 0 | 20/20 | 0/0/20 |
| svg | 20 | 20 | 0 | 0 | 0 | 20/20 | 20/0/0 |

Across both runs there were no crashes, panics or timeouts.

## Open issues

Each entry names the module and a minimal repro. Items in another agent's module are left for them.

1. **Toonz scene cells (`.tnz`).** An OpenToonz 1.2 xsheet cell written as `<level id='1'/>"-"0` is
   refused as `malformed cell: level "-"0 is not a number`. Repro:
   `flare import /home/user/ref/tnz-samples/opentoonz_sample/dwanko_run.tnz out.flare`. Before this
   branch the same file was refused earlier, at the bare `&` in `Lempel-Ziv & Welch`, and at a
   duplicate level id; those two are fixed. Owner: flare-toonz (`scene/cells.rs`), not started.
2. **Toonz filled regions.** A `.pli` with filled regions imports, but its fills are not decoded:
   "N filled regions are not decoded; only strokes are drawn" (`scribble_chalk.pli`). Owner: flare-toonz.
3. **Toonz `.pli` before version 5.7** is refused by design ("files written before version 5.7 in pli
   is not supported yet", `plum.pli`).
4. **Wick (`.wick`).** The importer has not landed on `origin/main`, so all 20 sampled files are
   refused. Repro: `flare import /home/user/ref/wick-editor/public/examples/reference-random.choice.wick out.flare`.
   Owner: the Wick importer work.
5. **Flash `.fla`, `.xfl` and `.swf` partial imports.** These are the most common `partial` reports
   and they are in another agent's `xfl/` and `swf/` modules. The importer's own messages, counted
   across the sample: "frame scripts are not imported" (30 files of the `.fla`/`.xfl` rows),
   "scripts are not imported" (21 `.swf`), "frame labels and scenes are not imported" (12 `.swf`),
   "sound ... not imported" (11 `.swf`), "DOMDynamicText / DOMStaticText / DOMOvalObject is not
   imported" (`.fla`/`.xfl`), "buttons are not imported" and "color effects are not imported" (`.swf`).
   Repro: `flare import /home/user/ref/flash-samples/web/flashbang/shumway/test/swfs/blendmode/blendmode_1.swf out.flare`.
6. **Flipnote sound.** `keke.ppm` and `memoC.kwz` import, but their sound is not imported ("1 audio
   tracks have no Flare sound layer yet"). Owner: flare-import `flipnote/`, a known gap in
   docs/FLIPNOTE.md.
7. **`.jpg` is not a claimed format.** Flare's file kinds do not claim `.jpg` (a test asserts it), so
   a JPEG is refused with its reason, "JPEG is not imported; Flare reads PNG, GIF and SVG still
   images". Open question for the maintainer: should JPEG be claimed and imported? The claim list is
   not changed here.
8. **Samples missing.** No `.as2`, `.as3`, `.anme`, `.anime` or `.tnx` sample was found on disk or from
   a web search. A Renderosity product zip with a `.anme` exists, but its licence is unclear, so it is
   not used.
9. **Single `.tif` files** are refused by design: Toonz brings its own sequences through `.tnz`.
   The OpenToonz sample sequences were refused as single files, as intended.

## Results that are correct refusals

These rows are refused and the refusal is right; they are kept in the table so a reader can see
them.

- `.gif` `target.gif` is a PNG with a `.gif` name ("malformed GIF header").
- `.ppm` `testorig.ppm` is a libjpeg test image, not a Flipnote PPM ("the magic is not PARA").
- `.xml` files other than an XFL `DOMDocument.xml` (`PublishSettings.xml`, `config.xml`) are refused.
- `.as`, `.sol`, `.flv`, `.swc` and `.jsfl` are refused with their own reason.

## Samples and licences

- OpenToonz sample scenes: `opentoonz/opentoonz_sample`, CC BY-NC 4.0 (`LICENSE` in the folder).
- OpenToonz and Tahoma2D repositories: GPL-3 library files (`.pli`, `.tpl`, `.tif`).
- Ruffle test corpus: MIT OR Apache-2.0.
- Moho, Flipnote, FlipaClip and Wick samples: see each folder; none are committed here.

## Per-file results

Source paths are the sample files as found on disk. The type and result columns are what the
counts above add up.

| type | file | licence/source | result | notes |
|---|---|---|---|---|
| fla | params.fla | flash-samples: mixed GitHub samples, see each repo | partial | 3 unsupported, 1 warnings |
| fla | test.fla | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | partial | 3 unsupported, 3 warnings |
| fla | shape-intersecting-lines.fla | flash-samples: mixed GitHub samples, see each repo | ok | info and still export ok |
| fla | Parent7LB8.fla | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | partial | 0 unsupported, 1 warnings |
| fla | timeline_events_fp10.fla | flash-samples: mixed GitHub samples, see each repo | partial | 11 unsupported, 4 warnings |
| fla | test.fla | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | partial | 1 unsupported, 0 warnings |
| fla | Timeline7.fla | flash-samples: mixed GitHub samples, see each repo | partial | 3 unsupported, 0 warnings |
| fla | test.fla | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | partial | 1 unsupported, 1 warnings |
| fla | blendmode_3.fla | flash-samples: mixed GitHub samples, see each repo | partial | 1 unsupported, 0 warnings |
| fla | layer1.fla | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | partial | 0 unsupported, 1 warnings |
| fla | banana.fla | flash-samples: mixed GitHub samples, see each repo | partial | 1 unsupported, 0 warnings |
| fla | test.fla | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | partial | 1 unsupported, 0 warnings |
| fla | Simple.fla | flash-samples: mixed GitHub samples, see each repo | partial | 1 unsupported, 0 warnings |
| fla | test.fla | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | partial | 1 unsupported, 0 warnings |
| fla | dispatch.fla | flash-samples: mixed GitHub samples, see each repo | partial | 1 unsupported, 0 warnings |
| fla | test.fla | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | partial | 1 unsupported, 0 warnings |
| fla | button3.fla | flash-samples: mixed GitHub samples, see each repo | partial | 1 unsupported, 1 warnings |
| fla | test.fla | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | partial | 1 unsupported, 0 warnings |
| fla | pac3.fla | flash-samples: mixed GitHub samples, see each repo | partial | 10 unsupported, 1 warnings |
| fla | test.fla | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | partial | 2 unsupported, 0 warnings |
| xfl | rich-dynamic.xfl | flash-samples: mixed GitHub samples, see each repo | partial | 2 unsupported, 0 warnings |
| xfl | various-strokes.xfl | flash-samples: mixed GitHub samples, see each repo | partial | 1 unsupported, 0 warnings |
| xfl | various-shapes.xfl | flash-samples: mixed GitHub samples, see each repo | partial | 1 unsupported, 0 warnings |
| xfl | align.xfl | flash-samples: mixed GitHub samples, see each repo | partial | 2 unsupported, 0 warnings |
| xfl | fiveframes.xfl | flash-samples: mixed GitHub samples, see each repo | ok | info and still export ok |
| xfl | bounding-boxes.xfl | flash-samples: mixed GitHub samples, see each repo | partial | 1 unsupported, 0 warnings |
| xfl-document | DOMDocument.xml | flash-samples: mixed GitHub samples, see each repo | partial | 1 unsupported, 0 warnings |
| xfl-document | DOMDocument.xml | flash-samples: mixed GitHub samples, see each repo | partial | 1 unsupported, 0 warnings |
| xfl-document | DOMDocument.xml | flash-samples: mixed GitHub samples, see each repo | partial | 0 unsupported, 1 warnings |
| xfl-document | DOMDocument.xml | flash-samples: mixed GitHub samples, see each repo | partial | 2 unsupported, 0 warnings |
| xfl-document | DOMDocument.xml | flash-samples: mixed GitHub samples, see each repo | partial | 2 unsupported, 0 warnings |
| xfl-document | DOMDocument.xml | flash-samples: mixed GitHub samples, see each repo | partial | 1 unsupported, 0 warnings |
| xfl-document | DOMDocument.xml | flash-samples: mixed GitHub samples, see each repo | ok | info and still export ok |
| xfl-document | DOMDocument.xml | flash-samples: mixed GitHub samples, see each repo | partial | 7 unsupported, 3 warnings |
| xml | PublishSettings.xml | flash-samples: mixed GitHub samples, see each repo | refused | not an XFL document: expected an XFL directory, a modern FLA (zip) or an .xfl file |
| xml | STD_inoSubtractFx.xml | opentoonz-lib: OpenToonz repo (GPL-3), library samples | refused | not an XFL document: expected an XFL directory, a modern FLA (zip) or an .xfl file |
| xml | config.xml | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | refused | not an XFL document: expected an XFL directory, a modern FLA (zip) or an .xfl file |
| xml | STD_fourPointsGradientFx.xml | tahoma2d-lib: Tahoma2D repo (GPL-3), library samples | refused | not an XFL document: expected an XFL directory, a modern FLA (zip) or an .xfl file |
| xml | metadata.xml | flash-samples: mixed GitHub samples, see each repo | refused | not an XFL document: expected an XFL directory, a modern FLA (zip) or an .xfl file |
| xml | STD_dissolveFx.xml | opentoonz-lib: OpenToonz repo (GPL-3), library samples | refused | not an XFL document: expected an XFL directory, a modern FLA (zip) or an .xfl file |
| xml | config.xml | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | refused | not an XFL document: expected an XFL directory, a modern FLA (zip) or an .xfl file |
| xml | STD_inohsvAddFx.xml | tahoma2d-lib: Tahoma2D repo (GPL-3), library samples | refused | not an XFL document: expected an XFL directory, a modern FLA (zip) or an .xfl file |
| xml | metadata.xml | flash-samples: mixed GitHub samples, see each repo | refused | not an XFL document: expected an XFL directory, a modern FLA (zip) or an .xfl file |
| xml | STD_inoFogFx.xml | opentoonz-lib: OpenToonz repo (GPL-3), library samples | refused | not an XFL document: expected an XFL directory, a modern FLA (zip) or an .xfl file |
| xml | config.xml | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | refused | not an XFL document: expected an XFL directory, a modern FLA (zip) or an .xfl file |
| xml | STD_inohlsAddFx.xml | tahoma2d-lib: Tahoma2D repo (GPL-3), library samples | refused | not an XFL document: expected an XFL directory, a modern FLA (zip) or an .xfl file |
| xml | PublishSettings.xml | flash-samples: mixed GitHub samples, see each repo | refused | not an XFL document: expected an XFL directory, a modern FLA (zip) or an .xfl file |
| xml | room5_menubar.xml | opentoonz-lib: OpenToonz repo (GPL-3), library samples | refused | not an XFL document: expected an XFL directory, a modern FLA (zip) or an .xfl file |
| xml | config.xml | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | refused | not an XFL document: expected an XFL directory, a modern FLA (zip) or an .xfl file |
| xml | STD_iwa_FlowPaintBrushFx.xml | tahoma2d-lib: Tahoma2D repo (GPL-3), library samples | refused | not an XFL document: expected an XFL directory, a modern FLA (zip) or an .xfl file |
| xml | IronGuardImport.xml | flash-samples: mixed GitHub samples, see each repo | refused | not an XFL document: expected an XFL directory, a modern FLA (zip) or an .xfl file |
| xml | STD_colorMatrixFx.xml | opentoonz-lib: OpenToonz repo (GPL-3), library samples | refused | not an XFL document: expected an XFL directory, a modern FLA (zip) or an .xfl file |
| xml | config.xml | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | refused | not an XFL document: expected an XFL directory, a modern FLA (zip) or an .xfl file |
| xml | STD_backlitFx.xml | tahoma2d-lib: Tahoma2D repo (GPL-3), library samples | refused | not an XFL document: expected an XFL directory, a modern FLA (zip) or an .xfl file |
| swf | flash_display_Loader.swf | flash-samples: mixed GitHub samples, see each repo | partial | 1 unsupported, 0 warnings |
| swf | test.swf | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | partial | 1 unsupported, 0 warnings |
| swf | blendmode_1.swf | flash-samples: mixed GitHub samples, see each repo | partial | 3 unsupported, 0 warnings |
| swf | test.swf | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | partial | 1 unsupported, 0 warnings |
| swf | slider1.swf | flash-samples: mixed GitHub samples, see each repo | partial | 2 unsupported, 5 warnings |
| swf | test.swf | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | partial | 4 unsupported, 0 warnings |
| swf | tiger.swf | flash-samples: mixed GitHub samples, see each repo | partial | 2 unsupported, 0 warnings |
| swf | test.swf | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | partial | 2 unsupported, 0 warnings |
| swf | race3s.swf | flash-samples: mixed GitHub samples, see each repo | partial | 20 unsupported, 2 warnings |
| swf | test.swf | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | partial | 2 unsupported, 0 warnings |
| swf | show.swf | flash-samples: mixed GitHub samples, see each repo | partial | 4 unsupported, 0 warnings |
| swf | test.swf | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | partial | 2 unsupported, 0 warnings |
| swf | timeline-1.swf | flash-samples: mixed GitHub samples, see each repo | partial | 2 unsupported, 0 warnings |
| swf | test.swf | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | partial | 2 unsupported, 0 warnings |
| swf | flash_ui_ContextMenu.swf | flash-samples: mixed GitHub samples, see each repo | partial | 1 unsupported, 0 warnings |
| swf | Dejagnu.swf | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | partial | 3 unsupported, 0 warnings |
| swf | sound.swf | flash-samples: mixed GitHub samples, see each repo | partial | 2 unsupported, 0 warnings |
| swf | test.swf | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | partial | 3 unsupported, 0 warnings |
| swf | Scene_2_MovieClipTimeline.swf | flash-samples: mixed GitHub samples, see each repo | partial | 3 unsupported, 0 warnings |
| swf | test.swf | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | partial | 2 unsupported, 0 warnings |
| spl | example_spl.spl | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | ok | info and still export ok |
| swc | Shumway.swc | flash-samples: mixed GitHub samples, see each repo | refused | not an XFL document: DOMDocument.xml was not found |
| swc | as3xls-lib.swc | flash-samples: mixed GitHub samples, see each repo | refused | not an XFL document: DOMDocument.xml was not found |
| as | TypographicCase.as | flash-samples: mixed GitHub samples, see each repo | refused | TypographicCase.as: an ActionScript source file (.as) holds code, not an animation; open the .fla or .swf it belongs to |
| as | Test.as | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | refused | Test.as: an ActionScript source file (.as) holds code, not an animation; open the .fla or .swf it belongs to |
| as | class-scope.as | flash-samples: mixed GitHub samples, see each repo | refused | class-scope.as: an ActionScript source file (.as) holds code, not an animation; open the .fla or .swf it belongs to |
| as | Test.as | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | refused | Test.as: an ActionScript source file (.as) holds code, not an animation; open the .fla or .swf it belongs to |
| as | closure0.as | flash-samples: mixed GitHub samples, see each repo | refused | closure0.as: an ActionScript source file (.as) holds code, not an animation; open the .fla or .swf it belongs to |
| as | Test.as | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | refused | Test.as: an ActionScript source file (.as) holds code, not an animation; open the .fla or .swf it belongs to |
| as | bitops-bitwise-and.as | flash-samples: mixed GitHub samples, see each repo | refused | bitops-bitwise-and.as: an ActionScript source file (.as) holds code, not an animation; open the .fla or .swf it belongs to |
| as | Child.as | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | refused | Child.as: an ActionScript source file (.as) holds code, not an animation; open the .fla or .swf it belongs to |
| as | Timer.as | flash-samples: mixed GitHub samples, see each repo | refused | Timer.as: an ActionScript source file (.as) holds code, not an animation; open the .fla or .swf it belongs to |
| as | InternalClassInner.as | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | refused | InternalClassInner.as: an ActionScript source file (.as) holds code, not an animation; open the .fla or .swf it belongs to |
| as | Endian.as | flash-samples: mixed GitHub samples, see each repo | refused | Endian.as: an ActionScript source file (.as) holds code, not an animation; open the .fla or .swf it belongs to |
| as | Test.as | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | refused | Test.as: an ActionScript source file (.as) holds code, not an animation; open the .fla or .swf it belongs to |
| as | printers.as | flash-samples: mixed GitHub samples, see each repo | refused | printers.as: an ActionScript source file (.as) holds code, not an animation; open the .fla or .swf it belongs to |
| as | noReturnCustomParam.as | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | refused | noReturnCustomParam.as: an ActionScript source file (.as) holds code, not an animation; open the .fla or .swf it belongs to |
| as | MaskTest.as | flash-samples: mixed GitHub samples, see each repo | refused | MaskTest.as: an ActionScript source file (.as) holds code, not an animation; open the .fla or .swf it belongs to |
| as | AVStreamSwitchEvent.as | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | refused | AVStreamSwitchEvent.as: an ActionScript source file (.as) holds code, not an animation; open the .fla or .swf it belongs to |
| as | String.as | flash-samples: mixed GitHub samples, see each repo | refused | String.as: an ActionScript source file (.as) holds code, not an animation; open the .fla or .swf it belongs to |
| as | test.as | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | refused | test.as: an ActionScript source file (.as) holds code, not an animation; open the .fla or .swf it belongs to |
| as | AccessibilityProperties.as | flash-samples: mixed GitHub samples, see each repo | refused | AccessibilityProperties.as: an ActionScript source file (.as) holds code, not an animation; open the .fla or .swf it belongs to |
| as | Test.as | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | refused | Test.as: an ActionScript source file (.as) holds code, not an animation; open the .fla or .swf it belongs to |
| jsfl | AddPlaceObject4Data.jsfl | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | refused | AddPlaceObject4Data.jsfl: a JSFL script (.jsfl) runs inside Animate and is not an animation |
| sol | amf_object_payload.sol | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | refused | amf_object_payload.sol: a Flash shared object (.sol) holds saved data, not an animation |
| sol | strict_array_test.sol | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | refused | strict_array_test.sol: a Flash shared object (.sol) holds saved data, not an animation |
| sol | RuffleTest.sol | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | refused | RuffleTest.sol: a Flash shared object (.sol) holds saved data, not an animation |
| sol | RuffleTestRef.sol | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | refused | RuffleTestRef.sol: a Flash shared object (.sol) holds saved data, not an animation |
| sol | RuffleTest.sol | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | refused | RuffleTest.sol: a Flash shared object (.sol) holds saved data, not an animation |
| sol | avm1_test_suite.ruffle.sol | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | refused | avm1_test_suite.ruffle.sol: a Flash shared object (.sol) holds saved data, not an animation |
| sol | avm1_test_suite.sol | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | refused | avm1_test_suite.sol: a Flash shared object (.sol) holds saved data, not an animation |
| flv | rufflelogo.flv | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | refused | rufflelogo.flv: a Flash video (.flv) is not imported; Flare opens animations and still images |
| flv | tone_stereo_44100hz.flv | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | refused | tone_stereo_44100hz.flv: a Flash video (.flv) is not imported; Flare opens animations and still images |
| flv | tone_mono_48000hz.flv | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | refused | tone_mono_48000hz.flv: a Flash video (.flv) is not imported; Flare opens animations and still images |
| flv | tone_mulaw_mono.flv | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | refused | tone_mulaw_mono.flv: a Flash video (.flv) is not imported; Flare opens animations and still images |
| flv | tone_stereo_48000hz.flv | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | refused | tone_stereo_48000hz.flv: a Flash video (.flv) is not imported; Flare opens animations and still images |
| flv | square.flv | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | refused | square.flv: a Flash video (.flv) is not imported; Flare opens animations and still images |
| flv | result.flv | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | refused | result.flv: a Flash video (.flv) is not imported; Flare opens animations and still images |
| flv | test.flv | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | refused | test.flv: a Flash video (.flv) is not imported; Flare opens animations and still images |
| flv | square.flv | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | refused | square.flv: a Flash video (.flv) is not imported; Flare opens animations and still images |
| flv | test.flv | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | refused | test.flv: a Flash video (.flv) is not imported; Flare opens animations and still images |
| flv | test_video.flv | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | refused | test_video.flv: a Flash video (.flv) is not imported; Flare opens animations and still images |
| flv | hsv.flv | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | refused | hsv.flv: a Flash video (.flv) is not imported; Flare opens animations and still images |
| flv | audio_timewarp.flv | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | refused | audio_timewarp.flv: a Flash video (.flv) is not imported; Flare opens animations and still images |
| flv | dummy.flv | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | refused | dummy.flv: a Flash video (.flv) is not imported; Flare opens animations and still images |
| flv | square.flv | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | refused | square.flv: a Flash video (.flv) is not imported; Flare opens animations and still images |
| flv | test_video.flv | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | refused | test_video.flv: a Flash video (.flv) is not imported; Flare opens animations and still images |
| flv | test_video.flv | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | refused | test_video.flv: a Flash video (.flv) is not imported; Flare opens animations and still images |
| flv | tone_mono_22050hz.flv | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | refused | tone_mono_22050hz.flv: a Flash video (.flv) is not imported; Flare opens animations and still images |
| flv | tone_stereo_22050hz.flv | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | refused | tone_stereo_22050hz.flv: a Flash video (.flv) is not imported; Flare opens animations and still images |
| flv | square.flv | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | refused | square.flv: a Flash video (.flv) is not imported; Flare opens animations and still images |
| moho | CharWalk.moho | godot-moho: godot_moho_importer example, MIT | ok | info and still export ok |
| moho | CharWalk.mohoproj | moho-samples: Moho samples, see the folder | ok | info and still export ok |
| moho | CharWalk.mohoproj | godot-moho: godot_moho_importer example, MIT | ok | info and still export ok |
| moho | CharWalk.moho | moho-samples: Moho samples, see the folder | ok | info and still export ok |
| toonz | scribble_chalk.pli | opentoonz-lib: OpenToonz repo (GPL-3), library samples | partial | 0 unsupported, 2 warnings |
| toonz | dwanko_run.tnz | opentoonz-sample: OpenToonz sample scenes, CC BY-NC 4.0 | refused | dwanko_run.tnz: malformed cell: level `"-"0` is not a number |
| toonz | plum.pli | tahoma2d-lib: Tahoma2D repo (GPL-3), library samples | refused | plum.pli: files written before version 5.7 in pli is not supported yet |
| toonz | large_brush2.pli | opentoonz-lib: OpenToonz repo (GPL-3), library samples | partial | 0 unsupported, 2 warnings |
| toonz | A_converted.tlv | opentoonz-sample: OpenToonz sample scenes, CC BY-NC 4.0 | refused | A_converted.tlv: not a TLV raster level |
| toonz | small_brush2.pli | tahoma2d-lib: Tahoma2D repo (GPL-3), library samples | partial | 0 unsupported, 2 warnings |
| toonz | pansee.pli | opentoonz-lib: OpenToonz repo (GPL-3), library samples | refused | pansee.pli: files written before version 5.7 in pli is not supported yet |
| toonz | raylit_source.pli | opentoonz-sample: OpenToonz sample scenes, CC BY-NC 4.0 | partial | 0 unsupported, 2 warnings |
| toonz | medium_brush6.pli | tahoma2d-lib: Tahoma2D repo (GPL-3), library samples | partial | 0 unsupported, 2 warnings |
| toonz | large_brush4.pli | opentoonz-lib: OpenToonz repo (GPL-3), library samples | partial | 0 unsupported, 2 warnings |
| toonz | A_painted.tlv | opentoonz-sample: OpenToonz sample scenes, CC BY-NC 4.0 | refused | A_painted.tlv: not a TLV raster level |
| toonz | stai.pli | tahoma2d-lib: Tahoma2D repo (GPL-3), library samples | refused | stai.pli: files written before version 5.7 in pli is not supported yet |
| toonz | buttonhole.pli | opentoonz-lib: OpenToonz repo (GPL-3), library samples | partial | 0 unsupported, 2 warnings |
| toonz | shadow.pli | opentoonz-sample: OpenToonz sample scenes, CC BY-NC 4.0 | partial | 0 unsupported, 7 warnings |
| toonz | ball.pli | tahoma2d-lib: Tahoma2D repo (GPL-3), library samples | refused | ball.pli: files written before version 5.7 in pli is not supported yet |
| toonz | gutt.pli | opentoonz-lib: OpenToonz repo (GPL-3), library samples | refused | gutt.pli: files written before version 5.7 in pli is not supported yet |
| toonz | A_converted.tpl | opentoonz-sample: OpenToonz sample scenes, CC BY-NC 4.0 | refused | A_converted.tpl: a palette is not a project: it holds 21 styles on 2 pages; import a scene or a vector level |
| toonz | medium_brush4.pli | tahoma2d-lib: Tahoma2D repo (GPL-3), library samples | partial | 0 unsupported, 2 warnings |
| toonz | bubb.pli | opentoonz-lib: OpenToonz repo (GPL-3), library samples | refused | bubb.pli: files written before version 5.7 in pli is not supported yet |
| toonz | A_painted.tpl | opentoonz-sample: OpenToonz sample scenes, CC BY-NC 4.0 | refused | A_painted.tpl: a palette is not a project: it holds 19 styles on 1 pages; import a scene or a vector level |
| tif | opripple.0011.tif | opentoonz-lib: OpenToonz repo (GPL-3), library samples | refused | opripple.0011.tif: a single TIFF is not imported; Toonz scenes (.tnz) bring their own image sequences |
| tif | A.0006.tif | opentoonz-sample: OpenToonz sample scenes, CC BY-NC 4.0 | refused | A.0006.tif: a single TIFF is not imported; Toonz scenes (.tnz) bring their own image sequences |
| tif | crater.0001.tif | tahoma2d-lib: Tahoma2D repo (GPL-3), library samples | refused | crater.0001.tif: a single TIFF is not imported; Toonz scenes (.tnz) bring their own image sequences |
| tif | snowflakerot.0002.tif | opentoonz-lib: OpenToonz repo (GPL-3), library samples | refused | snowflakerot.0002.tif: a single TIFF is not imported; Toonz scenes (.tnz) bring their own image sequences |
| tif | A.0002.tif | opentoonz-sample: OpenToonz sample scenes, CC BY-NC 4.0 | refused | A.0002.tif: a single TIFF is not imported; Toonz scenes (.tnz) bring their own image sequences |
| tif | drop2.0001.tif | tahoma2d-lib: Tahoma2D repo (GPL-3), library samples | refused | drop2.0001.tif: a single TIFF is not imported; Toonz scenes (.tnz) bring their own image sequences |
| tif | star.0004.tif | opentoonz-lib: OpenToonz repo (GPL-3), library samples | refused | star.0004.tif: a single TIFF is not imported; Toonz scenes (.tnz) bring their own image sequences |
| tif | A.0003.tif | opentoonz-sample: OpenToonz sample scenes, CC BY-NC 4.0 | refused | A.0003.tif: a single TIFF is not imported; Toonz scenes (.tnz) bring their own image sequences |
| tif | opripple.0012.tif | tahoma2d-lib: Tahoma2D repo (GPL-3), library samples | refused | opripple.0012.tif: a single TIFF is not imported; Toonz scenes (.tnz) bring their own image sequences |
| tif | star.0003.tif | opentoonz-lib: OpenToonz repo (GPL-3), library samples | refused | star.0003.tif: a single TIFF is not imported; Toonz scenes (.tnz) bring their own image sequences |
| tif | 02_trees.tif | opentoonz-sample: OpenToonz sample scenes, CC BY-NC 4.0 | refused | 02_trees.tif: a single TIFF is not imported; Toonz scenes (.tnz) bring their own image sequences |
| tif | bird2.0012.tif | tahoma2d-lib: Tahoma2D repo (GPL-3), library samples | refused | bird2.0012.tif: a single TIFF is not imported; Toonz scenes (.tnz) bring their own image sequences |
| tif | bird2.0016.tif | opentoonz-lib: OpenToonz repo (GPL-3), library samples | refused | bird2.0016.tif: a single TIFF is not imported; Toonz scenes (.tnz) bring their own image sequences |
| tif | 05_bush2.tif | opentoonz-sample: OpenToonz sample scenes, CC BY-NC 4.0 | refused | 05_bush2.tif: a single TIFF is not imported; Toonz scenes (.tnz) bring their own image sequences |
| tif | dragonfly.0070.tif | tahoma2d-lib: Tahoma2D repo (GPL-3), library samples | refused | dragonfly.0070.tif: a single TIFF is not imported; Toonz scenes (.tnz) bring their own image sequences |
| tif | scrawl.0001.tif | opentoonz-lib: OpenToonz repo (GPL-3), library samples | refused | scrawl.0001.tif: a single TIFF is not imported; Toonz scenes (.tnz) bring their own image sequences |
| tif | A.0005.tif | opentoonz-sample: OpenToonz sample scenes, CC BY-NC 4.0 | refused | A.0005.tif: a single TIFF is not imported; Toonz scenes (.tnz) bring their own image sequences |
| tif | star.0005.tif | tahoma2d-lib: Tahoma2D repo (GPL-3), library samples | refused | star.0005.tif: a single TIFF is not imported; Toonz scenes (.tnz) bring their own image sequences |
| tif | coalstroke.0001.tif | opentoonz-lib: OpenToonz repo (GPL-3), library samples | refused | coalstroke.0001.tif: a single TIFF is not imported; Toonz scenes (.tnz) bring their own image sequences |
| tif | 04_bush1.tif | opentoonz-sample: OpenToonz sample scenes, CC BY-NC 4.0 | refused | 04_bush1.tif: a single TIFF is not imported; Toonz scenes (.tnz) bring their own image sequences |
| flipaclip | p.fc | flipaclip: BFDI FlipaClip zip, see its source page | partial | 0 unsupported, 1 warnings |
| flipaclip | p.fc | flipaclip: BFDI FlipaClip zip, see its source page | partial | 0 unsupported, 1 warnings |
| wick | reference-isKeyJustPressed.wick | wick-examples: Wick Editor examples, see the repo | refused | not an XFL document: DOMDocument.xml was not found |
| wick | reference-keypressed.wick | wick-samples: Wick samples, see the folder | refused | not an XFL document: DOMDocument.xml was not found |
| wick | reference-random.choice.wick | wick-examples: Wick Editor examples, see the repo | refused | not an XFL document: DOMDocument.xml was not found |
| wick | reference-mousereleased.wick | wick-samples: Wick samples, see the folder | refused | not an XFL document: DOMDocument.xml was not found |
| wick | reference-Keydown-script.wick | wick-examples: Wick Editor examples, see the repo | refused | not an XFL document: DOMDocument.xml was not found |
| wick | reference-mouseY.wick | wick-samples: Wick samples, see the folder | refused | not an XFL document: DOMDocument.xml was not found |
| wick | reference-ifhitTest.wick | wick-examples: Wick Editor examples, see the repo | refused | not an XFL document: DOMDocument.xml was not found |
| wick | reference-gotoAndPlay.wick | wick-samples: Wick samples, see the folder | refused | not an XFL document: DOMDocument.xml was not found |
| wick | reference-random.float.wick | wick-examples: Wick Editor examples, see the repo | refused | not an XFL document: DOMDocument.xml was not found |
| wick | reference-update.wick | wick-samples: Wick samples, see the folder | refused | not an XFL document: DOMDocument.xml was not found |
| wick | reference-project.height.wick | wick-examples: Wick Editor examples, see the repo | refused | not an XFL document: DOMDocument.xml was not found |
| wick | reference-Mousehover-script.wick | wick-samples: Wick samples, see the folder | refused | not an XFL document: DOMDocument.xml was not found |
| wick | reference-mouseclick.wick | wick-examples: Wick Editor examples, see the repo | refused | not an XFL document: DOMDocument.xml was not found |
| wick | reference-opacity.wick | wick-samples: Wick samples, see the folder | refused | not an XFL document: DOMDocument.xml was not found |
| wick | reference-mouseY.wick | wick-examples: Wick Editor examples, see the repo | refused | not an XFL document: DOMDocument.xml was not found |
| wick | reference-parent.wick | wick-samples: Wick samples, see the folder | refused | not an XFL document: DOMDocument.xml was not found |
| wick | reference-gotoAndStop.wick | wick-examples: Wick Editor examples, see the repo | refused | not an XFL document: DOMDocument.xml was not found |
| wick | reference-stopAllSounds.wick | wick-samples: Wick samples, see the folder | refused | not an XFL document: DOMDocument.xml was not found |
| wick | DJSpookyman.wick | wick-examples: Wick Editor examples, see the repo | refused | not an XFL document: DOMDocument.xml was not found |
| wick | reference-project.width.wick | wick-samples: Wick samples, see the folder | refused | not an XFL document: DOMDocument.xml was not found |
| flipnote | memoC.kwz | flipnote-samples: Flipnote samples, see the folder | partial | 1 unsupported, 0 warnings |
| flipnote | keke.ppm | flipnote.js: flipnote.js test samples, see the repo | partial | 1 unsupported, 0 warnings |
| flipnote | testorig.ppm | opentoonz-lib: OpenToonz repo (GPL-3), library samples | refused | testorig.ppm: not a Flipnote Studio PPM file (the magic is not PARA) |
| flipnote | rgb-3c-8b.ppm | tahoma2d-lib: Tahoma2D repo (GPL-3), library samples | refused | rgb-3c-8b.ppm: not a Flipnote Studio PPM file (the magic is not PARA) |
| flipnote | memoA.kwz | flipnote-samples: Flipnote samples, see the folder | partial | 1 unsupported, 0 warnings |
| flipnote | keke2.ppm | flipnote.js: flipnote.js test samples, see the repo | partial | 1 unsupported, 0 warnings |
| flipnote | rgb-3c-8b.ppm | opentoonz-lib: OpenToonz repo (GPL-3), library samples | refused | rgb-3c-8b.ppm: not a Flipnote Studio PPM file (the magic is not PARA) |
| flipnote | testorig.ppm | tahoma2d-lib: Tahoma2D repo (GPL-3), library samples | refused | testorig.ppm: not a Flipnote Studio PPM file (the magic is not PARA) |
| flipnote | memoD.kwz | flipnote-samples: Flipnote samples, see the folder | partial | 1 unsupported, 0 warnings |
| flipnote | memoG.kwz | flipnote.js: flipnote.js test samples, see the repo | partial | 1 unsupported, 0 warnings |
| flipnote | rgb-3c-16b.ppm | tahoma2d-lib: Tahoma2D repo (GPL-3), library samples | refused | rgb-3c-16b.ppm: not a Flipnote Studio PPM file (the magic is not PARA) |
| flipnote | pekira_beach.ppm | flipnote-samples: Flipnote samples, see the folder | partial | 1 unsupported, 0 warnings |
| flipnote | memoE.kwz | flipnote.js: flipnote.js test samples, see the repo | partial | 1 unsupported, 0 warnings |
| flipnote | memoB.kwz | flipnote-samples: Flipnote samples, see the folder | partial | 1 unsupported, 0 warnings |
| flipnote | memoC.kwz | flipnote.js: flipnote.js test samples, see the repo | partial | 1 unsupported, 0 warnings |
| flipnote | comment.kwc | flipnote-samples: Flipnote samples, see the folder | ok | info and still export ok |
| flipnote | comment.kwc | flipnote.js: flipnote.js test samples, see the repo | ok | info and still export ok |
| flipnote | keke.ppm | flipnote-samples: Flipnote samples, see the folder | partial | 1 unsupported, 0 warnings |
| flipnote | memoA.kwz | flipnote.js: flipnote.js test samples, see the repo | partial | 1 unsupported, 0 warnings |
| flipnote | sample6.kwz | flipnote-samples: Flipnote samples, see the folder | partial | 1 unsupported, 0 warnings |
| png | arrows.png | flash-samples: mixed GitHub samples, see each repo | ok | info and still export ok |
| png | preview_rgba_0_1.png | flipaclip: BFDI FlipaClip zip, see its source page | ok | info and still export ok |
| png | ghbanner@2x.png | flipnote.js: flipnote.js test samples, see the repo | ok | info and still export ok |
| png | HandL.png | godot-moho: godot_moho_importer example, MIT | ok | info and still export ok |
| png | UpperArmR.png | moho-samples: Moho samples, see the folder | ok | info and still export ok |
| png | thick.0010.png | opentoonz-lib: OpenToonz repo (GPL-3), library samples | ok | info and still export ok |
| png | A_colormodel_night.png | opentoonz-sample: OpenToonz sample scenes, CC BY-NC 4.0 | ok | info and still export ok |
| png | output.expected.png | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | ok | info and still export ok |
| png | Hard_Eraser_prev.png | tahoma2d-lib: Tahoma2D repo (GPL-3), library samples | ok | info and still export ok |
| png | framesLarge.png | wick-examples: Wick Editor examples, see the repo | ok | info and still export ok |
| png | c4d36f3e-80a7-4719-b3f1-d14674a76b5f.png | wick-samples: Wick samples, see the folder | ok | info and still export ok |
| png | background.png | flash-samples: mixed GitHub samples, see each repo | ok | info and still export ok |
| png | full_0_1.png | flipaclip: BFDI FlipaClip zip, see its source page | ok | info and still export ok |
| png | logo@2x.png | flipnote.js: flipnote.js test samples, see the repo | ok | info and still export ok |
| png | UpperLegR.png | godot-moho: godot_moho_importer example, MIT | ok | info and still export ok |
| png | Pupil.png | moho-samples: Moho samples, see the folder | ok | info and still export ok |
| png | Ball.0003.png | opentoonz-lib: OpenToonz repo (GPL-3), library samples | ok | info and still export ok |
| png | A_colormodel.png | opentoonz-sample: OpenToonz sample scenes, CC BY-NC 4.0 | ok | info and still export ok |
| png | output.05.right_down.expected.png | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | ok | info and still export ok |
| png | flow3.0004.png | tahoma2d-lib: Tahoma2D repo (GPL-3), library samples | ok | info and still export ok |
| gif | example.gif | godot-moho: godot_moho_importer example, MIT | ok | info and still export ok |
| gif | strike.gif | opentoonz-lib: OpenToonz repo (GPL-3), library samples | ok | info and still export ok |
| gif | target.gif | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | refused | target.gif: malformed GIF header |
| gif | tab_b.gif | tahoma2d-lib: Tahoma2D repo (GPL-3), library samples | ok | info and still export ok |
| gif | back.gif | opentoonz-lib: OpenToonz repo (GPL-3), library samples | ok | info and still export ok |
| gif | target.gif | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | ok | info and still export ok |
| gif | ring.gif | tahoma2d-lib: Tahoma2D repo (GPL-3), library samples | ok | info and still export ok |
| gif | background.gif | opentoonz-lib: OpenToonz repo (GPL-3), library samples | ok | info and still export ok |
| gif | target.gif | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | ok | info and still export ok |
| gif | info.gif | tahoma2d-lib: Tahoma2D repo (GPL-3), library samples | ok | info and still export ok |
| gif | titlebar_end.gif | opentoonz-lib: OpenToonz repo (GPL-3), library samples | ok | info and still export ok |
| gif | dave.gif | tahoma2d-lib: Tahoma2D repo (GPL-3), library samples | ok | info and still export ok |
| gif | titlebar.gif | opentoonz-lib: OpenToonz repo (GPL-3), library samples | ok | info and still export ok |
| gif | jim.gif | tahoma2d-lib: Tahoma2D repo (GPL-3), library samples | ok | info and still export ok |
| gif | cramps.gif | opentoonz-lib: OpenToonz repo (GPL-3), library samples | ok | info and still export ok |
| gif | tab_l.gif | tahoma2d-lib: Tahoma2D repo (GPL-3), library samples | ok | info and still export ok |
| gif | ring.gif | opentoonz-lib: OpenToonz repo (GPL-3), library samples | ok | info and still export ok |
| gif | titlebar.gif | tahoma2d-lib: Tahoma2D repo (GPL-3), library samples | ok | info and still export ok |
| gif | tab_r.gif | opentoonz-lib: OpenToonz repo (GPL-3), library samples | ok | info and still export ok |
| gif | cramps.gif | tahoma2d-lib: Tahoma2D repo (GPL-3), library samples | ok | info and still export ok |
| jpg | tree.jpg | flash-samples: mixed GitHub samples, see each repo | refused | tree.jpg: JPEG is not imported; Flare reads PNG, GIF and SVG still images |
| jpg | testing_pr_3.JPG | opentoonz-lib: OpenToonz repo (GPL-3), library samples | refused | testing_pr_3.JPG: JPEG is not imported; Flare reads PNG, GIF and SVG still images |
| jpg | image_height.jpeg | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | refused | not an XFL document: expected an XFL directory, a modern FLA (zip) or an .xfl file |
| jpg | testing_pr_3.JPG | tahoma2d-lib: Tahoma2D repo (GPL-3), library samples | refused | testing_pr_3.JPG: JPEG is not imported; Flare reads PNG, GIF and SVG still images |
| jpg | missing.jpg | wick-examples: Wick Editor examples, see the repo | refused | missing.jpg: JPEG is not imported; Flare reads PNG, GIF and SVG still images |
| jpg | galaxies.jpg | flash-samples: mixed GitHub samples, see each repo | refused | galaxies.jpg: JPEG is not imported; Flare reads PNG, GIF and SVG still images |
| jpg | jello.jpg | opentoonz-lib: OpenToonz repo (GPL-3), library samples | refused | jello.jpg: JPEG is not imported; Flare reads PNG, GIF and SVG still images |
| jpg | cube5.jpg | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | refused | cube5.jpg: JPEG is not imported; Flare reads PNG, GIF and SVG still images |
| jpg | testing_pr_2.JPG | tahoma2d-lib: Tahoma2D repo (GPL-3), library samples | refused | testing_pr_2.JPG: JPEG is not imported; Flare reads PNG, GIF and SVG still images |
| jpg | alf.jpg | flash-samples: mixed GitHub samples, see each repo | refused | alf.jpg: JPEG is not imported; Flare reads PNG, GIF and SVG still images |
| jpg | ogl_sm.jpg | opentoonz-lib: OpenToonz repo (GPL-3), library samples | refused | ogl_sm.jpg: JPEG is not imported; Flare reads PNG, GIF and SVG still images |
| jpg | test.jpg | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | refused | test.jpg: JPEG is not imported; Flare reads PNG, GIF and SVG still images |
| jpg | quad.jpg | tahoma2d-lib: Tahoma2D repo (GPL-3), library samples | refused | quad.jpg: JPEG is not imported; Flare reads PNG, GIF and SVG still images |
| jpg | class.jpg | flash-samples: mixed GitHub samples, see each repo | refused | class.jpg: JPEG is not imported; Flare reads PNG, GIF and SVG still images |
| jpg | testorig.jpg | opentoonz-lib: OpenToonz repo (GPL-3), library samples | refused | testorig.jpg: JPEG is not imported; Flare reads PNG, GIF and SVG still images |
| jpg | image_width.jpeg | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | refused | not an XFL document: expected an XFL directory, a modern FLA (zip) or an .xfl file |
| jpg | cover.jpg | tahoma2d-lib: Tahoma2D repo (GPL-3), library samples | refused | cover.jpg: JPEG is not imported; Flare reads PNG, GIF and SVG still images |
| jpg | high-level.jpg | flash-samples: mixed GitHub samples, see each repo | refused | high-level.jpg: JPEG is not imported; Flare reads PNG, GIF and SVG still images |
| jpg | bali.jpg | opentoonz-lib: OpenToonz repo (GPL-3), library samples | refused | bali.jpg: JPEG is not imported; Flare reads PNG, GIF and SVG still images |
| jpg | cube2.jpg | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | refused | cube2.jpg: JPEG is not imported; Flare reads PNG, GIF and SVG still images |
| svg | fontawesome-webfont.svg | flash-samples: mixed GitHub samples, see each repo | ok | info and still export ok |
| svg | prevFrame.svg | flipnote.js: flipnote.js test samples, see the repo | ok | info and still export ok |
| svg | icon_smart_bone.svg | godot-moho: godot_moho_importer example, MIT | ok | info and still export ok |
| svg | shortcuts.svg | opentoonz-lib: OpenToonz repo (GPL-3), library samples | ok | info and still export ok |
| svg | logo.svg | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | ok | info and still export ok |
| svg | channelmatte.svg | tahoma2d-lib: Tahoma2D repo (GPL-3), library samples | ok | info and still export ok |
| svg | strokewidth-icon.svg | wick-examples: Wick Editor examples, see the repo | ok | info and still export ok |
| svg | glyphicons-halflings-regular.svg | flash-samples: mixed GitHub samples, see each repo | ok | info and still export ok |
| svg | logo_light.svg | flipnote.js: flipnote.js test samples, see the repo | ok | info and still export ok |
| svg | icon_switch_layer.svg | godot-moho: godot_moho_importer example, MIT | ok | info and still export ok |
| svg | x_lock_on.svg | opentoonz-lib: OpenToonz repo (GPL-3), library samples | ok | info and still export ok |
| svg | logo.svg | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | ok | info and still export ok |
| svg | h_chandle_down.svg | tahoma2d-lib: Tahoma2D repo (GPL-3), library samples | ok | info and still export ok |
| svg | gnurl.svg | wick-examples: Wick Editor examples, see the repo | ok | info and still export ok |
| svg | favicon.svg | flash-samples: mixed GitHub samples, see each repo | ok | info and still export ok |
| svg | lastFrame.svg | flipnote.js: flipnote.js test samples, see the repo | ok | info and still export ok |
| svg | gradient-vertical_small.svg | opentoonz-lib: OpenToonz repo (GPL-3), library samples | ok | info and still export ok |
| svg | logo.svg | ruffle-tests: Ruffle test corpus, MIT OR Apache-2.0 | ok | info and still export ok |
| svg | shift_and_trace_edit.svg | tahoma2d-lib: Tahoma2D repo (GPL-3), library samples | ok | info and still export ok |
| svg | mascot-mark.svg | wick-examples: Wick Editor examples, see the repo | ok | info and still export ok |
