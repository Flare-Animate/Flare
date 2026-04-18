// XFLReader.cpp - XFL (XML Flash) format reader implementation
// Copyright (c) 2026 Flare Project
//
// Format knowledge (ideas only, no code copied):
//   - Adobe XFL specification (public) — element/attribute names, structure
//   - jpexs-decompiler (GPL-3.0)       — layer/frame/element model
//   - fla-viewer (MIT)                 — attribute-parsing approach
//   - ruffle (MIT/Apache-2.0)          — SWF format context (cross-reference)
//   - open-flash libraries (ISC)       — format constants

#include "XFLReader.h"
#include "tsystem.h"
#include "tconvert.h"
#include <fstream>
#include <sstream>
#include <cstring>
#include <vector>
#include <QString>
#include <QDateTime>
#include <QDebug>
#include <QDir>
#include <QXmlStreamReader>

// Minizip for ZIP/FLA extraction (from thirdparty/zlib-1.2.8/contrib/minizip)
#include "../../../../thirdparty/zlib-1.2.8/contrib/minizip/unzip.h"
#include "../../../../thirdparty/zlib-1.2.8/contrib/minizip/zip.h"

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

    char fileName[1024];  // larger buffer for deeply-nested paths
    char buf[8192];

    for (uLong i = 0; i < gi.number_entry; i++) {
        unz_file_info fi;
        fileName[0] = '\0';  // Initialize buffer
        if (unzGetCurrentFileInfo(uf, &fi, fileName, sizeof(fileName), nullptr, 0, nullptr, 0) != UNZ_OK)
            break;

        size_t nameLen = strlen(fileName);
        if (nameLen == 0) {
            // Skip empty entries
            if (i + 1 < gi.number_entry) unzGoToNextFile(uf);
            continue;
        }

        // Zip Slip protection: reject path traversal and absolute paths
        std::string entryStr(fileName, nameLen);

        // Normalize path separators and strip leading ./ for correct DOMDocument.xml detection
        for (auto &ch : entryStr) {
            if (ch == '\\') ch = '/';
        }
        while (entryStr.rfind("./", 0) == 0) {
            entryStr.erase(0, 2);
        }
        while (!entryStr.empty() && entryStr[0] == '/') {
            entryStr.erase(0, 1);
        }

        if (entryStr.empty() ||
            entryStr.find("../") != std::string::npos ||
            entryStr.find("..\\") != std::string::npos ||
            entryStr[0] == '/' || entryStr[0] == '\\' ||
            (entryStr.size() >= 2 && entryStr[1] == ':')) {
            if (i + 1 < gi.number_entry) unzGoToNextFile(uf);
            continue;
        }

        std::string fullOut = outDir + "/" + entryStr;

        // If it ends with '/', it's a directory entry
        if (fileName[nameLen - 1] == '/') {
            TSystem::mkDir(TFilePath(fullOut));
        } else {
            // Ensure parent directory exists
            size_t slashPos = fullOut.rfind('/');
            if (slashPos != std::string::npos) {
                std::string parent = fullOut.substr(0, slashPos);
                if (!parent.empty()) TSystem::mkDir(TFilePath(parent));
            }

            if (unzOpenCurrentFile(uf) == UNZ_OK) {
                FILE *fp = fopen(fullOut.c_str(), "wb");
                if (fp) {
                    int nbytes;
                    bool writeSuccess = true;
                    while ((nbytes = unzReadCurrentFile(uf, buf, sizeof(buf))) > 0) {
                        if (fwrite(buf, 1, nbytes, fp) != static_cast<size_t>(nbytes)) {
                            writeSuccess = false;
                            break;
                        }
                    }
                    fclose(fp);
                    if (!writeSuccess) {
                        // Do not fail the entire archive extraction for a single file;
                        // continue with the next entry after cleaning up.
                        unzCloseCurrentFile(uf);
                        if (i + 1 < gi.number_entry) unzGoToNextFile(uf);
                        continue;
                    }
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
    // Extract ZIP to a unique temp directory per import to avoid collisions
    QString uniqueName = "xfl_import_" + QString::number(
        QDateTime::currentMSecsSinceEpoch());
    TFilePath tmpDir = TSystem::getTempDir() + TFilePath(uniqueName.toStdString());
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
        // Fallback: scan first-level subdirectories for DOMDocument.xml
        try {
            TFilePathSet entries = TSystem::readDirectory(m_xflPath, false, true, true);
            for (const auto &entry : entries) {
                if (entry.getType() == "" && isXFLDirectory(entry)) {
                    m_xflPath = entry;
                    docPath   = m_xflPath + "DOMDocument.xml";
                    break;
                }
            }
        } catch (...) {}
    }

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

// ---------------------------------------------------------------------------
// Full DOMDocument.xml parser using QXmlStreamReader
//
// XFL document structure (Adobe XFL spec):
//   <DOMDocument width="..." height="..." frameRate="..." backgroundColor="...">
//     <media>
//       <DOMBitmapItem name="..." href="LIBRARY/foo.png" .../>
//     </media>
//     <symbols>
//       <Include href="LIBRARY/Symbol1.xml" name="Symbol1"/>
//     </symbols>
//     <timelines>
//       <DOMTimeline name="Scene 1">
//         <layers>
//           <DOMLayer name="..." layerType="normal|guide|mask|folder">
//             <frames>
//               <DOMFrame index="0" duration="1" keyFrame="true">
//                 <elements>
//                   <DOMBitmapInstance libraryItemName="...">
//                     <matrix><Matrix a="1" b="0" c="0" d="1" tx="0" ty="0"/></matrix>
//                   </DOMBitmapInstance>
//                 </elements>
//               </DOMFrame>
//             </frames>
//           </DOMLayer>
//         </layers>
//       </DOMTimeline>
//     </timelines>
//   </DOMDocument>
// ---------------------------------------------------------------------------
bool Reader::parseDOMDocument(const std::string &xmlContent) {
    QXmlStreamReader xml(QString::fromUtf8(xmlContent.c_str()));

    // Indices track position in each container; using indices (not pointers)
    // avoids iterator invalidation when vector::push_back reallocates.
    int tIdx = -1;  // current XFLTimeline in m_document.timelines
    int lIdx = -1;  // current XFLLayer  in timelines[tIdx].layers
    int fIdx = -1;  // current XFLFrame  in layers[lIdx].frames
    int eIdx = -1;  // current FrameElement in frames[fIdx].elements

    bool inMatrix = false;  // inside a <Matrix> element scoped to an element

    while (!xml.atEnd()) {
        xml.readNext();

        if (xml.isStartElement()) {
            const QString name = xml.name().toString();
            const QXmlStreamAttributes attrs = xml.attributes();

            // ---- root document attributes ----
            if (name == "DOMDocument") {
                if (attrs.hasAttribute("width"))
                    try { m_document.width = attrs.value("width").toInt(); } catch (...) {}
                if (attrs.hasAttribute("height"))
                    try { m_document.height = attrs.value("height").toInt(); } catch (...) {}
                if (attrs.hasAttribute("frameRate"))
                    try { m_document.frameRate = attrs.value("frameRate").toDouble(); } catch (...) {}
                if (attrs.hasAttribute("backgroundColor"))
                    m_document.backgroundColor = attrs.value("backgroundColor").toString().toStdString();
            }

            // ---- media section: bitmap library items ----
            else if (name == "DOMBitmapItem") {
                BitmapItem bi;
                bi.name = attrs.value("name").toString().toStdString();
                bi.href = attrs.value("href").toString().toStdString();
                if (!bi.name.empty())
                    m_document.bitmaps.push_back(bi);
            }

            // ---- timelines ----
            else if (name == "DOMTimeline") {
                XFLTimeline tl;
                tl.name = attrs.value("name").toString().toStdString();
                m_document.timelines.push_back(std::move(tl));
                tIdx = static_cast<int>(m_document.timelines.size()) - 1;
                lIdx = fIdx = eIdx = -1;
            }
            else if (name == "DOMLayer" && tIdx >= 0) {
                XFLLayer layer;
                layer.name     = attrs.value("name").toString().toStdString();
                layer.layerType = attrs.value("layerType").toString().toStdString();
                if (layer.layerType.empty()) layer.layerType = "normal";
                m_document.timelines[tIdx].layers.push_back(std::move(layer));
                lIdx = static_cast<int>(m_document.timelines[tIdx].layers.size()) - 1;
                fIdx = eIdx = -1;
            }
            else if (name == "DOMFrame" && tIdx >= 0 && lIdx >= 0) {
                XFLFrame frame;
                frame.index    = attrs.value("index").toInt();
                frame.duration = attrs.hasAttribute("duration")
                                    ? attrs.value("duration").toInt() : 1;
                frame.keyFrame = (attrs.value("keyFrame").toString() == "true");
                if (attrs.hasAttribute("name"))
                    frame.name = attrs.value("name").toString().toStdString();
                if (attrs.hasAttribute("tweenType"))
                    frame.tweenType = attrs.value("tweenType").toString().toStdString();
                m_document.timelines[tIdx].layers[lIdx].frames.push_back(std::move(frame));
                fIdx = static_cast<int>(m_document.timelines[tIdx].layers[lIdx].frames.size()) - 1;
                eIdx = -1;
            }
            else if (name == "DOMBitmapInstance" && tIdx >= 0 && lIdx >= 0 && fIdx >= 0) {
                FrameElement el;
                el.type            = FrameElement::BITMAP_INSTANCE;
                el.libraryItemName = attrs.value("libraryItemName").toString().toStdString();
                m_document.timelines[tIdx].layers[lIdx].frames[fIdx].elements.push_back(std::move(el));
                eIdx = static_cast<int>(
                    m_document.timelines[tIdx].layers[lIdx].frames[fIdx].elements.size()) - 1;
            }
            else if (name == "DOMSymbolInstance" && tIdx >= 0 && lIdx >= 0 && fIdx >= 0) {
                FrameElement el;
                el.type            = FrameElement::SYMBOL_INSTANCE;
                el.libraryItemName = attrs.value("libraryItemName").toString().toStdString();
                m_document.timelines[tIdx].layers[lIdx].frames[fIdx].elements.push_back(std::move(el));
                eIdx = static_cast<int>(
                    m_document.timelines[tIdx].layers[lIdx].frames[fIdx].elements.size()) - 1;
            }
            // <matrix><Matrix .../></matrix> — transform for the current element
            else if (name == "Matrix" && eIdx >= 0 && tIdx >= 0 && lIdx >= 0 && fIdx >= 0) {
                Transform &m =
                    m_document.timelines[tIdx].layers[lIdx].frames[fIdx].elements[eIdx].matrix;
                if (attrs.hasAttribute("a"))  m.a  = attrs.value("a").toDouble();
                if (attrs.hasAttribute("b"))  m.b  = attrs.value("b").toDouble();
                if (attrs.hasAttribute("c"))  m.c  = attrs.value("c").toDouble();
                if (attrs.hasAttribute("d"))  m.d  = attrs.value("d").toDouble();
                if (attrs.hasAttribute("tx")) m.tx = attrs.value("tx").toDouble();
                if (attrs.hasAttribute("ty")) m.ty = attrs.value("ty").toDouble();
            }
        }
        else if (xml.isEndElement()) {
            const QString name = xml.name().toString();
            if      (name == "DOMBitmapInstance" || name == "DOMSymbolInstance") eIdx = -1;
            else if (name == "DOMFrame")    fIdx = -1;
            else if (name == "DOMLayer")    lIdx = -1;
            else if (name == "DOMTimeline") { tIdx = -1; lIdx = fIdx = eIdx = -1; }
        }
    }

    if (xml.hasError()) {
        // Non-fatal: report but proceed with whatever was parsed.
        m_error += "XML warning: " + xml.errorString().toStdString() + "\n";
    }

    return true;
}

bool Reader::parseSymbol(const std::string &xmlContent, const std::string &symbolName) {
    if (xmlContent.find("<DOMSymbolItem") == std::string::npos) {
        return false;
    }

    Symbol symbol;
    symbol.name = symbolName;

    // Use simple attribute extraction for symbol-level metadata only.
    std::string value;
    if (parseXMLAttribute(xmlContent, "itemID", value))
        symbol.itemId = value;

    if (parseXMLAttribute(xmlContent, "symbolType", value)) {
        if (value == "movie clip")
            symbol.type = SYMBOL_MOVIECLIP;
        else if (value == "button")
            symbol.type = SYMBOL_BUTTON;
        else
            symbol.type = SYMBOL_GRAPHIC;
    }

    if (parseXMLAttribute(xmlContent, "linkageClassName", value))
        symbol.linkageClass = value;

    if (parseXMLAttribute(xmlContent, "linkageExportForAS", value))
        symbol.linkageExport = (value == "true");

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

bool writeFLA(const TFilePath &xflPath, const TFilePath &flaPath) {
    if (!TSystem::doesExistFileOrLevel(xflPath) || !TFileStatus(xflPath).isDirectory()) {
        qDebug() << "[XFL] writeFLA failed: source is not a directory" << xflPath.getQString();
        return false;
    }

    QString srcDir = xflPath.getQString();
    QString dstZip = flaPath.getQString();
    qDebug() << "[XFL] writeFLA" << srcDir << "->" << dstZip;

    zipFile zf = zipOpen(dstZip.toUtf8().constData(), APPEND_STATUS_CREATE);
    if (!zf) {
        qDebug() << "[XFL] writeFLA failed: could not open output zip" << dstZip;
        return false;
    }

    TFilePathSet files;
    try {
        // readDirectoryTree(path, groupFrames=false, onlyFiles=true): recursively
        // lists all files under xflPath, including LIBRARY/ subdirectory assets.
        files = TSystem::readDirectoryTree(xflPath, false, true);
    } catch (...) {
        qDebug() << "[XFL] writeFLA failed: could not read source directory" << srcDir;
        zipClose(zf, nullptr);
        return false;
    }

    for (const auto &fp : files) {
        QString fullPath = fp.getQString();
        QString relPath = QDir(srcDir).relativeFilePath(fullPath).replace('\\', '/');
        if (relPath.isEmpty()) continue;

        zip_fileinfo zi = {};
        int err = zipOpenNewFileInZip(zf, relPath.toUtf8().constData(), &zi,
                                     nullptr, 0, nullptr, 0, nullptr,
                                     Z_DEFLATED, Z_DEFAULT_COMPRESSION);
        if (err != ZIP_OK) {
            qDebug() << "[XFL] writeFLA failed: cannot add" << relPath;
            zipClose(zf, nullptr);
            return false;
        }

        QFile inFile(fullPath);
        if (!inFile.open(QIODevice::ReadOnly)) {
            qDebug() << "[XFL] writeFLA failed: cannot open file" << fullPath;
            zipCloseFileInZip(zf);
            zipClose(zf, nullptr);
            return false;
        }

        QByteArray content = inFile.readAll();
        inFile.close();

        if (!content.isEmpty()) {
            if (zipWriteInFileInZip(zf, content.constData(), content.size()) != ZIP_OK) {
                qDebug() << "[XFL] writeFLA failed: error writing file" << relPath;
                zipCloseFileInZip(zf);
                zipClose(zf, nullptr);
                return false;
            }
        }

        zipCloseFileInZip(zf);
    }

    if (zipClose(zf, nullptr) != ZIP_OK) {
        qDebug() << "[XFL] writeFLA failed: could not close zip" << dstZip;
        return false;
    }

    qDebug() << "[XFL] writeFLA success" << dstZip;
    return true;
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
