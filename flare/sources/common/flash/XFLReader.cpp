// XFLReader.cpp - XFL (XML Flash) format reader implementation
// Copyright (c) 2026 Flare Project

#include "XFLReader.h"
#include "tsystem.h"
#include "tconvert.h"
#include <fstream>
#include <sstream>
#include <cstring>

// Minimal XML parsing - for full implementation, would use a proper XML library
// For now, simple attribute extraction for key properties

namespace XFL {

//-----------------------------------------------------------------------------
// Reader implementation
//-----------------------------------------------------------------------------

Reader::Reader(const TFilePath &xflPath) 
    : m_xflPath(xflPath)
    , m_isZip(false)
{
    // Check if it's a ZIP-based file (modern FLA or XFL)
    std::string ext = xflPath.getType();
    m_isZip = (ext == "fla" || ext == "xfl");
}

Reader::~Reader() {
}

bool Reader::read() {
    m_error.clear();
    
    if (m_isZip) {
        return readFromZip();
    } else {
        return readFromDirectory();
    }
}

bool Reader::readFromZip() {
    // For ZIP reading, we need to extract first
    // This is a placeholder - full implementation would use zip library
    m_error = "ZIP-based FLA reading requires extraction. Use extractZip() first or open the extracted directory.";
    return false;
}

bool Reader::readFromDirectory() {
    // Look for DOMDocument.xml in the directory
    TFilePath docPath = m_xflPath + "DOMDocument.xml";
    
    if (!TSystem::doesExistFileOrLevel(docPath)) {
        m_error = "DOMDocument.xml not found in XFL directory";
        return false;
    }
    
    // Read the document XML
    std::ifstream file(docPath.getQString().toStdString());
    if (!file.is_open()) {
        m_error = "Cannot open DOMDocument.xml";
        return false;
    }
    
    std::stringstream buffer;
    buffer << file.rdbuf();
    std::string xmlContent = buffer.str();
    file.close();
    
    if (!parseDOMDocument(xmlContent)) {
        return false;
    }
    
    // Look for library symbols in LIBRARY directory
    TFilePath libPath = m_xflPath + "LIBRARY";
    if (TSystem::doesExistFileOrLevel(libPath)) {
        TFilePathSet files = TSystem::readDirectory(libPath, true, false, true);
        for (const auto &symbolPath : files) {
            if (symbolPath.getType() == "xml") {
                std::ifstream symbolFile(symbolPath.getQString().toStdString());
                if (symbolFile.is_open()) {
                    std::stringstream symbolBuffer;
                    symbolBuffer << symbolFile.rdbuf();
                    std::string symbolContent = symbolBuffer.str();
                    symbolFile.close();
                    
                    parseSymbol(symbolContent, symbolPath.getName());
                }
            }
        }
    }
    
    return true;
}

bool Reader::parseDOMDocument(const std::string &xmlContent) {
    // Simple attribute parsing (would use proper XML parser in production)
    std::string value;
    
    if (parseXMLAttribute(xmlContent, "width", value)) {
        m_document.width = std::stoi(value);
    }
    
    if (parseXMLAttribute(xmlContent, "height", value)) {
        m_document.height = std::stoi(value);
    }
    
    if (parseXMLAttribute(xmlContent, "frameRate", value)) {
        m_document.frameRate = std::stod(value);
    }
    
    if (parseXMLAttribute(xmlContent, "backgroundColor", value)) {
        m_document.backgroundColor = value;
    }
    
    return true;
}

bool Reader::parseSymbol(const std::string &xmlContent, const std::string &symbolName) {
    // Check if this is a DOMSymbolItem
    if (xmlContent.find("<DOMSymbolItem") == std::string::npos) {
        return false;
    }
    
    Symbol symbol;
    symbol.name = symbolName;
    
    std::string value;
    
    if (parseXMLAttribute(xmlContent, "itemID", value)) {
        symbol.itemId = value;
    }
    
    if (parseXMLAttribute(xmlContent, "symbolType", value)) {
        if (value == "movie clip") {
            symbol.type = SYMBOL_MOVIECLIP;
        } else if (value == "button") {
            symbol.type = SYMBOL_BUTTON;
        } else {
            symbol.type = SYMBOL_GRAPHIC;
        }
    }
    
    if (parseXMLAttribute(xmlContent, "linkageClassName", value)) {
        symbol.linkageClass = value;
    }
    
    if (parseXMLAttribute(xmlContent, "linkageExportForAS", value)) {
        symbol.linkageExport = (value == "true");
    }
    
    m_document.symbols.push_back(symbol);
    return true;
}

bool Reader::parseXMLAttribute(const std::string &xml, const std::string &attrName, std::string &value) {
    // Simple attribute extraction: attrName="value"
    std::string searchStr = attrName + "=\"";
    size_t pos = xml.find(searchStr);
    if (pos == std::string::npos) {
        return false;
    }
    
    pos += searchStr.length();
    size_t endPos = xml.find("\"", pos);
    if (endPos == std::string::npos) {
        return false;
    }
    
    value = xml.substr(pos, endPos - pos);
    return true;
}

TFilePath Reader::extractZip(const TFilePath &outputDir) {
    // Placeholder for ZIP extraction
    // Would use zlib or similar to extract
    m_error = "ZIP extraction not yet implemented - use external tool to extract .fla file first";
    return TFilePath();
}

//-----------------------------------------------------------------------------
// Helper functions
//-----------------------------------------------------------------------------

bool isFLAZipBased(const TFilePath &flaPath) {
    // Check if file exists and has .fla extension
    if (!TSystem::doesExistFileOrLevel(flaPath)) {
        return false;
    }
    
    if (flaPath.getType() != "fla") {
        return false;
    }
    
    // Read first few bytes to check for ZIP signature (PK)
    std::ifstream file(flaPath.getQString().toStdString(), std::ios::binary);
    if (!file.is_open()) {
        return false;
    }
    
    char header[2];
    file.read(header, 2);
    file.close();
    
    // ZIP files start with 'PK' (0x50 0x4B)
    return (header[0] == 0x50 && header[1] == 0x4B);
}

bool isXFLDirectory(const TFilePath &dirPath) {
    // Check if directory contains DOMDocument.xml
    TFilePath docPath = dirPath + "DOMDocument.xml";
    return TSystem::doesExistFileOrLevel(docPath);
}

} // namespace XFL
