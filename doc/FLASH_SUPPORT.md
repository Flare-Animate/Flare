# Flash/FLA File Format Support in Flare

This document describes Flare's support for Adobe Flash file formats including SWF, FLA, and XFL.

## Overview

Flare provides multiple pathways for working with Flash/Animate content:

1. **SWF Import** - Import SWF animations as raster or vector
2. **FLA/XFL Import** - Import Flash source projects
3. **SWF Export** - Export Flare content to SWF format
4. **XFL Export** - Export to XML-based Flash format (in development)

## File Formats

### SWF (Shockwave Flash)
- **Extension**: `.swf`
- **Type**: Binary compiled Flash movie
- **Support**: Import (via FFmpeg or JPEXS), Export (via TFlash)
- **Use Cases**: Published Flash animations, web content

### FLA (Flash Source)
- **Extension**: `.fla`
- **Type**: Binary or ZIP-based source project
- **Support**: Import via JPEXS decompiler
- **Modern Format**: Since CS5, FLA files are ZIP archives containing XFL

### XFL (XML Flash)
- **Extension**: `.xfl` or uncompressed directory
- **Type**: XML-based source format
- **Support**: Import/Export with xfl_handler.py
- **Advantages**: Human-readable, version-control friendly

## Import Workflows

### Method 1: Import SWF (Raster)
Uses FFmpeg to decode SWF as video frames.

**Requirements:**
- FFmpeg installed and in PATH

**Steps:**
1. File → Load Level
2. Select `.swf` file
3. Flare imports as raster sequence

**Limitations:**
- Vector data is rasterized
- No ActionScript support
- Frame-by-frame only

### Method 2: Import Flash (Vector via External Decompiler)
Uses JPEXS to extract vector content.

**Requirements:**
- JPEXS Free Flash Decompiler (ffdec) installed
- Python 3

**Steps:**
1. File → Import → Import Flash (Vector via External Decompiler)
2. Select `.swf` or `.fla` file
3. Configure decompiler path in Preferences if needed
4. Script runs JPEXS to export SVG/images
5. Import the exported assets into Flare

**Advantages:**
- Preserves vector quality
- Exports shapes, symbols, timelines
- Can handle complex Flash content

**Script:** `tools/flash/decompile_flash.py`

### Method 3: Import Flash Container (Native)
Handles multiple Flash container formats.

**Supported Formats:**
- `.swf` - Decompiled via JPEXS
- `.fla` - Unzipped if XFL-based, otherwise decompiled
- `.xfl` - Parsed directly
- `.swc` - Extracted and decompiled
- `.as` - Copied as reference
<<<<<<< HEAD
- `.jsfl` - Adobe Animate JSFL scripts are copied and (optionally) linted

**Script linting:** `import_container.py` will, by default, perform a syntax check on extracted `.jsfl`, `.js` and `.as` files using the Python `esprima` package if installed. Problems are recorded in the `manifest.json` under the `problems` key and a `script_problems.json` file is also written. Linting may be disabled with `--no-lint-scripts`.

**Steps:**
1. File → Import → Import Flash (Native)
2. Select file
3. Script extracts/decompiles content
4. Review manifest.json for imported files and any script problems
=======

**Steps:**
1. File → Import → Import Flash/XFL/SWC (Native)
2. Select file
3. Script extracts/decompiles content
4. Review manifest.json for imported files
>>>>>>> origin/master

**Script:** `tools/flash/import_container.py`

### Method 4: Direct XFL Import
Parse XFL projects directly.

**Requirements:**
- `xfl_handler.py` utility

**Usage:**
```bash
python tools/flash/xfl_handler.py --read project.xfl
python tools/flash/xfl_handler.py --read /path/to/uncompressed/xfl/
```

**Features:**
- Reads document properties (size, framerate, background)
- Parses library symbols
- Identifies ActionScript exports
- Extracts timeline structure

## Export Workflows

### Method 1: Export to SWF
Uses the built-in TFlash rendering engine.

**Current Status:** ✅ Implemented
**Location:** `flare/sources/common/tvrender/tflash.cpp`

**Features:**
- Vector shape export
- Raster image embedding
- Timeline/frame support
- Gradient fills
- Sound support (basic)

