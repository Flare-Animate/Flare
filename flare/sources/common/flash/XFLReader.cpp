// XFLReader.cpp - XFL (XML Flash) format reader implementation
// Copyright (c) 2026 Flare Project

#include "XFLReader.h"
#include "tsystem.h"
#include "tconvert.h"
#include <fstream>
#include <sstream>
#include <cstring>
#include <vector>

// Minizip for ZIP/FLA extraction (from thirdparty/zlib-1.2.8/contrib/minizip)
#include "unzip.h"

namespace XFL {

//-----------------------------------------------------------------------------
// Internal helper: extract a ZIP archive to a directory using minizip
// Returns true on success.
//-----------------------------------------------------------------------------
static bool extractZipToDir(const std::string &zipPath, const std::string &outDir) {
    unzFile uf = unzOpen(zipPath.c_str());
    if (!uf) return false;

    unz_global_info gi;
    if (unzGetGlobalInfo(uf, &gi) != UNZ_OK) {
        unzClose(uf);
        return false;
    }

    char fileName[512];
    char buf[8192];

    for (uLong i = 0; i < gi.number_entry; i++) {
        unz_file_info fi;
        if (unzGetCurrentFileInfo(uf, &fi, fileName, sizeof(fileName), nullptr, 0, nullptr, 0) != UNZ_OK)
            break;

        std::string fullOut = outDir + "/" + fileName;

        // If it ends with '/', it's a directory entry
        if (fileName[strlen(fileName) - 1] == '/') {
            TSystem::mkDir(TFilePath(fullOut));
        } else {
            // Ensure parent directory exists
            std::string parent = fullOut.substr(0, fullOut.rfind('/'));
            if (!parent.empty()) TSystem::mkDir(TFilePath(parent));

            if (unzOpenCurrentFile(uf) == UNZ_OK) {
                FILE *fp = fopen(fullOut.c_str(), "wb");
                if (fp) {
                    int nbytes;
                    while ((nbytes = unzReadCurrentFile(uf, buf, sizeof(buf))) > 0)
                        fwrite(buf, 1, nbytes, fp);
                    fclose(fp);
                }
                unzCloseCurrentFile(uf);
            }
        }

        if (i + 1 < gi.number_entry) {
            if (unzGoToNextFile(uf) != UNZ_OK) break;
        }
    }

    unzClose(uf);
    return true;
}

//-----------------------------------------------------------------------------
// Reader implementation
//-----------------------------------------------------------------------------

Reader::Reader(const TFilePath &xflPath) 
    : m_xflPath(xflPath)
    , m_isZip(false)
{
    std::string ext = xflPath.getType();
    // .fla and .swc are ZIP archives; .xfl files can be either ZIP or directory
    m_isZip = (ext == "fla" || ext == "swc");
    if (ext == "xfl") {
        // Check for ZIP signature
        m_isZip = isFLAZipBased(xflPath);
    }
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
    // Extract ZIP to a temp directory, then read as directory
    TFilePath tmpDir = TSystem::getTempDir() + TFilePath("xfl_import_tmp");
    TSystem::mkDir(tmpDir);

    TFilePath extracted = extractZip(tmpDir);
    if (extracted.isEmpty()) {
        return false;
    }

    // If the extracted result is a directory, read it
    if (isXFLDirectory(extracted)) {
        m_xflPath = extracted;
        m_isZip = false;
        return readFromDirectory();
    }

    // Try the extracted directory directly
    m_xflPath = tmpDir;
    m_isZip = false;
    return readFromDirectory();
}

bool Reader::readFromDirectory() {
    // Look for DOMDocument.xml in the directory
    TFilePath docPath = m_xflPath + "DOMDocument.xml";
    
    if (!TSystem::doesExistFileOrLevel(docPath)) {
        m_error = "DOMDocument.xml not found in XFL directory: " + m_xflPath.getQString().toStdString();
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
    std::string value;
    
    if (parseXMLAttribute(xmlContent, "width", value)) {
        try { m_document.width = std::stoi(value); } catch (...) {}
    }
    
    if (parseXMLAttribute(xmlContent, "height", value)) {
        try { m_document.height = std::stoi(value); } catch (...) {}
    }
    
    if (parseXMLAttribute(xmlContent, "frameRate", value)) {
        try { m_document.frameRate = std::stod(value); } catch (...) {}
    }
    
    if (parseXMLAttribute(xmlContent, "backgroundColor", value)) {
        m_document.backgroundColor = value;
    }
    
    return true;
}

bool Reader::parseSymbol(const std::string &xmlContent, const std::string &symbolName) {
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
    std::string searchStr = attrName + "=\"";
    size_t pos = xml.find(searchStr);
    if (pos == std::string::npos) return false;
    
    pos += searchStr.length();
    size_t endPos = xml.find("\"", pos);
    if (endPos == std::string::npos) return false;
    
    value = xml.substr(pos, endPos - pos);
    return true;
}

TFilePath Reader::extractZip(const TFilePath &outputDir) {
    TFilePath outDir = outputDir;
    if (outDir.isEmpty()) {
        outDir = TSystem::getTempDir() + TFilePath("xfl_extract");
    }

    if (!TSystem::doesExistFileOrLevel(outDir)) {
        TSystem::mkDir(outDir);
    }

    std::string zipPath = m_xflPath.getQString().toStdString();
    std::string outPath = outDir.getQString().toStdString();

    if (!extractZipToDir(zipPath, outPath)) {
        m_error = "Failed to extract ZIP archive: " + zipPath;
        return TFilePath();
    }

    // Try to find the XFL directory inside the extracted folder
    // It might be directly in outDir or in a subdirectory
    if (isXFLDirectory(outDir)) {
        return outDir;
    }

    // Search one level deep for DOMDocument.xml
    try {
        TFilePathSet entries = TSystem::readDirectory(outDir, false, false, true);
        for (const auto &entry : entries) {
            if (isXFLDirectory(entry)) {
                return entry;
            }
        }
    } catch (...) {}

    return outDir;
}

//-----------------------------------------------------------------------------
// Helper functions
//-----------------------------------------------------------------------------

bool isFLAZipBased(const TFilePath &flaPath) {
    if (!TSystem::doesExistFileOrLevel(flaPath)) return false;
    
    std::ifstream file(flaPath.getQString().toStdString(), std::ios::binary);
    if (!file.is_open()) return false;
    
    char header[2];
    file.read(header, 2);
    file.close();
    
    // ZIP files start with 'PK' (0x50 0x4B)
    return (static_cast<unsigned char>(header[0]) == 0x50 &&
            static_cast<unsigned char>(header[1]) == 0x4B);
}

bool isXFLDirectory(const TFilePath &dirPath) {
    TFilePath docPath = dirPath + "DOMDocument.xml";
    return TSystem::doesExistFileOrLevel(docPath);
}

} // namespace XFL
