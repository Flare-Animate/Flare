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
#include "ZipArchive.h"

namespace XFL {

//-----------------------------------------------------------------------------
// Internal helper: extract a ZIP archive to a directory.
//
// Delegates to the shared extractor, which repairs a wrong end-of-central-
// directory record before unpacking. Many real .fla/.swc files in the wild
// carry a stale trailer that otherwise makes unzOpen() fail outright.
//-----------------------------------------------------------------------------
static bool extractZipToDir(const std::string &zipPath, const std::string &outDir) {
    std::string detail;
    if (FlareZip::extract(TFilePath(QString::fromStdString(zipPath)),
                          TFilePath(QString::fromStdString(outDir)), detail))
        return true;
    if (!detail.empty())
        qDebug() << "[XFL] ZIP extraction failed:" << detail.c_str();
    return false;
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
    // Directory-based XFL projects (Adobe Animate / Flash "Save as XFL") are a
    // FOLDER containing DOMDocument.xml, LIBRARY/, bin/, and a tiny <name>.xfl
    // *marker* file. That marker is NOT a container — if the user selected it,
    // m_xflPath points at the marker file, so m_xflPath + "DOMDocument.xml"
    // resolves to the nonsensical "<name>.xfl/DOMDocument.xml". Detect this and
    // use the marker's parent directory (the real project root) instead.
    if (!isXFLDirectory(m_xflPath) && isXFLDirectory(m_xflPath.getParentDir())) {
        m_xflPath = m_xflPath.getParentDir();
    }

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
    
    // Walk the LIBRARY directory *recursively*. Adobe Animate mirrors the
    // library's folder structure on disk, so a typical FLA keeps most of its
    // symbols in subdirectories ("Body Parts/Characters/.../Face.xml"); a
    // flat scan therefore sees only the handful sitting at the top level.
    TFilePath libPath = m_xflPath + "LIBRARY";
    if (TSystem::doesExistFileOrLevel(libPath)) {
        TFilePathSet files;
        try {
            files = TSystem::readDirectoryTree(libPath, false, true);
        } catch (...) {
        }
        for (const auto &symbolPath : files) {
            if (symbolPath.getType() != "xml") continue;
            std::ifstream symbolFile(symbolPath.getQString().toStdString());
            if (!symbolFile.is_open()) continue;
            std::stringstream symbolBuffer;
            symbolBuffer << symbolFile.rdbuf();
            std::string symbolContent = symbolBuffer.str();
            symbolFile.close();

            // Use the path relative to LIBRARY/ as the display name: it is what
            // DOMSymbolInstance/@libraryItemName refers to for nested symbols.
            parseSymbol(symbolContent, symbolPath);
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

            // ---- content census -------------------------------------------
            // Counted even when the reader cannot convert it, so the importer
            // can tell the user what a document contains instead of silently
            // producing an empty scene.
            else if (name == "DOMShape")          ++m_document.census.shapes;
            else if (name == "DOMShapeText")      ++m_document.census.shapeText;
            else if (name == "DOMMorphShape")     ++m_document.census.morphs;
            else if (name == "DOMStaticText" ||
                     name == "DOMText")           ++m_document.census.texts;
            else if (name == "DOMSoundItem")      ++m_document.census.sounds;
            else if (name == "DOMVideoItem")      ++m_document.census.videos;
            else if (name == "DOMComponentInstance") ++m_document.census.components;

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
                ++m_document.census.bitmaps;
            }
            else if (name == "DOMSymbolInstance" && tIdx >= 0 && lIdx >= 0 && fIdx >= 0) {
                FrameElement el;
                el.type            = FrameElement::SYMBOL_INSTANCE;
                el.libraryItemName = attrs.value("libraryItemName").toString().toStdString();
                m_document.timelines[tIdx].layers[lIdx].frames[fIdx].elements.push_back(std::move(el));
                eIdx = static_cast<int>(
                    m_document.timelines[tIdx].layers[lIdx].frames[fIdx].elements.size()) - 1;
                ++m_document.census.symbols;
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

bool Reader::parseSymbol(const std::string &xmlContent, const TFilePath &symbolPath) {
    QXmlStreamReader xml(QString::fromUtf8(xmlContent.c_str()));

    // localName() strips any namespace prefix, so this matches both the
    // unprefixed <DOMSymbolItem> that Animate writes into .fla archives and the
    // <ns:DOMSymbolItem> form produced by "Save as XFL". A substring search for
    // "<DOMSymbolItem" silently failed on the prefixed variant, which dropped
    // every symbol in an uncompressed XFL project.
    bool found = false;
    Symbol symbol;
    QString relativeName;

    while (!xml.atEnd()) {
        xml.readNext();
        if (!xml.isStartElement()) continue;

        const QString name = xml.name().toString();
        const QXmlStreamAttributes attrs = xml.attributes();

        if (name == QLatin1String("DOMSymbolItem") && !found) {
            found = true;
            symbol.name = attrs.value("name").toString().toStdString();
            symbol.itemId = attrs.value("itemID").toString().toStdString();

            const QString type = attrs.value("symbolType").toString();
            if (type == QLatin1String("movie clip"))
                symbol.type = SYMBOL_MOVIECLIP;
            else if (type == QLatin1String("button"))
                symbol.type = SYMBOL_BUTTON;
            else
                symbol.type = SYMBOL_GRAPHIC;

            symbol.linkageClass = attrs.value("linkageClassName").toString().toStdString();
            symbol.linkageExport =
                (attrs.value("linkageExportForAS").toString() == QLatin1String("true"));

        // Bitmap assets live here, not in a <media> section: Animate writes each
        // one as a graphic symbol whose <BitmapData href="..."/> points at the
        // real file. Without this the bitmap list is empty for every modern FLA
        // and the whole timeline import is skipped.
        } else if (name == QLatin1String("DOMBitmapItem") ||
                   name == QLatin1String("BitmapData")) {
            const QString href = attrs.value("href").toString();
            if (href.isEmpty()) continue;
            BitmapItem item;
            item.href = href.toStdString();
            const int slash = href.lastIndexOf(QLatin1Char('/'));
            const QString base = (slash >= 0) ? href.mid(slash + 1) : href;
            const int dot = base.lastIndexOf(QLatin1Char('.'));
            item.name = ((dot > 0) ? base.left(dot) : base)
                            .replace(QLatin1Char('&'), QLatin1String("and"))
                            .toStdString();
            if (symbol.name.empty()) symbol.name = item.name;
            if (item.name.empty()) item.name = symbol.name;
            m_document.bitmaps.push_back(item);

        } else if (name == QLatin1String("DOMShape")) {
            ++m_document.census.shapes;
        } else if (name == QLatin1String("DOMShapeText")) {
            ++m_document.census.shapeText;
        } else if (name == QLatin1String("DOMMorphShape")) {
            ++m_document.census.morphs;
        } else if (name == QLatin1String("DOMStaticText") ||
                   name == QLatin1String("DOMText")) {
            ++m_document.census.texts;
        } else if (name == QLatin1String("DOMSoundItem")) {
            ++m_document.census.sounds;
        } else if (name == QLatin1String("DOMVideoItem")) {
            ++m_document.census.videos;
        } else if (name == QLatin1String("DOMComponentInstance")) {
            ++m_document.census.components;
        } else if (name == QLatin1String("DOMSymbolInstance") ||
                   name == QLatin1String("DOMBitmapInstance")) {
            // Record the library item name a symbol instance refers to, so a
            // bitmap symbol can still be resolved by name.
            const QString ref = attrs.value("libraryItemName").toString();
            if (!found && !ref.isEmpty()) relativeName = ref;
        }
    }

    if (!found) return false;

    // Prefer the name declared in the file; fall back to the path relative to
    // LIBRARY/, which is how nested symbols are addressed.
    if (symbol.name.empty()) {
        symbol.name = relativeName.toStdString();
    }
    if (symbol.name.empty()) {
        symbol.name = symbolPath.getName();
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
