// flashimport.cpp - Native built-in Flash format import (FLA, XFL, SWF, SWC, FLV, F4V, AS)
// No external tools or third-party processes required.
//
// FLA  = ZIP archive containing an XFL document (DOMDocument.xml + assets)
// XFL  = Unzipped FLA; a directory or ZIP containing DOMDocument.xml
// SWF  = Compiled Flash binary (header + tag stream)
// SWC  = ZIP archive containing library.swf and assets (Apache Flex SDK format)
// FLV  = Flash Video container
// F4V  = Flash H.264 video (ISO BMFF / MPEG-4 Part 12)
// AS   = ActionScript source file (imported as plain text for reference)

#include "flare/menubarcommandids.h"
#include "flare/menubar.h"
#include "flare/ocaio.h"
#include "flare/tproject.h"
#include "flare/preferences.h"
#include "flare/tapp.h"
#include "flare/tscenehandle.h"
#include "flare/txsheethandle.h"
#include "flare/toonzfolders.h"
#include "flare/toonzscene.h"
#include "flare/txsheet.h"
#include "flare/txshcell.h"
#include "flare/txshsimplelevel.h"
#include "flare/txshlevelcolumn.h"
#include "flare/tstageobject.h"

#include "flareqt/gutil.h"
#include "flareqt/dvdialog.h"
#include "flare/filebrowserpopup.h"
#include "iocommand.h"

#include "tsystem.h"
#include "tfilepath.h"

// Native flash infrastructure (include_directories contains ../common/flash)
#include "XFLReader.h"
#include "ZipArchive.h"
#include "SWFAssets.h"
#include "As3Bridge.h"
#include "FSWFStream.h"
#include "Macromedia.h"

#include <QFile>
#include <QDir>
#include <QImage>
#include <QVector>
#include <QSet>
#include <QList>
#include <QDirIterator>
#include <QDateTime>
#include <QFileInfo>
#include <QDesktopServices>
#include <QUrl>
#include <QDebug>
#include <QFileDialog>
#include <QRegularExpression>

#include <fstream>
#include <cstring>
#include <climits>
#include <algorithm>

using namespace DVGui;

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

