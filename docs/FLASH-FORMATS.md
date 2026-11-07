# Flash formats: what Flare reads, and how that was checked

What Flare imports from the legacy Flash family, which samples were run through it, and what is
still missing. The surfaces (desktop, browser, `flare`, `flare-mcp`) are in
[SURFACES.md](SURFACES.md); the reference sources and their licences are in
[BACKLOG.md](BACKLOG.md).

Checked on 2026-11-05 with the `flare` command line (`flare import <file> <out.flare>`), against
the Ruffle test corpus vendored at `third_party/ruffle` (MIT OR Apache-2.0). Nothing from that
corpus is copied into this repository; it is read in place.

## Status by format

"Verified" means the reader ran on real files and the result was checked. "Spec only" means the
format is known from notes but no real file was checked by hand. "Not built" means no reader.

| Format | Reader | Status | Checked against |
|---|---|---|---|
| SWF, uncompressed (`FWS`), zlib (`CWS`), LZMA (`ZWS`), any version | `flare-import` `swf/` | Verified for shapes, sprites, placements, names, stage, rate, background. Missing End records repaired (below). Other tags reported, not imported | 5,046 `.swf` files; see results |
| SPL (a SWF with a splash extension) | the SWF reader (the bytes decide) | Verified on one file | 1 `.spl` file |
| Modern FLA (a ZIP of XFL parts, CS5 and later) | `flare-import` `xfl/`, `source.rs` | Verified: every zipped `.fla` in the corpus is read without a crash; many report unsupported items | 1,256 zipped `.fla` files |
| XFL folder and `.xfl` stub | `flare-import` `xfl/` | Verified by the existing `xfl*` tests | the XFL test suite (not the corpus) |
| XFL motion tweens on instances, classic (`motion`) and motion object (`motion object`) | `flare-import` `xfl/timeline.rs`, `xfl/motion/` | Classic: a linear transform tween with the frame's `acceleration`. Motion object: the `AnimationCore` curves of X, Y, rotation, skew X and scale are sampled into one keyframe per frame (`tests/xfl_motion.rs`), so the motion follows the file's handles. Time maps, skew Y, colour and filter curves are reported, and a tween over 4096 frames stays linear | one real sample, Flashbang's `audio/sound2.fla` (see the motion section below) |
| XFL library bitmaps (PNG and baseline JPEG) and `DOMBitmapInstance`, read from `bin/` through `bitmapDataHRef` | `flare-import` `xfl/entry.rs`, `xfl/elements.rs`, `bitmap.rs` | Verified on synthetic fixtures (`tests/xfl_bitmaps.rs`) and on Flashbang's real FLAs (below). Animate's own lossless payload (`bin/M *.dat`, starts `03 05`) is decoded (`flash_bitmap.rs`; layout below). 8-bit palette payloads and CMYK JPEG are not imported | none of the Ruffle corpus FLA zips has bitmap items |
| Legacy binary FLA (Flash 5 to CS4, a compound file with a `Contents` stream) | `flare-import` `legacy/` | Partly verified: stage, frame rate, background, library names and types are read; drawings, timelines, text, media, scripts are not. Imported as a skeleton with a warning | 159 compound files in the corpus; `btnstrob.fla` (a real MX 2004 file, from the lifeart reference's fixtures) |
| FLA `Contents` layouts other than the ones in `legacy/layout.rs` | none | Refused with the stream names and the format number | 5 synthetic fixtures from the lifeart reference (see below) |
| SOL (AMF0 local shared object) | none | Not built | 7 `.sol` files in the corpus, not run |
| `.as`, `.asc` (ActionScript text) | none | Not built; they are plain text and would be kept as source data | 4,728 `.as` files in the corpus, not run |
| SWC and SWF templates (`.swc`, `.spa`) | none | Not built | none |
| FLV (Flash Video) | none | Not built; `flare-video` does not read it | 24 `.flv` files in the corpus, not run |
| JSFL | none | Not built (read only was the ask; no reader yet) | 1 `.jsfl` file in the corpus, not run |

## The SWF repair pass

Flash Player plays some movies whose tag blocks have no End record, either the movie's own or a
sprite's. The `swf` crate, which Flare uses, requires the record, so these movies failed.
`flare-import/src/swf/repair.rs` walks the tag stream once, with its own stack, and:

- adds the missing End record to a block, and sets the length of a sprite that gained one;
- drops a single stray byte at the end of a block, as Flash Player does;
- refuses a sprite nested more than 32 levels deep. The `swf` reader recurses once per level, and
  a file of nested sprite headers would run out of a 1 MiB stack. This is a bound on a known
  recursion, not a fix for the Windows report in issue #117 (see below);
- refuses a tag that runs past its block, as the reader would.

When a movie needed an End record, the import says so with an Info note. Before this pass, 110
corpus movies failed with "unexpected end of file" in a sprite or at the top level. 109 of them
now import; the one left is in the gap list below.

## Results on the Ruffle corpus

Counts are from `flare import` on each file, one process per file, 30 s limit each. The run was
done twice; the table is the second run, after the repair pass and the non-movie check.

| Samples | Imported | Refused |
|---|---|---|
| 5,046 `.swf` | 5,027 | 19 |
| 1,415 `.fla` (1,256 zipped XFL, 159 legacy binary) | 1,415 | 0 |
| 1 `.spl` | 1 | 0 |
| **6,462** | **6,443** | **19** |

Before the repair pass the same run imported 6,335 and refused 127. Of the 127, 110 were the End
record cases above and 17 were other refusals.

The 19 refusals, by reason:

- **Refused correctly (10):** files that Ruffle's suite marks as broken on purpose: a zero-length
  file, a truncated tag past the end of the file, an LZMA length that claims too much or too
  little, two movies whose tag counts ask for memory they should not get, and four files that
  are not Flash at all (a PNG and similar, named `.swf`).
- **Refused, a gap (9):** valid-looking movies Flare does not accept yet:
  - a stage 0 or negative pixels wide or high (5 movies: `goto_rewind_movieclip_replace`,
    `scopes_dont_cache/order-1`, `order-2`, `slot_disp_id_shared_numbering`,
    `tags_after_last_showframe`). Flash Player would still play these. The right fallback is not
    decided, so they stay refused with the reason;
  - a frame rate of 0 or negative (2 movies: `stage_framerate_zero`, `stage_framerate_negative`);
  - `PlaceObject2` with a key-press clip action and no key code
    (`timeline/clip_action_no_key_code`): the `swf` crate rejects it;
  - a sprite that still fails with "unexpected end of file" after the repair pass
    (`visual/static_text_offstage`). Not investigated yet.

## What the import leaves out (counts from the same run)

Counted over the 6,319 corpus movies and FLA files that imported with at least one note. The
note is the importer's own text, so the list is what Flare says about each file, not a claim of
its totals. The counts are from the build before the repair pass; the repair adds one Info note
per movie that needed it.

| Note (summarised) | Files |
|---|---|
| Scripts (ActionScript, AS3 `DoABC`) not imported | 5,134 |
| Frame labels and scenes not imported | 2,846 |
| Frame scripts not imported | 2,047 |
| Source data Flare cannot edit, kept and written back on export | 1,256 |
| Movie clip keeps its type, its clock and scripts not carried out | 968 |
| Text (static, dynamic, input) not imported | about 800 |
| Buttons not imported (type kept) | 174 |
| Pictures and bitmap instances not imported | about 120 |
| Sound not imported | 51 |
| Gradients: first stop taken; radial as the average of the ends | 120 and 38 |
| Masks: the mask left out (27); mask layers skipped (27); blend modes (50); filters (105 instance, 36 other); colour effects (102 not imported, 182 instance effects ignored) | see left |
| Shape morphs not imported | 17 |
| Legacy binary FLA: skeleton only | 159 |

## Known open issue: #117 (Windows crash importing FLA and SWF)

Issue #117 reports a crash about 1% into an import of a small FLA, on Windows 10, and says a
Flare project file crashes the same way. The recursion the import has is bounded now (above), but
that does not explain the report: a project file does not go through the Flash reader. The
desktop app loads a file on its UI thread (`flare-runtime` `document_file/load.rs`, called from
`flare-animate` `open_any`), whose main thread has 1 MiB on Windows, while the command line runs
its work on a 16 MiB thread (`apps/flare-cli/src/main.rs`). Not done here: moving the desktop load
onto a worker thread. It needs a result to be polled by the UI, and it could not be checked on
Windows from this environment. The next step is to build the desktop app on Windows with a
debug stack trace, or to move the load off the UI thread and test that.

## Issue #47 (BFDI `.fla`)

Not checked: no BFDI file is in this environment, and the reference samples do not include one.
The legacy reader gives a skeleton for any legacy file, so a BFDI file would import as a skeleton.

## Reference sources

- [fla-decoder](https://github.com/eddiemoore/fla-decoder) (MIT): its `docs/FORMAT.md` is the
  binary FLA notes. Read for facts. No code is ported yet.
- [fla-viewer](https://github.com/lifeart/fla-viewer) (ISC per `package.json`, no licence file):
  facts only. Its `src/__tests__/fixtures` has six FLA files. `btnstrob.fla` is a real MX 2004
  file, according to its `docs/binary-fla-fixes.md`. The others are generated by its `scripts/`
  (`gen-binary-fla-*.mjs`). They are used locally and not committed. Their document-settings
  streams do not match the layouts in `legacy/layout.rs`, so they are refused (binary-layers,
  binary-mx2004, binary-timeline, binary-v4, binary-difat). `easing-tokens.fla` is a ZIP and
  imports.
- Ruffle's `swf` crate (MIT OR Apache-2.0): the SWF parser, a dependency.
- Not fetched in this session: JPEXS (GPLv3, facts only if ever), openfl, flash-fla-extractor and
  the other parsers a web search would find. No leaked Adobe specification was used.

## Next steps, in order

1. Legacy binary FLA: read the timeline and drawings of the MX and CS4 layout, starting with
   `btnstrob.fla`, which is the one real file. Add more real files before writing more code.
2. The stage and frame-rate gaps above: decide the fallback with the maintainer, then test it.
3. Desktop load on a worker thread for #117.
4. SOL, `.as`, SWC, FLV: each needs its own reader and a sample set first.

## Real files found on the web (2026-11-06)

The XFL projects were found by hand and fetched from raw public URLs. Licences are as stated in each
project's LICENSE file (the header was read); each sample is read in place and never committed.

| Source | Licence (checked) | Files | `flare import` result |
|---|---|---|---|
| Ruffle test corpus (`third_party/ruffle`) | see `docs/BACKLOG.md` | 6,462: 5,046 SWF, 1,415 FLA, 1 SPL | 6,443 ok, 19 refused (as above). All 1,416 FLA/SPL import |
| Flashbang (`cure53/Flashbang`, `shumway/` examples and tests) | MPL-2.0 (`LICENSE`) | 77 FLA (zipped CS-era files), 160 SWF, 6 XFL folders | 77 FLA ok, 159 SWF ok, 1 refused (`shumway/examples/clipping/mask.swf`: frame rate "-125" out of range), 6 XFL ok |
| OStimNG `flash/OstimBar` (`VersuchDrei/OStimNG`) | GNU GPL (LICENSE header) | 1 XFL project (`DOMDocument.xml` and 1 library file) | ok after the `/` fix. Before it, 2 edges were skipped with "bad number" |
| CrazyFlashNight `scripts/WarlordDogHost` (`FlashNightModReborn/CrazyFlashNight`) | GNU GPL (LICENSE header) | 1 partial XFL project | ok. One instance refers to a symbol in a library file this project does not include ("missing symbol", accurate) |

GPL samples are test inputs only; nothing from them is committed.

What the real files exposed, and what was fixed:

- **Animate's lossless bitmap payload (`bin/M *.dat`)**, worked out from real files (no licensed
  code used). Little-endian throughout: `03 05`; row length in bytes, width, height (u16 each);
  frame in twips, left, right, top, bottom (i32 each, 20 twips a pixel); at offset 24 a has-alpha
  byte, at 25 a compressed byte; the data from offset 26. Compressed, the data is one zlib stream
  (`78 01` header and Adler-32 included) cut into chunks that each start with their u16 length,
  ended by a chunk of length 0; inflated it is exactly `row length * height` bytes. Pixels are
  premultiplied **ARGB** bytes (alpha first); with the alpha byte 0 the pixels are opaque. Checked
  on 67 `.dat` payloads from the Flashbang FLAs: all 53 with the signature are 32-bit rows and
  inflate to exactly the header size (49 with alpha, 4 without), and several decode to sensible
  pictures (a photo, a Firefox logo with soft shadow, button skins). The rest are not bitmaps in
  this format (empty files, JPEG, headerless audio). Not verified: an uncompressed payload (the
  compressed byte 0) was never seen, and is read as raw rows after offset 26; a payload whose
  row length is not 4 * width (presumably 8-bit palette) was never seen and is reported as not
  imported.

- **Bitmap pixels live in `bin/`**: Animate writes `bitmapDataHRef="M 1 ....dat"`, with the file
  at `bin/M 1 ....dat`, not `LIBRARY/<href>`. Those reported "image file not found" (47 items).
  Now read from `bin/`; PNG, JPEG and Animate's own payload (below) decode.
- **Edges with `/` as a line**: `!209 -235/-33 -1` (OStimNG) was a bad number. `/` is a line now.
- **Motion object tweens** (sampled per frame since 2026-11-07, see below): `tweenType="motion object"` on instances was imported as a linear
  tween (the curves were reported as not imported; one real file has them, see below).

Still missing, from the same files:

- **Frame scripts** (178 FLAs carry them), **movie clip clocks**, **frame labels**, **text**
  (`DOMStaticText`, `DOMDynamicText`), **radial and multi-stop gradients**, **instance colour
  effects** (`instance color effects are ignored`, 10 FLAs), **filters**, **blend modes**, **sound**.

## Motion object curves (2026-11-07)

Read from the `AnimationCore` of a `motionObjectXML` (CS4 and later). Behaviour was checked
against the file layout described in the reference listed under "Reference sources"; the code
is Flare's own. What is done:

- Each frame of the tween becomes one keyframe, at the matrix the curves give there. The value
  of a channel at frame `n` is `n * TimeScale / frame rate` ticks into its keys, so the
  file's own Bezier handles (in ticks) shape the motion; a key with no handle is a straight line.
- Channels: `Motion_X` and `Motion_Y` (offsets, in pixels), `Rotation_Z` (degrees), `Skew_X`
  (degrees, leaning the second axis), `Scale_X` and `Scale_Y` (percent). Each is relative to its
  value at the first tick, so frame 0 is the instance as it was placed.
- Not imported, each named in the diagnostics: a non-zero time map (the keys keep their own
  timing); `Skew_Y`; colour curves (`Colors`, waiting for instance colour effects); filter
  curves. A key with a value that is not a number is skipped; two keys at the same time make the
  curve unreadable and it is reported.
- A tween over 4096 frames stays linear and says so. The `motionObjectXML` is kept as source
  data, so an export still carries it beside the sampled frames.

- A whole-curve time map (`<TimeMap type strength>`, the Motion Editor's ease) of the Quadratic,
  Cubic, Quartic or Quintic family is applied to the motion's timing. Its direction and blend
  follow the sign and value of the strength (checked against no Animate playback). Other types
  are reported and the curves keep their own timing.
- The rotation and scale turn about the instance's `transformationPoint`, and the offsets move
  that point; with no transformation point the origin is used.

Checked by `tests/xfl_motion.rs` (12 tests, failure paths included). The one real motion object
found is `web/flashbang/shumway/examples/audio/sound2.fla` (Flashbang, MPL-2.0, read in place):
its tween has `Motion_X`, `Motion_Y` and `Rotation_Z` keys in ticks (TimeScale 24000, 79 frames),
a `Quadratic` time map of strength 10 on each property, and a transformation point of (51.7, 51.7).
That file is a zip with a damaged central directory, which Flare reads through its own path;
the `Skew_Y` curve is reported as not imported. No other sample has a motion object.

## Import before and after (2026-11-07)

`flare import` on 83 sample files: the Flashbang `.fla` and `.xfl` files, the OStimNG and
CrazyFlashNight projects, and the Ruffle corpus's `.fla` files under `third_party/ruffle` (read in
place; the list and counts are in the run log of this session). Before: the build without the
motion code. After: the build with it. Each file's diagnostics were counted by the importer's own
note text.

- 82 files give the same diagnostics as before.
- `sound2.fla` changes: "motion object curves are not imported" is replaced by the sampled tween
  (79 frames, one keyframe each), and the diagnostics name the curves that are still not
  imported (`Skew_Y`). Its other notes are unchanged.
- Refusals: none before or after.
