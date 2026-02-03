// XFLReader.h - XFL (XML Flash) format reader
// Copyright (c) 2026 Flare Project
// 
// This module provides XFL format parsing capabilities for importing
// Adobe Animate/Flash projects. XFL is an XML-based format used by
// modern Flash/Animate as the source format for FLA files.
//
// References:
// - Adobe XFL format specification
// - jpexs-decompiler XFLConverter implementation

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
    std::vector<Symbol> symbols;
    
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

} // namespace XFL

#endif // XFLREADER_H_
