# Flare Flash Format Import (FLA / XFL / SWF / SWC / FLV / F4V / AS)

Flare includes **built-in, native** import support for all Flash file formats.
No external tools, Java runtimes, Python scripts, or third-party software are
required — everything is compiled directly into the application.

## Supported formats

| Extension | Description | Native handling |
|-----------|-------------|-----------------|
| `.fla`    | Flash/Animate project (ZIP-backed XFL) | ZIP extracted with minizip; DOMDocument.xml parsed |
| `.xfl`    | Uncompressed XFL project (directory or ZIP) | Parsed with XFLReader |
| `.swf`    | Compiled SWF binary | Header decoded; embedded bitmaps extracted via tag scan |
| `.swc`    | SWF component library (ZIP) | ZIP extracted; `catalog.xml` parsed; `library.swf` bitmaps extracted |
| `.flv`    | Flash Video | Header validated; loaded as raster level via FFmpeg |
| `.f4v`    | Flash H.264 video (ISO BMFF / MP4) | `ftyp` box read; loaded as raster level via FFmpeg |
| `.as`     | ActionScript source | Copied as reference text |

## Why not JPEXS / external tools?

JPEXS Free Flash Decompiler is **GPL v3** — incompatible with Flare's BSD
licence — and requires a Java runtime.  The old approach of calling it via
Python scripts was deliberately replaced by this native C++ implementation.
No external tools are needed or used.

## How to import

**File → Import → Import Flash (FLA / XFL / SWF / SWC / FLV / F4V / AS)...**

A file dialog opens; select any supported file. After import Flare:

1. Validates the file signature natively (no external process).
2. Extracts ZIP archives (FLA, SWC) using the bundled minizip library.
3. Parses `DOMDocument.xml` (FLA/XFL) and reports document properties.
4. Scans SWF tag streams and extracts all embedded JPEG and lossless bitmaps.
5. Parses SWC `catalog.xml` (Apache Flex SDK format) to list exported symbols.
6. Decompresses zlib-compressed SWF bodies (CWS, SWF6+) using Qt.
7. Reads FLV / F4V headers and loads video files as raster levels via FFmpeg.
8. Auto-loads extracted bitmap assets into the current scene.
9. Writes `manifest.txt` and optionally opens the export folder.

## Architecture

```
flare/sources/common/flash/
    XFLReader.h/cpp         XFL/FLA parser; ZIP extraction via minizip
    FSWFStream.h/cpp        Low-level SWF binary stream (write path)
    FDT*.h/cpp              Flash data-type tag implementations
    FCT.h/cpp               Flash character tables
    FAction.h/cpp           ActionScript tag stubs
    tflash.h/cpp            TFlash: SWF write / render engine

flare/sources/flare/
    flashimport.cpp         UI commands (zero external dependencies)
                            — readSwfHeader(), extractSwfBitmaps()
                            — readFlvHeader(), readF4vHeader()
                            — SWC catalog.xml parser

flare/sources/image/tiio.cpp
                            FLV + F4V registered as RASTER_LEVEL
                            (reader via TLevelReaderFFmpeg when FFmpeg present)

thirdparty/zlib-1.2.8/contrib/minizip/
    unzip.c, ioapi.c        ZIP extraction compiled into Flare
```

## Format references (not bundled)

| Reference | Used for | License |
|-----------|----------|---------|
| [Ruffle](https://github.com/ruffle-rs/ruffle) `swf/src/tag_code.rs` | SWF tag codes, RECT bit layout, DefineBitsJPEG/Lossless binary format | MIT/Apache 2.0 |
| [Apache Flex SDK](https://github.com/apache/flex-sdk) | SWC `catalog.xml` schema | Apache 2.0 |
| [lifeart/fla-viewer](https://github.com/lifeart/fla-viewer) | XFL DOMDocument.xml schema | MIT |
| FLV/F4V public spec | FLV 9-byte header, ISO BMFF `ftyp` box | Public spec |
