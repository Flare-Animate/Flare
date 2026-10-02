# Flash / SWF / FLA Native Support in Flare

## Status: fully built-in — no external tools required

All Flash format import is native C++. No Java, no Python, no JPEXS, no FFmpeg
dependency for the import commands themselves (FFmpeg is used only for FLV/F4V
video playback if it is installed).

## Supported formats

The container is identified from the **leading bytes**, not the extension. The
extension is only consulted when the content is unidentifiable, which is why a
SWF named `.ssf` and an FLA re-zipped as `.zip` both import correctly. The
single list of advertised extensions lives in
`FlashAssets::supportedExtensions()`, so the file dialog and the dispatch cannot
drift apart.

### Fully supported — contents become levels or files

| Format | Extension | What is imported |
|--------|-----------|------------------|
| Flash project (XFL-based, CS5+) | `.fla` | Document, library, timeline layers/frames, bitmap instances to levels; binary media; every asset unpacked |
| XFL project | `.xfl` | Directory or ZIP; same as above |
| Compiled Flash | `.swf` | Header metadata, **images to levels**, **sounds to files**, ActionScript 3 to source |
| Component library | `.swc` | ZIP + `catalog.xml` + `library.swf` images and sounds |
| Mislabeled SWF | `.ssf` / `.dat` | Sniffed as SWF |
| Re-zipped FLA | `.zip` | Sniffed, trailer repaired if needed, imported as FLA |
| ActionScript 3 | inside `.swf` | Decompiled to `.as` via the optional `flare-as3` helper |

### Detected, contents partially converted

| Format | Extension | Status |
|--------|-----------|--------|
| Legacy binary FLA (CS4 and earlier) | `.fla` | OLE2/CFBF. Embedded images recovered and validated by decoding them. **Timeline and vector art not converted** |
| Flash Lite project | `.fls` | Sniffed (same containers as `.fla`) |
| Compressed-sound SWF | `.swz` | Sniffed (same FWS/CWS/ZWS header) |
| Keystroke-signed SWF | `.ksk` | Sniffed (same header) |
| Flash Shared Library | `.sol` | Unpacked as ZIP |
| Flash Video | `.flv` | Header validated; copied for reference; raster playback via FFmpeg |
| Flash H.264 video | `.f4v` | ISO BMFF `ftyp`; copied for reference; playback via FFmpeg |
| MPEG-4 video | `.m4v` | Sniffed as ISO BMFF |

### Unpacked for inspection; code is never executed

| Format | Extension |
|--------|-----------|
| Adobe extension package | `.zxp` / `.mxp` |
| Adobe Native Extension | `.ane` |
| Adobe AIR application | `.air` |
| Open Architecture Module | `.oam` |
| ActionScript source / command script | `.as` / `.asc` / `.mxml` / `.jsfl` |
| Legacy libraries, copied as reference | `.rsl` / `.afl` / `.lwf` |

### Known gaps

These are **detected and reported**, never silently dropped. The import dialog
names what a document contains that it could not convert, so an import that
produces no levels is always explained rather than looking like a broken file.

| Content | Where | Status |
|---------|-------|--------|
| Vector shapes, XFL | FLA/XFL `<DOMShape>` `<edges>` | **Decoded.** `common/flash/XFLShape` reads the `edges` attribute to contours and emits SVG. Verified against 491 real shapes from a published FLA, against an independent decoder, coordinate for coordinate. The sibling `cubics` attribute is deliberately ignored: it is an editor hint and on real documents describes a *different* outline |
| Vector shapes, SWF | `DefineShape`/`DefineShape2`/`DefineShape3`/`DefineShape4` | Not yet converted. The record grammar is well specified and quadrant-based, but SWF shape bounds are stroke-inclusive, so a wrong decode is not obvious from the geometry alone |
| Text | `<DOMStaticText>`, `<DOMText>`, `DefineText`/`Text2` | Not converted |
| Embedded fonts | `DefineFont`/`Font2`/`Font3` | Not converted |
| Video items | `<DOMVideoItem>`, `DefineVideoStream` | Not converted |
| Components | `<DOMComponentInstance>` | Not converted |
| Nested sprite timelines | SWF `DefineSprite` + `PlaceObject*` | Counted and reported, not rebuilt |
| ActionScript 1/2 | `DoAction` / `DoInitAction` | Counted and reported. Only AS3 (`DoABC`) is decompiled |
| Binary FLA timeline | Legacy OLE2 `.fla` | Not reconstructed; re-save as CS5+ or XFL |
| Sound codecs | `DefineSound`, `SoundStreamBlock` | MP3 and raw PCM are written as playable files. ADPCM, Nellymoser, Speex and AAC are written under an honest extension (`.adpcm` / `.raw`) because no decoder is bundled |
| Mislabeled SWF | `.ssf` / `.dat` | Content-sniffed and imported as SWF; extension is only a hint |
| Re-zipped FLA | `.zip` | Content-sniffed, trailer repaired if needed, imported as FLA |

