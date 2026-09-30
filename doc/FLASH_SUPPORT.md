# Flash / SWF / FLA Native Support in Flare

## Status: fully built-in — no external tools required

All Flash format import is native C++. No Java, no Python, no JPEXS, no FFmpeg
dependency for the import commands themselves (FFmpeg is used only for FLV/F4V
video playback if it is installed).

## Supported formats

| Format | Extension | Support |
|--------|-----------|---------|
| Flash project (XFL-based, CS5+) | `.fla` | Extract with minizip → parse XFLReader |
| Flash project (legacy binary, CS4-) | `.fla` | OLE2 compound document — detected, embedded bitmaps recovered (QImage-validated); full timeline import not yet supported (re-save as CS5+/XFL) |
| XFL project | `.xfl` | Directory or ZIP → parse XFLReader |
| Compiled Flash | `.swf` | Header + embedded bitmap extraction |
| Component library | `.swc` | ZIP + catalog.xml + library.swf bitmaps |
| Flash Video | `.flv` | Header validated; raster level via FFmpeg |
| Flash H.264 video | `.f4v` | ISO BMFF ftyp; raster level via FFmpeg |
| ActionScript source | `.as` | Copied as reference text |
| Animate command script | `.jsfl` | Copied and top-level functions listed; never executed |
| Adobe extension package | `.zxp` / `.mxp` | Safely unpacked for inspection; installer code is never executed |
| Mislabeled SWF | `.ssf` / `.dat` | Content-sniffed and imported as SWF; extension is only a hint |
| Re-zipped FLA | `.zip` | Content-sniffed, trailer repaired if needed, imported as FLA |

## Why not JPEXS?

JPEXS (GPL v3 + Java) is **licence-incompatible** with Flare's BSD licence and
requires an external runtime. The previous implementation used it via Python
scripts; that entire approach has been replaced by native C++.

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
    XFLReader.h/cpp         XFL/FLA parser (document, library, bitmaps)
    SWFAssets.h/cpp         SWF/FLV/F4V headers, SWF tag-stream bitmaps,
                            legacy OLE2/CFBF FLA carving, content sniffing
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
