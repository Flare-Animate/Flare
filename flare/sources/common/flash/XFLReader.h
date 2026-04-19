// XFLReader.h - XFL (XML Flash) format reader
// Copyright (c) 2026 Flare Project
//
// Parses Adobe Animate / Flash XFL/FLA project files.
// XFL is the XML-based source format written by Adobe Animate (formerly Flash
// Professional). A FLA file is simply a ZIP archive of an XFL directory tree.
//
// Format knowledge drawn from (ideas only, no code copied):
//   - Adobe XFL specification (public)
//   - jpexs-decompiler (GPL-3.0) — XFLConverter timeline/layer/frame model
//   - fla-viewer (MIT)           — DOMDocument XML attribute names
//   - ruffle (MIT/Apache-2.0)    — SWF tag reference numbers cited in comments
//   - open-flash/swf-bitmap (ISC) — bitmap format byte constants
//   - lightspark (LGPL-3.0)      — CWS/ZWS decompression approach

#ifndef XFLREADER_H_
#define XFLREADER_H_

#include "tcommon.h"
#include "tfilepath.h"
#include <string>
#include <vector>
#include <map>

#undef DVAPI
#undef DVVAR
#ifdef TFLASH_EXPORTS
#define DVAPI DV_EXPORT_API
#define DVVAR DV_EXPORT_VAR
#else
#define DVAPI DV_IMPORT_API
#define DVVAR DV_IMPORT_VAR
#endif

namespace XFL {

// ---------------------------------------------------------------------------
// 2-D affine transform from a <Matrix> element (DOMBitmapInstance etc.)
// a/b/c/d = rotation+scale 2×2; tx/ty = translation in pixels.
// ---------------------------------------------------------------------------
struct Transform {
    double a  = 1.0, b  = 0.0;
    double c  = 0.0, d  = 1.0;
    double tx = 0.0, ty = 0.0;
};

// ---------------------------------------------------------------------------
// A bitmap asset declared in the <media> section of DOMDocument.xml.
// 'name' is the key referenced by DOMBitmapInstance.libraryItemName.
// 'href' is a path relative to the XFL directory root (e.g. "LIBRARY/a.png").
// ---------------------------------------------------------------------------
struct BitmapItem {
    std::string name;
    std::string href;
};

// ---------------------------------------------------------------------------
// A single element placed inside a DOMFrame's <elements>.
// Only bitmap instances are populated from DOMDocument; symbol instances
// are noted but their full timeline lives in a separate LIBRARY/ XML.
// ---------------------------------------------------------------------------
struct FrameElement {
    enum Type { NONE, BITMAP_INSTANCE, SYMBOL_INSTANCE } type = NONE;
    std::string libraryItemName;  // key into Document::bitmaps
    Transform   matrix;
};

// ---------------------------------------------------------------------------
// One "keyframe span" in the XFL timeline.
// index    = first row (0-based)
// duration = how many consecutive rows this span occupies
// tweenType: "" | "motion" | "shape" (classic tweens)
// ---------------------------------------------------------------------------
struct XFLFrame {
    int  index    = 0;
    int  duration = 1;
    bool keyFrame = false;
    std::string name;
    std::string tweenType;  // "motion", "shape", or "" for static
    std::vector<FrameElement> elements;
};

// ---------------------------------------------------------------------------
// One layer in a DOMTimeline.
// layerType: "normal" | "guide" | "mask" | "folder"
// ---------------------------------------------------------------------------
struct XFLLayer {
    std::string name;
    std::string layerType;
    std::vector<XFLFrame> frames;
};

// ---------------------------------------------------------------------------
// A complete timeline (Scene 1 or a symbol's internal timeline).
// ---------------------------------------------------------------------------
struct XFLTimeline {
    std::string name;
    std::vector<XFLLayer> layers;
};

// Symbol types in XFL library
enum SymbolType {
    SYMBOL_GRAPHIC,
    SYMBOL_MOVIECLIP,
    SYMBOL_BUTTON
};

// Represents a symbol in the XFL library
struct Symbol {
    std::string name;
    std::string itemId;
    SymbolType type;
    std::string linkageClass;  // ActionScript class name if exported
    bool linkageExport;         // True if exported for ActionScript
    
    Symbol() : type(SYMBOL_GRAPHIC), linkageExport(false) {}
};

// Represents the main XFL document properties
struct Document {
    int width;
    int height;
    double frameRate;
    std::string backgroundColor;
    std::vector<Symbol>      symbols;   // from LIBRARY/*.xml
    std::vector<BitmapItem>  bitmaps;   // from <media> section
    std::vector<XFLTimeline> timelines; // from <timelines> section

    Document() : width(550), height(400), frameRate(24.0), backgroundColor("#FFFFFF") {}
};

// XFL Reader class - parses XFL format files
class DVAPI Reader {
public:
    // Constructor
    // @param xflPath: Path to .xfl/.fla file (ZIP) or XFL directory
    Reader(const TFilePath &xflPath);
    ~Reader();
    
    // Read and parse the XFL structure
    // @return: true if successful, false otherwise
    bool read();
    
    // Get the parsed document
    const Document& getDocument() const { return m_document; }
    
    // Check if file is a ZIP-based XFL/FLA
    bool isZipBased() const { return m_isZip; }
    
    // Get error message if read() failed
    std::string getError() const { return m_error; }
    
    // Extract a ZIP-based FLA to a temporary directory
    // @param outputDir: Where to extract (empty = create temp dir)
    // @return: Path to extracted directory, or empty on error
    TFilePath extractZip(const TFilePath &outputDir = TFilePath());

private:
    TFilePath m_xflPath;
    bool m_isZip;
    Document m_document;
    std::string m_error;
    
    // Internal parsing methods
    bool readFromZip();
    bool readFromDirectory();
    bool parseDOMDocument(const std::string &xmlContent);
    bool parseSymbol(const std::string &xmlContent, const std::string &symbolName);
    
    // XML parsing helper
    bool parseXMLAttribute(const std::string &xml, const std::string &attrName, std::string &value);
};

// Helper function to check if a file is a modern FLA (ZIP-based XFL)
DVAPI bool isFLAZipBased(const TFilePath &flaPath);

// Helper function to check if a directory contains XFL structure
DVAPI bool isXFLDirectory(const TFilePath &dirPath);

// Write XFL directory back into a FLA ZIP archive.
// @param xflPath: source XFL folder containing DOMDocument.xml + assets
// @param flaPath: destination .fla file (ZIP archive)
// @return: true on success.
DVAPI bool writeFLA(const TFilePath &xflPath, const TFilePath &flaPath);

} // namespace XFL

#endif // XFLREADER_H_