## Why not JPEXS?

JPEXS (GPL v3 + Java) is **licence-incompatible** with Flare's BSD licence and
requires an external runtime. The previous implementation used it via Python
scripts; that entire approach has been replaced by native C++.

## Embedded audio

SWF carries audio in two ways, both handled:

- **DefineSound (14)** — a self-contained clip per tag. `DefineSound` 1/2
  (uncompressed), 3 (MP3) and 4 (uncompressed LE) each write one file. MP3 is
  scanned for the first MPEG frame sync before writing, because some encoders
  prepend padding bytes that stop players from opening the file.
- **SoundStreamHead (18/45/89) + SoundStreamBlock (19/60)** — streaming audio,
  split across blocks. Consecutive blocks are concatenated per stream and
  flushed when the stream ends.

Uncompressed 16-bit PCM is wrapped in a canonical 44-byte RIFF/WAVE header so
it opens in any player. ADPCM, Nellymoser, Speex and AAC have no container we
can write without a codec we do not bundle, so their bytes are written under an
honest `.adpcm` / `.raw` extension rather than being dropped or mislabelled as
MP3.

Measured on the 3.5 MB sample SWF: **63 clips extracted, all well-formed.**

## Content census

`FlashAssets::censusSwf()` and `XFLReader`'s `ContentCensus` walk the document
and count what is actually in it: images, sounds, vector shapes, text, fonts,
video, components, nested timelines, and both ActionScript generations. The
import dialog prints the counts it cannot convert.

This exists because the previous behaviour was the worst kind: a vector-only FLA
imported as a completely empty scene and reported "import complete" with no
explanation, which is indistinguishable from a file Flare failed to read. The
same FLA now reports, for example, "491 vector shape(s) not converted".

### Tag-code dispatch is tested, because a wrong code is silent

The census and the bitmap extractor dispatch on SWF tag codes separately, and
getting one wrong produces no error: the JPEG branch probes the payload and
`continue`s when it does not decode, and the lossless branch rejects a format
byte outside 3/4/5. A transposed tag family therefore writes no file and reports
success.

`flash_reader_tests` pins every code it dispatches on — 2/22/32/46/83 shapes,
6/20/21/35/36/90 bitmaps, 10/48/75 fonts, 60/62 video — against the
specification's tag table and against `flare/sources/common/flash/Macromedia.h`,
and `test_jpeg3_extraction_by_tag_code` builds a real `DefineBitsJPEG3` and
`DefineBitsJPEG4` and asserts each yields a decodable file. The four controls in
`mutation_check.py` that reintroduce the original transpositions must all be
caught, or the test is not covering them.

This was added after a merge brought a corrected copy of `SWFAssets.cpp` into
view, at which point it turned out the JPEG and lossless families had been
transposed, tag 24 (Protect) was being counted as a font while 48
(DefineFont2) was not, and video was counted on 81/93 — codes that belong to
`DefineSceneAndFrameLabelData` and `DefineScalingGrid`, so that tally was never
reachable at all.

## ZIP trailer repair

A number of real FLAs carry an end-of-central-directory record whose size and
offset fields do not describe the central directory actually in the file. Adobe
emits a duplicate local `mimetype` header and then computes the trailer from a
stale entry count; on the sample used for testing, `cd_size` was overstated by
exactly one `mimetype` record (54 bytes) while the central directory itself was
intact.

