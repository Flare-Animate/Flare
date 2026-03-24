# Flash / SWF / FLA Native Support in Flare

## Status: fully built-in — no external tools required

All Flash format import is native C++. No Java, no Python, no JPEXS, no FFmpeg
dependency for the import commands themselves (FFmpeg is used only for FLV/F4V
video playback if it is installed).

## Supported formats

| Format | Extension | Support |
|--------|-----------|---------|
| Flash project (ZIP) | `.fla` | Extract with minizip → parse XFLReader |
| XFL project | `.xfl` | Directory or ZIP → parse XFLReader |
| Compiled Flash | `.swf` | Header + embedded bitmap extraction |
| Component library | `.swc` | ZIP + catalog.xml + library.swf bitmaps |
| Flash Video | `.flv` | Header validated; raster level via FFmpeg |
| Flash H.264 video | `.f4v` | ISO BMFF ftyp; raster level via FFmpeg |
| ActionScript source | `.as` | Copied as reference text |

## Why not JPEXS?

JPEXS (GPL v3 + Java) is **licence-incompatible** with Flare's BSD licence and
requires an external runtime. The previous implementation used it via Python
scripts; that entire approach has been replaced by native C++.

## SWF bitmap extraction

SWF tag codes (referenced from Ruffle `swf/src/tag_code.rs`, MIT/Apache 2.0):

| Tag | Code | Extraction |
|-----|------|-----------|
| DefineBits | 6 | JPEG with global JpegTables |
| JpegTables | 8 | Global JPEG header table |
| DefineBitsJpeg2 | 21 | Self-contained JPEG → saved as `.jpg` |
| DefineBitsJpeg3 | 35 | JPEG + zlib alpha → JPEG portion saved |
| DefineBitsJpeg4 | 90 | JPEG with deblocking → saved as `.jpg` |
| DefineBitsLossless2 | 36 | zlib ARGB → decompressed via Qt → saved as `.png` |

CWS (zlib-compressed SWF, version 6+) bodies are decompressed with `qUncompress`
before the tag scan.

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
    XFLReader.h/cpp         XFL/FLA parser (minizip-based ZIP extraction)
    FSWFStream.h/cpp        SWF binary stream
    FDT*.h/cpp              Flash data-type tags
    FCT.h/cpp               Character tables
    FAction.h/cpp           ActionScript stubs

flare/sources/flare/
    flashimport.cpp         MI_ImportFlashVector / MI_ImportFlashContainer
                            readSwfHeader(), extractSwfBitmaps()
                            readFlvHeader(), readF4vHeader()
                            SWC catalog.xml parser

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