**Steps:**
1. Create animation in Flare
2. File → Export → Export to SWF (when implemented in UI)
3. Configure SWF properties:
   - Line quality (Constant/Mixed/Variable)
   - Compression
   - Autoplay/Looping
   - JPEG quality for embedded images

**API Usage:**
```cpp
TFlash flash(width, height, frameCount, frameRate, properties);
flash.setBackgroundColor(bgColor);
flash.beginFrame(frameIndex);
// ... draw content ...
flash.endFrame(isLast, frameCount, lastScene);
flash.writeMovie(filePointer);
```

### Method 2: Export to XFL
Create XML-based Flash projects.

**Current Status:** 🚧 In Development
**Script:** `tools/flash/xfl_handler.py`

**Planned Features:**
- Generate XFL directory structure
- Export symbols with linkage
- Create timeline XML
- Embed assets (images, sounds)
- Compatible with Adobe Animate

**Usage (Future):**
```bash
python tools/flash/xfl_handler.py --write output.xfl
```

## Architecture

### Flash Library Components

Located in `flare/sources/common/flash/`:

- **FDT.h/cpp** - Define Type base class
- **FCT.h/cpp** - Control Type base class  
- **FObj.h/cpp** - Base Flash object
- **FSWFStream.h/cpp** - SWF binary stream I/O
- **FDTShapes.h/cpp** - Shape definitions
- **FDTSprite.h/cpp** - Sprite/MovieClip support
- **FDTText.h/cpp** - Text field handling
- **FDTFonts.h/cpp** - Font embedding
- **FDTBitmaps.h/cpp** - Bitmap handling
- **FDTSounds.h/cpp** - Audio support
- **FDTButtons.h/cpp** - Button symbols
- **FPrimitive.h/cpp** - Primitive drawing
- **FAction.h/cpp** - ActionScript support
- **tflash.h** - TFlash main interface

### Helper Scripts

Located in `tools/flash/`:

1. **decompile_flash.py**
   - Wrapper for JPEXS decompiler
   - Exports SVG, images from SWF
   - Generates manifest.json

2. **import_container.py**
   - Multi-format container handler
   - Extracts ZIP-based formats
   - Coordinates decompilation
   - Copies ActionScript sources

3. **xfl_handler.py**
   - XFL format reader/writer
   - Parses DOMDocument.xml
   - Generates XFL structure
   - Symbol/library management

## Implementation Details

### XFL Format Structure

```
project.xfl (or project.fla in modern versions)
├── DOMDocument.xml          # Main document properties
├── PublishSettings.xml      # Export settings
└── LIBRARY/
    ├── Symbol1.xml          # Library symbol
    ├── Symbol2.xml
    └── assets/
        ├── image1.png
        └── sound1.mp3
```

### XFL Document Properties

```xml
<DOMDocument 
    width="550" 
    height="400" 
    frameRate="24.0" 
    backgroundColor="#FFFFFF"
    xflVersion="2.97">
```

### Symbol with ActionScript Export

```xml
<DOMSymbolItem 
    name="MySymbol" 
    symbolType="movie clip"
    linkageExportForAS="true"
    linkageClassName="com.example.MySymbol">
```

## Code References

### JPEXS Decompiler
**GitHub:** https://github.com/jindrapetrik/jpexs-decompiler

**Key Concepts Adapted:**
- XFLConverter architecture
- Shape conversion algorithms
- Timeline export logic
- Symbol/library handling
- ActionScript integration

**Java Classes Referenced:**
- `com.jpexs.decompiler.flash.xfl.XFLConverter`
- `com.jpexs.decompiler.flash.types.SHAPE`
- `com.jpexs.decompiler.flash.timeline.Timeline`

### OpenFL SWF Library
**GitHub:** https://github.com/openfl/swf

**Key Concepts Adapted:**
- SWF tag parsing
- MovieClip/Sprite structure
- Symbol/library organization
- Asset optimization strategies

**Haxe Classes Referenced:**
- `swf.exporters.SWFLiteExporter`
- `swf.exporters.AnimateExporter`
- `swf.SWFLibrary`

## Conversion Workflows

### SWF → Flare → SWF (Round-trip)

1. Import SWF via JPEXS:
   ```bash
   python tools/flash/decompile_flash.py --input animation.swf --output extracted/
   ```

2. Edit in Flare
   - Load extracted SVG/images
   - Modify animation
   - Add effects