minizip rejects such an archive outright — `unzReadEndOfCentralDirRecord()`
bails with `UNZ_BADZIPFILE` when `eocdOffset < cdOffset + cdSize` — and so does
Python's `zipfile`. That is what produced the "invalid/corrupt ZIP" error users
reported on perfectly valid files (issue #70).

`common/flash/ZipArchive.cpp` locates the real central directory and re-emits a
corrected trailer, copying the payload verbatim. A recovered directory is only
accepted when a full walk of its records lands exactly on the existing EOCD
*and* every record's back-pointer resolves to a real local file header, so a
coincidental `PK\x01\x02` inside compressed data cannot be mistaken for one.
ZIP64 archives are left alone; minizip already handles those.

## SWF bitmap extraction

Tag codes per the Adobe SWF specification, cross-checked against Ruffle's
`swf/src/tag_code.rs` (MIT/Apache 2.0):

| Tag | Code | Extraction |
|-----|------|-----------|
| DefineBits | 6 | JPEG with global JpegTables |
| JpegTables | 8 | Global JPEG header table |
| DefineBitsLossless | 20 | zlib; 8-bit indexed / RGB555 / 24-bit RGB |
| DefineBitsJPEG2 | 21 | Self-contained JPEG → `.jpg` |
| DefineBitsJPEG3 | 22 | JPEG + zlib alpha → composited to `.png` |
| DefineBitsJPEG4 | 23 | As 22, plus a deblocking parameter |
| DefineBitsJPEG5 | 24 | As 22 |
| DefineBitsLossless2 | 35 | zlib; 8-bit palettized or 32-bit premultiplied ARGB |
| DefineBitsLossless3 | 36 | Same layout as 35 |
| DefineBitsLossless4 | 90 | Same layout as 35 |

Rows of a lossless payload are padded out to a multiple of 4 **bytes**, so the
stride depends on the pixel size: 1- and 3-byte-per-pixel rows are padded,
2- and 4-byte rows already are. Getting this wrong truncates every image whose
width is not a multiple of 4.

CWS (zlib-compressed SWF, version 6+) bodies are decompressed with `qUncompress`
before the tag scan.

Measured on a 3.5 MB SWF named `.ssf`: 919 embedded bitmaps recovered, all
decodable.

## ActionScript

AS3 lives in AVM2 bytecode inside DoABC/DoABC2 tags. Decompiling it needs a real
AVM2 decompiler, so Flare treats it as an *optional* sidecar: the Next2Flash
`as3_decompiler` package is vendored under
`tools/flash/next2flash/vendor/as3_decompiler/` (MIT) and driven through
`common/flash/As3Bridge.{h,cpp}`.

`As3Bridge::isAvailable()` is probed once and cached, exactly the way FFmpeg is
detected. When the helper or a Python interpreter is absent, the import dialog
says so and everything else — FLA/XFL/SWF bitmaps, the timeline — works
unchanged with no Python at all. See
[`NEXT2FLASH_INTEGRATION.md`](./NEXT2FLASH_INTEGRATION.md).

## SWC component libraries (Apache Flex SDK format)

SWC files are ZIP archives containing:
- `catalog.xml` — component/symbol manifest (`swccatalog/9` schema, Apache Flex SDK)
- `library.swf`  — compiled SWF with all embedded assets

Flare parses `catalog.xml` to list exported components, then runs the bitmap
extractor on `library.swf`.

## Architecture

```
flare/sources/common/flash/
    tflash.h/cpp            TFlash SWF writer / renderer
    XFLReader.h/cpp         XFL/FLA parser (document, library, bitmaps,
                            content census)
    SWFAssets.h/cpp         SWF/FLV/F4V headers, SWF tag-stream bitmaps and
                            audio, content census, legacy OLE2/CFBF FLA
                            carving, container sniffing, extension list
    ZipArchive.h/cpp        ZIP trailer repair + hardened extraction
    As3Bridge.h/cpp         optional client for the flare-as3 helper
    FSWFStream.h/cpp        SWF binary stream
    FDT*.h/cpp              Flash data-type tags
    FCT.h/cpp               Character tables
    FAction.h/cpp           ActionScript stubs

flare/sources/flare/
    flashimport.cpp         MI_ImportFlashVector: dialog, format dispatch,
                            scene import, SWC catalog.xml parsing

flare/sources/image/tiio.cpp
                            FLV + F4V declared as RASTER_LEVEL;
                            TLevelReaderFFmpeg registered when FFmpeg present

thirdparty/zlib-1.2.8/contrib/minizip/
    unzip.c, ioapi.c        ZIP/FLA/SWC extraction (compiled into Flare)
```

## SWF export

Flare can write SWF output via `TFlash` (built-in, no external tools):

```cpp
TFlash flash(width, height, frameCount, frameRate, props);
flash.setBackgroundColor(bgColor);
flash.beginFrame(idx);
// … draw …
flash.endFrame(isLast, frameCount, lastScene);
flash.writeMovie(fp);
```

## Format references used (not bundled, not required)

| Project | What we reference | Licence |
|---------|-------------------|---------|
| [Ruffle](https://github.com/ruffle-rs/ruffle) | SWF tag codes, RECT bit layout, bitmap tag binary format | MIT / Apache 2.0 |
| [Apache Flex SDK](https://github.com/apache/flex-sdk) | SWC `catalog.xml` schema | Apache 2.0 |
| [lifeart/fla-viewer](https://github.com/lifeart/fla-viewer) | XFL DOMDocument.xml structure | MIT |
| FLV / ISO BMFF public spec | FLV 9-byte header, `ftyp` box layout | Public spec |

## Roadmap: Next2Flash merge

The Flare-Animate org is consolidating its Flash tooling by merging
[Next2Flash](https://github.com/SSF2-Mods-Official/Next2Flash) (an MIT-licensed
SWF round-trip editor with a native AS3 decompiler) into Flare. Because the two
projects use different stacks (C++/Qt vs Python/JS/Electron), the merge ports the
native-friendly pieces and bridges AS3 as an optional helper. See
[`NEXT2FLASH_INTEGRATION.md`](./NEXT2FLASH_INTEGRATION.md) for the full plan.