namespace {

// Create a unique timestamped output directory under the system temp dir.
static TFilePath makeTempImportDir(const QString &prefix) {
    QString name = prefix + QString::number(QDateTime::currentMSecsSinceEpoch());
    TFilePath dir = TSystem::getTempDir() + TFilePath(name.toStdString());
    try { TSystem::mkDir(dir); } catch (...) {}
    return dir;
}

// Asset file filters for auto-import scan
static const QStringList kAssetFilters = {
    "*.png", "*.jpg", "*.jpeg", "*.svg", "*.xml", "*.as", "*.jsfl"
};

// Extract every entry of a ZIP archive to outDir.
//
// Thin wrapper over the shared ZipArchive extractor, which repairs a wrong
// end-of-central-directory record before unpacking (many real .fla files carry
// a stale trailer) and owns the Zip Slip hardening.
static bool extractZip(const QString &zipPath, const QString &outDir) {
    std::string detail;
    if (FlareZip::extract(TFilePath(zipPath), TFilePath(outDir), detail)) return true;
    if (!detail.empty()) qDebug() << "[FlashImport] ZIP extraction failed:" << detail.c_str();
    return false;
}

// Build a plain-text manifest listing imported files in outDir.
static void writeManifest(const QString &outDir, const QStringList &files,
                          const QString &sourceFile) {
    QFile mf(outDir + "/manifest.txt");
    if (!mf.open(QIODevice::WriteOnly | QIODevice::Text)) return;
    mf.write(QByteArray("Source: ") + sourceFile.toUtf8() + "\n");
    mf.write("Exported files:\n");
    for (const auto &f : files) mf.write(QByteArray("  ") + f.toUtf8() + "\n");
    mf.close();
}

// Open the output folder in the system file manager.
static void openFolder(const QString &path) {
    QDesktopServices::openUrl(QUrl::fromLocalFile(path));
}

// ---------------------------------------------------------------------------
// Copy a single file to outDir and add its filename to the exported list.
// Used by text/binary format handlers (AS, ASC, MXML, LWF, RSL, AFL).
// ---------------------------------------------------------------------------
static void copyFileForReference(const QString &srcPath, const QString &outDir,
                                 QStringList &exported, const QString &infoMsg = {}) {
    QString fname = QFileInfo(srcPath).fileName();
    QString dst   = outDir + "/" + fname;
    QFile::copy(srcPath, dst);
    exported << fname;
    (void)infoMsg;
}

// Adobe JSFL uses host-specific APIs. Flare never executes imported scripts.
static QStringList findJsflFunctions(const QString &source) {
    QStringList names;
    QRegularExpression re(R"(\bfunction\s+([A-Za-z_$][A-Za-z0-9_$]*)\s*\()",
                          QRegularExpression::MultilineOption);
    QRegularExpressionMatchIterator it = re.globalMatch(source);
    while (it.hasNext()) names << it.next().captured(1);
    names.removeDuplicates();
    return names;
}

// ---------------------------------------------------------------------------
// ANE / AIR / OAM — ZIP-based Adobe packaging formats.
//
// ANE  (Adobe Native Extension) — ZIP; contains META-INF/ANE/extension.xml
// AIR  (Adobe AIR application)  — ZIP; contains META-INF/AIR/application.xml
// OAM  (Open Architecture Mod.) — ZIP; contains OAMMetadata.xml or META-INF/OAM/metadata.xml
//
// All three use the same ZIP extraction pipeline as FLA/SWC.
// Format knowledge: Adobe AIR SDK Reference, Adobe Animate OAM spec.
// ---------------------------------------------------------------------------
static QString extractAdobeZipPackage(const QString &srcPath, const QString &outDir,
                                      const QString &ext) {
    if (!extractZip(srcPath, outDir)) return {};

    // Find and report the manifest/metadata XML specific to each format.
    QStringList candidates;
    if (ext == "ane")
        candidates = {"META-INF/ANE/extension.xml", "META-INF/extension.xml"};
    else if (ext == "air")
        candidates = {"META-INF/AIR/application.xml", "META-INF/MANIFEST.MF"};
    else if (ext == "oam")
        candidates = {"OAMMetadata.xml", "META-INF/OAM/metadata.xml", "metadata.xml"};
    else if (ext == "zxp")
        candidates = {"CSXS/manifest.xml", "META-INF/manifest.xml", "manifest.xml"};
    else if (ext == "mxp")
        candidates = {"install.xml", "Install.xml", "manifest.xml"};

    for (const QString &rel : candidates) {
        QFile f(outDir + "/" + rel);
        if (f.exists()) {
            return rel;  // return the found manifest path
        }
    }
    return {};
}

// Name the content an XFL/FLA document holds that the importer cannot yet turn
// into a level. A vector-only FLA is the common case, and without this it
// reports a successful import that added nothing to the scene - which looks
// exactly like a broken file.
static QString describeUnconverted(const XFL::ContentCensus &c) {
    QStringList missing;
    if (c.shapes)     missing << QObject::tr("%1 vector shape(s)").arg(c.shapes);
    if (c.shapeText)  missing << QObject::tr("%1 shape text object(s)").arg(c.shapeText);
    if (c.texts)      missing << QObject::tr("%1 text object(s)").arg(c.texts);
    if (c.morphs)     missing << QObject::tr("%1 morph shape(s)").arg(c.morphs);
    if (c.sounds)     missing << QObject::tr("%1 sound(s)").arg(c.sounds);
    if (c.videos)     missing << QObject::tr("%1 video item(s)").arg(c.videos);
    if (c.components) missing << QObject::tr("%1 component instance(s)").arg(c.components);
    if (missing.isEmpty()) return {};
    return QObject::tr("\n  Not converted to levels: %1. Vector art and text "
                       "import is not implemented yet; the document is unpacked "
                       "to the export folder.")
        .arg(missing.join(", "));
}

// ---------------------------------------------------------------------------
// Native FLA/XFL scene import
//
// After XFLReader has fully parsed the document (including timelines),
// this function populates the active Flare xsheet:
//   - each DOMLayer  → TXshLevelColumn
//   - each DOMBitmapItem → PNG loaded via IoCmd::loadResources
//   - each DOMFrame span → cells set via TXsheet::setCell
//
// Approach:
//   1. Map libraryItemName → TFilePath from parsed BitmapItem list
//   2. Load each bitmap once using IoCmd::loadResources (expose=false)
//   3. Iterate layers (topmost XFL layer = leftmost Flare column)
//   4. For each keyframe with a BITMAP_INSTANCE element, set cells for its duration
//
// References:
//   jpexs-decompiler XFLConverter (GPL-3.0) — layer/frame/element model (ideas only)
//   fla-viewer (MIT) — attribute interpretation (ideas only)
//   OCA import (ocaio.cpp) — xsheet insertion pattern
// ---------------------------------------------------------------------------
static void importXFLScene(ToonzScene *scene, TXsheet *xsheet,
                            const XFL::Document &doc,
                            const TFilePath &xflBaseDir) {
    if (doc.timelines.empty()) return;

    // Build libraryItemName → absolute file path
    QMap<QString, TFilePath> bitmapPaths;
    for (const XFL::BitmapItem &bi : doc.bitmaps) {
        QString href = QString::fromStdString(bi.href);
        href.replace('\\', '/');
        TFilePath fp = xflBaseDir + TFilePath(href.toStdString());
        if (TSystem::doesExistFileOrLevel(fp))
            bitmapPaths[QString::fromStdString(bi.name)] = fp;
    }
    if (bitmapPaths.isEmpty()) return;

    // Load all referenced bitmaps; collect TXshSimpleLevel* per name.
    // IoCmd::loadResources(expose=false) → levels added to scene but no column auto-inserted.
    QMap<QString, TXshSimpleLevel *> bitmapLevels;
    for (auto it = bitmapPaths.constBegin(); it != bitmapPaths.constEnd(); ++it) {
        IoCmd::LoadResourceArguments args;
        args.expose = false;
        args.resourceDatas.emplace_back(it.value());
        IoCmd::loadResources(args);
        if (!args.loadedLevels.empty()) {
            TXshLevel *lv = *args.loadedLevels.begin();
            if (TXshSimpleLevel *sl = lv ? dynamic_cast<TXshSimpleLevel *>(lv) : nullptr)
                bitmapLevels[it.key()] = sl;
        }
    }
    if (bitmapLevels.isEmpty()) return;

    const XFL::XFLTimeline &tl = doc.timelines[0];

    // Iterate layers: XFL layers[0] = topmost visual layer → col 0 in Flare.
    for (const XFL::XFLLayer &layer : tl.layers) {
        if (layer.layerType == "guide" || layer.layerType == "folder") continue;

        // Skip layers with no bitmap elements
        bool hasCells = false;
        for (const XFL::XFLFrame &fr : layer.frames)
            if (!fr.elements.empty()) { hasCells = true; break; }
        if (!hasCells) continue;

        int col = xsheet->getFirstFreeColumnIndex();
        TXshLevelColumn *column = new TXshLevelColumn();
        xsheet->insertColumn(col, column);

        if (!layer.name.empty()) {
            TStageObject *obj = xsheet->getStageObject(TStageObjectId::ColumnId(col));
            if (obj) obj->setName(layer.name);
        }

        for (const XFL::XFLFrame &frame : layer.frames) {
            // Use only the first bitmap element per frame span
            for (const XFL::FrameElement &el : frame.elements) {
                if (el.type != XFL::FrameElement::BITMAP_INSTANCE) continue;
                TXshSimpleLevel *sl = bitmapLevels.value(
                    QString::fromStdString(el.libraryItemName), nullptr);
                if (!sl) continue;
                TXshCell cell(sl, TFrameId(1));  // PNG = single frame, id=1
                for (int r = frame.index; r < frame.index + frame.duration; ++r)
                    xsheet->setCell(r, col, cell);
                break;
            }
        }
    }

    xsheet->updateFrameCount();
    TApp::instance()->getCurrentLevel()->notifyLevelChange();
    TApp::instance()->getCurrentScene()->notifyCastChange();
    TApp::instance()->getCurrentXsheet()->notifyXsheetChanged();
}

} // anonymous namespace

