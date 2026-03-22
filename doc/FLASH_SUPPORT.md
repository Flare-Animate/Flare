# Flash / SWF / FLA Native Support in Flare

## Status

All Flash format import is **fully built-in**. No external tools, Java
runtimes, Python scripts, or third-party decompilers are required.

## Supported import formats

| Format | Extension | Notes |
|--------|-----------|-------|
| Flash project (ZIP) | `.fla` | Extracted with minizip, parsed with XFLReader |
| XFL project | `.xfl` | Directory or ZIP; parsed with XFLReader |
| Compiled Flash | `.swf` | Header parsed natively; bitmaps extracted |
| Component library | `.swc` | ZIP extracted with minizip |
| ActionScript source | `.as` | Copied as reference text |

## Architecture

```
flare/sources/common/flash/   ← compiled into tnzcore
    tflash.h / tflash.cpp         TFlash: SWF writing / rendering engine
    XFLReader.h / XFLReader.cpp   XFL/FLA parser (uses minizip internally)
    FSWFStream.h / .cpp           Low-level SWF binary stream
    FDT*.h / .cpp                 Flash data-type tag implementations
    FCT.h / .cpp                  Flash character tables
    FAction.h / .cpp              ActionScript tag stubs
    F3SDK.h, Macromedia.h         SWF constants and SDK types

flare/sources/flare/
    flashimport.cpp               UI commands — zero external dependencies

thirdparty/zlib-1.2.8/contrib/minizip/
    unzip.c, ioapi.c              ZIP extraction, compiled into Flare
```

## SWF Export

Flare can export scenes to SWF via the `TFlash` class:

```cpp
TFlash flash(width, height, frameCount, frameRate, properties);
flash.setBackgroundColor(bgColor);
flash.beginFrame(frameIdx);
// draw content …
flash.endFrame(isLast, frameCount, lastScene);
flash.writeMovie(fp);
```

## Flashlight integration

The [Flashlight](https://github.com/B4uti4github/Flashlight) sister project
ports the same FCT / FDT / XFLReader infrastructure to JavaScript so Flash
content can be converted to open-web formats. Both projects share the same
C++ architecture from `flare/sources/common/flash/`.