3. Export to SWF:
   ```cpp
   TFlash flash(width, height, frames, fps, props);
   // ... rendering ...
   flash.writeMovie(fp);
   ```

### FLA → XFL → Flare

1. Extract FLA (if ZIP-based):
   ```bash
   python tools/flash/import_container.py --input project.fla --output extracted/
   ```

2. Parse XFL:
   ```bash
   python tools/flash/xfl_handler.py --read extracted/
   ```

3. Import assets into Flare:
   - Use extracted images/SVG
   - Reference ActionScript for logic
   - Rebuild timeline

### Flare → XFL → Animate

1. Export Flare animation
2. Generate XFL:
   ```bash
   python tools/flash/xfl_handler.py --write output.xfl
   ```

3. Open in Adobe Animate:
   - File → Open → Select output.xfl
   - Continue editing in Animate
   - Publish to SWF/HTML5

## Limitations

### Current Limitations

1. **ActionScript Execution**
   - ActionScript code is imported as reference only
   - No runtime execution in Flare
   - Frame scripts not supported

2. **Advanced Features**
   - Text Layout Framework (TLF) not supported
   - Inverse kinematics not supported
   - Shape tweens require manual conversion
   - Motion tweens require manual conversion

3. **Audio**
   - Basic sound support in SWF export
   - Limited audio sync features
   - No sound envelope editing

4. **Fonts**
   - Embedded fonts supported in SWF export
   - Font rendering may differ from Flash
   - TLF fonts not supported

### Workarounds

**For ActionScript:**
- Document scripts as comments
- Implement logic manually in Flare
- Use external tools for scripted content

**For Tweens:**
- Export tween frames individually
- Use Flare's keyframe animation
- Manual in-betweening if needed

**For TLF Text:**
- Convert to classic text fields
- Render text as vectors/images
- Use external text rendering

## Best Practices

### Importing Flash Content

1. **Use Vector Import When Possible**
   - Preserves quality
   - Allows editing
   - Smaller file sizes

2. **Organize Extracted Assets**
   - Review manifest.json
   - Categorize by type (images, shapes, symbols)
   - Maintain folder structure

3. **Test Decompiler Paths**
   - Set JPEXS path in Preferences
   - Test with simple SWF first
   - Verify export quality

### Exporting to Flash

1. **Optimize for Target**
   - Choose appropriate line quality
   - Compress for web delivery
   - Test in Flash Player

2. **Embed Resources Carefully**
   - JPEG quality affects file size
   - Reuse bitmaps when possible
   - Vectorize when appropriate

3. **Timeline Best Practices**
   - Keep frame structure simple
   - Avoid excessive layers
   - Test playback in Flash

## Future Enhancements

### Planned Features

- [ ] Native XFL export from Flare UI
- [ ] ActionScript code generation
- [ ] Improved shape conversion
- [ ] Motion tween support
- [ ] Enhanced font handling
- [ ] Full TLF support
- [ ] Bones/IK support

### Community Contributions

We welcome contributions to enhance Flash support:

1. **SWF Tag Parsers**
   - Implement missing tag types
   - Improve shape conversion
   - Add advanced features

2. **XFL Generation**
   - Complete XFL writer
   - Add missing XML structures
   - Improve compatibility

3. **ActionScript Support**
   - Add code generation
   - Implement basic VM
   - Support class exports

## References

### Documentation
- Adobe XFL Format Specification
- SWF File Format Specification (v19)
- ActionScript 3.0 Language Reference

### Tools
- JPEXS Free Flash Decompiler: https://github.com/jindrapetrik/jpexs-decompiler
- OpenFL SWF Library: https://github.com/openfl/swf
- FFmpeg: https://ffmpeg.org/

### Related Projects
- as3swf: https://github.com/claus/as3swf
- swftools: http://www.swftools.org/
- Gordon Flash Runtime: https://github.com/tobeytailor/gordon

## Support

For issues or questions:
1. Check GitHub Issues: https://github.com/Flare-Animate/Flare/issues
2. Review documentation: `doc/how_to_import_swf.md`
3. Test with sample files in `tools/flash/tests/`

## License

Flash format support is implemented under Flare's BSD-3-Clause license.
External tools (JPEXS, FFmpeg) retain their own licenses.