// ---------------------------------------------------------------------------
// Command: Import Flash (FLA / XFL / SWC / SWF / AS) — fully native
// ---------------------------------------------------------------------------

class ImportFlashVectorCommand final : public MenuItemHandler {
public:
    ImportFlashVectorCommand() : MenuItemHandler(MI_ImportFlashVector) {}
    void execute() override;
} g_importFlashVectorCommand;

void ImportFlashVectorCommand::execute() {
    TApp *app = TApp::instance();
    TSceneHandle *sceneHandle = app->getCurrentScene();
    ToonzScene *scene = sceneHandle->getScene();

    static GenericLoadFilePopup *loadPopup = nullptr;
    if (!loadPopup) {
        loadPopup = new GenericLoadFilePopup(
            QObject::tr("Import Flash / Animate File"));
        // One list, owned by the format module, so the dialog and the dispatch
        // can never drift apart.
        for (const QString &ext : FlashAssets::supportedExtensions())
            loadPopup->addFilterType(ext);
    }

    if (!scene->isUntitled())
        loadPopup->setFolder(scene->getScenePath().getParentDir());
    else
        loadPopup->setFolder(
            TProjectManager::instance()->getCurrentProject()->getScenesPath());

    TFilePath fp = loadPopup->getPath();
    if (fp.isEmpty()) return;

    TFilePath outDir = makeTempImportDir("flare_flash_import_");
    QString   outPath = outDir.getQString();
    QString   srcPath = fp.getQString();
    QString   ext     = QString::fromStdString(fp.getType()).toLower();
    QStringList exported;
    QString info;

    // Identify the container from its leading bytes, falling back to the
    // extension only for formats that have no reliable magic number. This is
    // what lets a plain SWF named ".ssf", or an FLA re-zipped as ".zip", open
    // instead of being rejected as an unsupported format.
    const FlashAssets::Format detected = FlashAssets::detectFormat(srcPath);
    // Trust the bytes. Only fall back to the extension when the content is not
    // identifiable at all, so that a SWF named ".fla" still imports as a SWF.
    const bool zipLike =
        (detected == FlashAssets::Format::Zip) ||
        (detected == FlashAssets::Format::Unknown &&
         (ext == "fla" || ext == "swc" || ext == "zxp" || ext == "mxp" ||
          ext == "ane" || ext == "air" || ext == "oam" || ext == "zip" ||
          ext == "sol" || ext == "fls"));
    // ".swz" (pre-compressed sounds) and ".ksk" (keystroke-signed) carry the
    // ordinary FWS/CWS/ZWS header, so the sniffer already resolves them; the
    // extension is only consulted for content the magic number cannot place.
    const bool isSwf = (detected == FlashAssets::Format::Swf) ||
                       (detected == FlashAssets::Format::Unknown &&
                        (ext == "swf" || ext == "ssf" || ext == "dat" ||
                         ext == "swz" || ext == "ksk"));

    // ---- Legacy binary FLA (Flash CS4 and earlier; OLE2 compound document) ----
    if (detected == FlashAssets::Format::Ole2Fla) {
        QFile flaFile(srcPath);
        QStringList bitmaps;
        if (flaFile.open(QIODevice::ReadOnly)) {
            QByteArray flaData = flaFile.readAll();
            flaFile.close();
            bitmaps = FlashAssets::extractLegacyFlaBitmaps(flaData, outPath);
        }
        exported += bitmaps;
        info = QObject::tr(
            "Legacy binary FLA detected (Adobe Flash CS4 or earlier).\n"
            "Flare natively imports XFL-based FLAs (Animate / Flash CS5 and "
            "newer). Full timeline and symbol import for the older binary format "
            "is not supported yet — to import everything, open this file in Adobe "
            "Animate and re-save it as a CS5+ FLA or an uncompressed XFL.");
        if (!bitmaps.isEmpty())
            info += QObject::tr("\n  Recovered %1 embedded bitmap(s) from the file.")
                        .arg(bitmaps.size());
        else
            info += QObject::tr("\n  No embedded bitmaps could be recovered.");

    // ---- FLA / XFL / SWC : ZIP-based container ----
    } else if (zipLike) {

        if (!extractZip(srcPath, outPath)) {
            DVGui::error(
                QObject::tr("Failed to extract archive (not a readable ZIP): %1")
                    .arg(srcPath));
            return;
        }
        // Tell the user when the trailer had to be salvaged: the archive is
        // valid, but naive tools (and older Flare builds) rejected it.
        if (ext == "fla" || ext == "zip") {
            const TFilePathSet probe = TSystem::readDirectory(outPath, false, true, true);
            bool hasDomDocument = false;
            for (const auto &e : probe) hasDomDocument |= (e.getName() == "DOMDocument.xml");
            if (hasDomDocument)
                info = QObject::tr(
                    "Archive opened after repairing a stale ZIP trailer. "
                    "The file is valid; some other tools cannot read it.");
        }

        if (ext == "swc") {
            // SWC (Flex/Flash component library) format (Apache Flex SDK reference,
            // Apache License 2.0): ZIP containing catalog.xml + library.swf.
            //
            // catalog.xml structure:
            //   <swc xmlns="http://www.adobe.com/flash/swccatalog/9">
            //     <components>
            //       <component className="..." name="..." uri="..."/>
            //     </components>
            //   </swc>
            QFile catFile(outPath + "/catalog.xml");
            if (catFile.open(QIODevice::ReadOnly | QIODevice::Text)) {
                QString catXml = QString::fromUtf8(catFile.readAll());
                catFile.close();
                int compCount = 0;
                QStringList compNames;
                int searchPos = 0;
                while (true) {
                    int idx = catXml.indexOf("<component ", searchPos, Qt::CaseInsensitive);
                    if (idx < 0) break;
                    compCount++;
                    int nameIdx = catXml.indexOf("name=\"", idx);
                    if (nameIdx >= 0 && nameIdx < idx + 200) {
                        int nameStart = nameIdx + 6;
                        int nameEnd   = catXml.indexOf('"', nameStart);
                        if (nameEnd > nameStart)
                            compNames << catXml.mid(nameStart, nameEnd - nameStart);
                    }
                    searchPos = idx + 1;
                }
                info = QObject::tr("SWC: %1 component(s) exported").arg(compCount);
                if (!compNames.isEmpty())
                    info += "\n  " + compNames.join(", ");
            }

            // Extract bitmaps from the embedded library.swf
            QFile libSwf(outPath + "/library.swf");
            if (libSwf.open(QIODevice::ReadOnly)) {
                QByteArray swfData = libSwf.readAll();
                libSwf.close();
                // Use shared decompression helper with size cap
                QByteArray decompressed = FlashAssets::decompressCwsSwf(swfData);
                const QByteArray &src = decompressed.isEmpty() ? swfData : decompressed;
                QStringList bitmaps = FlashAssets::extractSwfBitmaps(src, outPath);
                exported += bitmaps;
                if (!bitmaps.isEmpty())
                    info += QObject::tr("\n  %1 bitmap(s) extracted from library.swf")
                            .arg(bitmaps.size());
            }

            // Also include any other extracted files (scripts, assets)
            {
                QDirIterator it(outPath, kAssetFilters,
                                QDir::Files | QDir::NoDotAndDotDot,
                                QDirIterator::Subdirectories);
                QDir base(outPath);
                while (it.hasNext()) {
                    it.next();
                    QString rel = base.relativeFilePath(it.filePath());
                    if (!exported.contains(rel)) exported << rel;
                }
            }
        } else {
            // For XFL/FLA: parse document structure and report
            TFilePath extractedXfl = outDir;
            if (!XFL::isXFLDirectory(extractedXfl)) {
                try {
                    TFilePathSet entries = TSystem::readDirectory(outDir, false, false, true);
                    for (const auto &e : entries)
                        if (XFL::isXFLDirectory(e)) { extractedXfl = e; break; }
                } catch (...) {}
            }
            if (XFL::isXFLDirectory(extractedXfl)) {
                XFL::Reader r2(extractedXfl);
                if (r2.read()) {
                    const XFL::Document &doc = r2.getDocument();
                    int tlCount = static_cast<int>(doc.timelines.size());
                    int bmCount = static_cast<int>(doc.bitmaps.size());
                    info = QObject::tr(
                        "Document: %1 × %2 px  |  %3 fps  |  %4 symbol(s)  |  %5 bitmap(s)  |  %6 timeline(s)")
                        .arg(doc.width).arg(doc.height)
                        .arg(doc.frameRate, 0, 'f', 1)
                        .arg(static_cast<int>(doc.symbols.size()))
                        .arg(bmCount)
                        .arg(tlCount);
                    // Native scene import: map FLA layers → Flare columns
                    info += describeUnconverted(doc.census);
                    if (tlCount > 0) {
                        TXsheet *xsheet = TApp::instance()->getCurrentXsheet()->getXsheet();
                        importXFLScene(scene, xsheet, doc, extractedXfl);
                    }
                } else {
                    QString err = QString::fromStdString(r2.getError());
                    info = QObject::tr("Failed to parse XFL metadata: %1").arg(err);
                    DVGui::warning(info);
                }
            }
            // Extract binary media from FLA's bin/ directory (.dat → PNG/JPG)
            QStringList binMedia = FlashAssets::extractFLABinaryMedia(outPath);
            exported += binMedia;
            if (!binMedia.isEmpty())
                info += QObject::tr("\n  %1 bitmap(s) extracted from FLA binary media")
                        .arg(binMedia.size());
            // Also look in subdirectories if the XFL was nested
            if (extractedXfl != outDir) {
                QStringList nestedMedia = FlashAssets::extractFLABinaryMedia(extractedXfl.getQString());
                for (const QString &m : nestedMedia) {
                    if (!exported.contains(m)) {
                        // Copy to output root for auto-load
                        QString src2 = extractedXfl.getQString() + "/" + m;
                        QString dst2 = outPath + "/" + m;
                        if (!QFile::exists(dst2)) QFile::copy(src2, dst2);
                        exported << m;
                    }
                }
            }
            {
                QDirIterator it(outPath, kAssetFilters,
                                QDir::Files | QDir::NoDotAndDotDot,
                                QDirIterator::Subdirectories);
                QDir base(outPath);
                while (it.hasNext()) { it.next(); exported << base.relativeFilePath(it.filePath()); }
            }
        }

    // ---- XFL directory or .xfl marker file ----
    } else if (ext == "xfl" || QFileInfo(srcPath).isDir()) {
        if (!QFileInfo(srcPath).exists()) {
            DVGui::error(QObject::tr("XFL path does not exist: %1").arg(srcPath));
            return;
        }
        // A directory-based XFL project is a FOLDER; when the user selects the
        // tiny "<name>.xfl" marker file inside it, the real project root is that
        // file's parent directory. Resolve it so asset copying and href lookups
        // use the folder, not the marker file — but only when the parent
        // actually looks like an XFL project (has DOMDocument.xml); otherwise
        // this is some other .xfl file and we keep treating srcPath itself as
        // the target, matching the pre-existing (non-marker) behavior.
        QFileInfo srcInfo(srcPath);
        TFilePath xflDir = fp;
        QString   xflDirPath = srcPath;
        if (srcInfo.isFile()) {
            TFilePath parentDir = fp.getParentDir();
            if (XFL::isXFLDirectory(parentDir)) {
                xflDir     = parentDir;
                xflDirPath = parentDir.getQString();
            }
        }

        XFL::Reader reader(xflDir);
        if (!reader.read()) {
            DVGui::error(QObject::tr("Failed to read XFL: %1").arg(reader.getError().c_str()));
            return;
        }
        const XFL::Document &doc = reader.getDocument();
        info = QObject::tr(
            "Document: %1 × %2 px  |  %3 fps  |  %4 symbol(s)  |  %5 bitmap(s)  |  %6 timeline(s)")
            .arg(doc.width).arg(doc.height)
            .arg(doc.frameRate, 0, 'f', 1)
            .arg(static_cast<int>(doc.symbols.size()))
            .arg(static_cast<int>(doc.bitmaps.size()))
            .arg(static_cast<int>(doc.timelines.size()));
        info += describeUnconverted(doc.census);

        // Native scene import for XFL directory
        if (!doc.timelines.empty()) {
            TXsheet *xsheet = TApp::instance()->getCurrentXsheet()->getXsheet();
            importXFLScene(scene, xsheet, doc, xflDir);
        }

        // Copy assets to output dir using recursive iteration
        {
            QDirIterator it(xflDirPath, kAssetFilters,
                            QDir::Files | QDir::NoDotAndDotDot,
                            QDirIterator::Subdirectories);
            QDir base(xflDirPath);
            while (it.hasNext()) {
                it.next();
                QString rel = base.relativeFilePath(it.filePath());
                QString dst = outPath + "/" + rel;
                QDir().mkpath(QFileInfo(dst).absolutePath());
                if (!QFile::copy(it.filePath(), dst)) continue;
                exported << rel;
            }
        }

    // ---- SWF binary: read header + extract embedded bitmaps ----
    } else if (isSwf) {
        SwfInfo swf = FlashAssets::readSwfHeader(srcPath);
        if (!swf.valid) {
            DVGui::error(QObject::tr("Not a valid SWF file: %1").arg(srcPath));
            return;
        }
        if (ext != "swf" && ext != "ssf")
            info = QObject::tr("File extension is .%1 but the content is SWF.")
                       .arg(ext.isEmpty() ? QStringLiteral("(none)") : ext);
        info = QObject::tr(
            "SWF v%1  |  %2 × %3 px  |  %4 fps  |  %5 frame(s)%6")
            .arg(swf.version)
            .arg(swf.width).arg(swf.height)
            .arg(swf.frameRate)
            .arg(swf.frameCount)
            .arg(swf.compressed ? QObject::tr("  [compressed]") : QString());

        // Read entire SWF and extract embedded bitmaps via tag scan.
        // For zlib-compressed SWF (CWS, version 6+), decompress body first.
        // Uses shared FlashAssets::decompressCwsSwf() helper with size cap
        // (approach consistent with lightspark and open-flash/swf-bitmap).
        QFile swfFile(srcPath);
        if (swfFile.open(QIODevice::ReadOnly)) {
            QByteArray swfData = swfFile.readAll();
            swfFile.close();
            QByteArray decompressed = FlashAssets::decompressCwsSwf(swfData);
            const QByteArray &src2 = decompressed.isEmpty() ? swfData : decompressed;
            QStringList bitmaps = FlashAssets::extractSwfBitmaps(src2, outPath);
            exported += bitmaps;
            if (!bitmaps.isEmpty())
                info += QObject::tr("\n  %1 embedded bitmap(s) extracted").arg(bitmaps.size());

            // Embedded audio. MP3 and raw PCM become playable files; ADPCM and
            // the proprietary codecs are written under an honest extension
            // rather than dropped.
            {
                const QStringList audio = FlashAssets::extractSwfAudio(src2, outPath);
                exported += audio;
                if (!audio.isEmpty())
                    info += QObject::tr("\n  %1 embedded sound(s) extracted")
                                .arg(audio.size());
            }

            // Name whatever the movie holds that cannot become a level.
            // Without this, a vector-only SWF reports a successful import that
            // produced nothing, which reads exactly like a broken file.
            {
                const FlashAssets::SwfContent c = FlashAssets::censusSwf(src2);
                QStringList missing;
                if (c.shapes)  missing << QObject::tr("%1 vector shape(s)").arg(c.shapes);
                if (c.texts)   missing << QObject::tr("%1 text object(s)").arg(c.texts);
                if (c.fonts)   missing << QObject::tr("%1 embedded font(s)").arg(c.fonts);
                if (c.video)   missing << QObject::tr("%1 video stream(s)").arg(c.video);
                if (c.sprites) missing << QObject::tr("%1 nested timeline(s)").arg(c.sprites);
                if (c.binary)  missing << QObject::tr("%1 embedded binary blob(s)").arg(c.binary);
                if (c.actions)
                    missing << QObject::tr("%1 ActionScript 1/2 block(s)").arg(c.actions);
                if (!missing.isEmpty())
                    info += QObject::tr(
                                "\n  Not converted to levels: %1. The movie is "
                                "unpacked to the export folder.")
                            .arg(missing.join(", "));
                if (c.abc)
                    info += QObject::tr("\n  %1 ActionScript 3 block(s) present.")
                                .arg(c.abc);
            }

            // ActionScript: needs the optional flare-as3 helper (Next2Flash
            // merge). Probed the same way FFmpeg is - when it is absent the
            // rest of the import is unaffected and AS3 is simply skipped.
            if (As3Bridge::isAvailable()) {
                const As3Bridge::Result as3 =
                    As3Bridge::decompile(fp, outDir);
                if (as3.ok && !as3.classes.isEmpty()) {
                    const QString asDir = outPath + "/as3";
                    QDir().mkpath(asDir);
                    QStringList asFiles;
                    QDirIterator it(asDir, QStringList{"*.as"}, QDir::Files,
                                    QDirIterator::Subdirectories);
                    while (it.hasNext()) {
                        it.next();
                        asFiles << "as3/" +
                                    QDir(asDir).relativeFilePath(it.filePath());
                    }
                    asFiles.sort();
                    exported += asFiles;
                    info += QObject::tr("\n  %1 ActionScript class(es) decompiled "
                                        "(%2)")
                                .arg(as3.classes.size()).arg(as3.blocks);
                } else if (!as3.error.isEmpty()) {
                    info += QObject::tr("\n  ActionScript not extracted: %1")
                                .arg(as3.error);
                }
            } else {
                info += QObject::tr("\n  ActionScript decompilation is unavailable "
                                    "(optional flare-as3 helper not installed).");
            }
        }

        // Always copy the SWF itself to output for reference
        QString dstSwf = outPath + "/" + QFileInfo(srcPath).fileName();
        if (QFile::copy(srcPath, dstSwf)) {
            QString fileName = QFileInfo(srcPath).fileName();
            if (!exported.contains(fileName))
                exported << fileName;
        } else {
            // If we can't copy SWF, continue with the rest of import rather than failing.
            qDebug() << "Warning: failed to copy SWF to" << dstSwf;
        }

    // ---- ActionScript source (.as) ----
    } else if (ext == "as") {
        copyFileForReference(srcPath, outPath, exported);
        info = QObject::tr("ActionScript source copied for reference.");

    // ---- ActionScript command script (.asc) ----
    } else if (ext == "asc") {
        copyFileForReference(srcPath, outPath, exported);
        info = QObject::tr("ActionScript command script (.asc) copied for reference.");

    // ---- MXML (Apache Flex / Royale UI definition) ----
    } else if (ext == "mxml") {
        copyFileForReference(srcPath, outPath, exported);
        info = QObject::tr("MXML (Flex UI) file copied for reference.");

    // ---- JSFL (Adobe Animate command script) ----
    } else if (ext == "jsfl") {
        QFile script(srcPath);
        QStringList functions;
        if (script.open(QIODevice::ReadOnly | QIODevice::Text)) {
            functions = findJsflFunctions(QString::fromUtf8(script.readAll()));
            script.close();
        }
        copyFileForReference(srcPath, outPath, exported);
        info = QObject::tr("JSFL script copied for inspection; scripts are not executed.");
        if (!functions.isEmpty())
            info += QObject::tr("\n  Functions: %1").arg(functions.join(", "));

    // ---- ZXP / MXP (Adobe extension packages) ----
    } else if (ext == "zxp" || ext == "mxp") {
        QString manifest = extractAdobeZipPackage(srcPath, outPath, ext);
        QDirIterator it(outPath, kAssetFilters, QDir::Files | QDir::NoDotAndDotDot,
                        QDirIterator::Subdirectories);
        QDir base(outPath);
        while (it.hasNext()) {
            it.next();
            exported << base.relativeFilePath(it.filePath());
        }
        info = manifest.isEmpty()
            ? QObject::tr("%1 extension package extracted (no manifest found).").arg(ext.toUpper())
            : QObject::tr("%1 extension package extracted; manifest: %2").arg(ext.toUpper(), manifest);
        info += QObject::tr("\n  Extension code is not executed by Flare.");

    // ---- FLV (Flash Video) ----
    } else if (ext == "flv") {
        FlvInfo flv = FlashAssets::readFlvHeader(srcPath);
        if (!flv.valid) {
            DVGui::error(QObject::tr("Not a valid FLV file: %1").arg(srcPath));
            return;
        }
        info = QObject::tr("FLV v%1  |  %2%3")
            .arg(flv.version)
            .arg(flv.hasVideo ? QObject::tr("video") : QString())
            .arg(flv.hasAudio ? QObject::tr(flv.hasVideo ? " + audio" : "audio") : QString());
        copyFileForReference(srcPath, outPath, exported);

    // ---- F4V / M4V (Flash H.264, ISO BMFF container) ----
    } else if (detected == FlashAssets::Format::IsoBmff) {
        const F4vInfo f4v = FlashAssets::readF4vHeader(srcPath);
        if (!f4v.valid) {
            DVGui::error(QObject::tr("Not a valid F4V/ISOBMFF file: %1").arg(srcPath));
            return;
        }
        info = QObject::tr("%1  |  brand: %2").arg(ext.toUpper(), f4v.majorBrand);
        if (!f4v.compatBrands.isEmpty())
            info += QObject::tr("  |  compatible: %1").arg(f4v.compatBrands);
        if (ext != "f4v")
            info += QObject::tr("\n  Extension is .%1 but the content is an "
                               "ISO base-media (MPEG-4) file.").arg(ext);
        copyFileForReference(srcPath, outPath, exported);

    // ---- ANE (Adobe Native Extension) — ZIP ----
    } else if (ext == "ane") {
        QString manifest = extractAdobeZipPackage(srcPath, outPath, "ane");
        info = manifest.isEmpty()
            ? QObject::tr("ANE: extracted (no extension.xml found)")
            : QObject::tr("ANE: extension manifest at %1").arg(manifest);
        QDirIterator it(outPath, kAssetFilters, QDir::Files | QDir::NoDotAndDotDot,
                        QDirIterator::Subdirectories);
        QDir base(outPath);
        while (it.hasNext()) { it.next(); exported << base.relativeFilePath(it.filePath()); }

    // ---- AIR (Adobe AIR application package) — ZIP ----
    } else if (ext == "air") {
        QString manifest = extractAdobeZipPackage(srcPath, outPath, "air");
        info = manifest.isEmpty()
            ? QObject::tr("AIR: extracted (no application.xml found)")
            : QObject::tr("AIR: application manifest at %1").arg(manifest);
        QDirIterator it(outPath, kAssetFilters, QDir::Files | QDir::NoDotAndDotDot,
                        QDirIterator::Subdirectories);
        QDir base(outPath);
        while (it.hasNext()) { it.next(); exported << base.relativeFilePath(it.filePath()); }

    // ---- OAM (Open Architecture Module) — ZIP ----
    } else if (ext == "oam") {
        QString manifest = extractAdobeZipPackage(srcPath, outPath, "oam");
        info = manifest.isEmpty()
            ? QObject::tr("OAM: extracted (no OAMMetadata.xml found)")
            : QObject::tr("OAM: metadata at %1").arg(manifest);
        QDirIterator it(outPath, kAssetFilters, QDir::Files | QDir::NoDotAndDotDot,
                        QDirIterator::Subdirectories);
        QDir base(outPath);
        while (it.hasNext()) { it.next(); exported << base.relativeFilePath(it.filePath()); }

    // ---- LWF (Lightweight SWF alternative) / RSL (Runtime Shared Library) /
    //      AFL (ActionScript Library, legacy) — copy for reference ----
    } else if (ext == "lwf" || ext == "rsl" || ext == "afl") {
        copyFileForReference(srcPath, outPath, exported);
        info = QObject::tr("%1 file copied for reference.")
                   .arg(ext.toUpper());

    } else {
        DVGui::warning(QObject::tr("Unsupported Flash format: .%1").arg(ext));
        return;
    }

    writeManifest(outPath, exported, srcPath);

    // SWF/FLV/F4V are never directly loadable as Flare levels — Flare has no
    // native level reader for these binary Flash formats.  Only the extracted
    // assets (PNG, JPG, SVG) can be auto-loaded into the scene.
    //
    // Implemented:  SWF images, SWF audio, XFL/FLA images + timeline/layers.
    // Not yet:       SWF vector art, text, embedded fonts, video streams and
    //                nested sprite timelines; the binary FLA timeline. The
    //                import dialog names what it found, so an empty result is
    //                always explained rather than looking like a broken file.
    if (isSwf)
        info += QObject::tr("\n  Embedded bitmaps and sounds extracted for import.");
    else if (ext == "flv")
        info += QObject::tr("\n  FLV copied for reference (no native FLV level reader).");
    else if (detected == FlashAssets::Format::IsoBmff)
        info += QObject::tr("\n  ISO base-media file copied for reference "
                            "(no native level reader).");

    if (detected == FlashAssets::Format::Ole2Fla)
        info += QObject::tr("\n  Note: legacy binary FLA timeline/vector import is "
                            "not supported yet; re-save as CS5+ or XFL for the full document.");
    else if (zipLike || ext == "xfl")
        info += QObject::tr("\n  Note: FLA/XFL import currently extracts bitmap media; "
                            "advanced timeline/vector/actionscript support is experimental.");

    // Auto-load only image assets that Flare can natively handle as levels.
    int imported = 0;
    {
        IoCmd::LoadResourceArguments args;
        for (const QString &rel : exported) {
            QString full = outPath + "/" + rel;
            QString e    = QFileInfo(full).suffix().toLower();
            // Only load image formats that Flare supports as levels
            if (e == "png" || e == "jpg" || e == "jpeg" || e == "svg") {
                args.resourceDatas.push_back(
                    IoCmd::LoadResourceArguments::ResourceData(TFilePath(full.toStdWString())));
            }
        }
        if (!args.resourceDatas.empty()) {
            imported = IoCmd::loadResources(args);
        }
    }

    QString msg = QObject::tr("Flash import complete.\n");
    if (!info.isEmpty()) msg += info + "\n";
    msg += QObject::tr("\n%1 file(s) exported to:\n%2").arg(exported.size()).arg(outPath);
    if (imported > 0)
        msg += QObject::tr("\n%1 asset(s) added to the scene.").arg(imported);

    std::vector<QString> btns = {QObject::tr("Open folder"), QObject::tr("Save as FLA"), QObject::tr("OK")};
    int ret = DVGui::MsgBox(DVGui::INFORMATION, msg, btns);
    if (ret == 1) {
      openFolder(outPath);
    } else if (ret == 2 && (zipLike || ext == "xfl")) {
      QString savePath = QFileDialog::getSaveFileName(nullptr,
          QObject::tr("Save as FLA"), outDir.getQString(),
          QObject::tr("Adobe FLA files (*.fla)"));
      if (!savePath.isEmpty()) {
        TFilePath sourceDir;
        if (ext == "xfl" && QFileInfo(srcPath).isDir())
          sourceDir = fp;
        else
          sourceDir = outDir;

        if (XFL::writeFLA(sourceDir, TFilePath(savePath.toStdWString()))) {
          DVGui::info(QObject::tr("Saved FLA successfully: %1").arg(savePath));
        } else {
          DVGui::error(QObject::tr("Failed to save FLA: %1").arg(savePath));
        }
      }
    }
}
