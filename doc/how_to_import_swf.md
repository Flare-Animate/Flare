# Flare Flash Format Import (FLA / XFL / SWF / SWC / AS)

Flare includes **built-in, native** import support for all common Flash file
formats. No external tools, decompilers, Python scripts, or third-party
software are required — everything runs inside the application.

## Supported formats

| Extension | Description |
|-----------|-------------|
| `.fla`    | Flash/Animate project (ZIP-backed XFL archive) |
| `.xfl`    | Uncompressed XFL project (directory or single-file ZIP) |
| `.swf`    | Compiled SWF binary |
| `.swc`    | SWF component library (ZIP archive) |
| `.as`     | ActionScript source file |

## How to import

**File → Import → Import Flash (FLA / XFL / SWF / SWC / AS)...**

A standard file dialog opens; pick any supported file.  After import Flare:

1. Extracts ZIP archives (FLA, SWC) using the bundled minizip library.
2. Parses `DOMDocument.xml` (FLA / XFL) and reports document properties.
3. Auto-loads embedded bitmap assets (PNG / JPEG) into the current scene.
4. Writes a `manifest.txt` next to the exported files.
5. Offers to open the export folder.

The unified command is also accessible as
**File → Import → Import Flash Container (FLA / SWC)...** for workflow
compatibility.

## How it works (technical)

All processing is done in native C++, compiled directly into Flare:

| Task | Component |
|------|-----------|
| ZIP extraction (FLA, SWC) | `unzip.c` / `ioapi.c` from `thirdparty/zlib-1.2.8/contrib/minizip` |
| XFL XML parsing | `flare/sources/common/flash/XFLReader.cpp` |
| SWF header decoding | Inline bit-stream reader in `flashimport.cpp` |
| Flash data types | `FDT*.cpp`, `FCT.cpp`, `FSWFStream.cpp` in `flare/sources/common/flash/` |
| SWF writing / rendering | `TFlash` class — `tflash.cpp` |

## ActionScript

ActionScript (`.as`) files are copied into the export folder for reference.
Flare does not execute ActionScript; the files are included so animators can
inspect original scripting logic and re-implement behaviour manually.
