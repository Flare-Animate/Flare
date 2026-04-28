// flashimport.cpp (flare_legacy) — Native Flash / Adobe Animate format import.
//
// Supported formats:
//   FLA  — ZIP archive of an XFL project (DOMDocument.xml + LIBRARY/)
//   XFL  — Uncompressed XFL project directory or ZIP
//   SWF  — Compiled Flash binary (tag stream)
//   SWC  — Flex/Flash component library (ZIP: catalog.xml + library.swf)
//   FLV  — Flash Video container
//   F4V  — Flash H.264 video (ISO BMFF)
//   AS   — ActionScript source (1.0/2.0/3.0)
//   ASC  — ActionScript command script
//   MXML — Apache Flex / Royale UI definition (XML)
//   ANE  — Adobe Native Extension (ZIP)
//   AIR  — Adobe AIR application package (ZIP)
//   OAM  — Open Architecture Module (ZIP)
//   LWF  — Lightweight SWF alternative (binary, reference copy)
//   RSL  — Runtime Shared Library (SWF-based; reference copy)
//   AFL  — ActionScript library, legacy (reference copy)
//
// Format knowledge (ideas only, no code copied):
//   Adobe XFL spec (public)             — DOMDocument XML structure
//   Adobe SWF spec (public)             — SWF header / RECT / tag format
//   Adobe FLV/F4V spec (public)         — FLV signature, F4V ftyp box
//   Adobe ANE SDK reference             — META-INF/ANE/extension.xml layout
//   Adobe AIR SDK reference             — META-INF/AIR/application.xml layout
//   Adobe OAM spec                      — OAMMetadata.xml layout
//   ruffle (MIT/Apache-2.0)             — SWF format constants / tag codes
//   jpexs-decompiler (GPL-3.0)          — XFL timeline / layer / frame model
//   fla-viewer (MIT)                    — XML attribute interpretation
//   open-flash/swf-bitmap (ISC)         — bitmap format byte values
//   lightspark (LGPL-3.0)               — CWS decompression approach
//   Apache Flex SDK (Apache-2.0)        — SWC catalog.xml structure

#include "menubarcommandids.h"
#include "menubar.h"
#include "tapp.h"
#include "flare/tscenehandle.h"
#include "flare/txsheethandle.h"
#include "flare/txshlevelhandle.h"
#include "flare/toonzscene.h"
#include "flare/tproject.h"
#include "flare/txsheet.h"
#include "flare/txshcell.h"
#include "flare/txshsimplelevel.h"
#include "flare/txshlevelcolumn.h"
#include "flare/tstageobject.h"

#include "flareqt/gutil.h"
#include "flareqt/dvdialog.h"
#include "filebrowserpopup.h"
#include "iocommand.h"
#include "tsystem.h"
#include "tfilepath.h"

#include "XFLReader.h"
#include "unzip.h"

#include <QFile>
#include <QDir>
#include <QDirIterator>
#include <QDateTime>
#include <QFileInfo>
#include <QDesktopServices>
#include <QRegularExpression>
#include <QUrl>
#include <cstring>

using namespace DVGui;

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------
namespace {

static TFilePath makeTempImportDir(const QString &prefix) {
    QString name = prefix + QString::number(QDateTime::currentMSecsSinceEpoch());
    TFilePath dir = TSystem::getTempDir() + TFilePath(name.toStdString());
    try { TSystem::mkDir(dir); } catch (...) {}
    return dir;
}

static const QStringList kAssetFilters = {
    "*.png", "*.jpg", "*.jpeg", "*.svg", "*.xml", "*.as"
};

// Zip Slip guard: reject paths that escape the target directory.
static bool isPathUnderDir(const QString &dir, const QString &candidate) {
    QString canonical = QFileInfo(candidate).canonicalFilePath();
    if (canonical.isEmpty()) canonical = QFileInfo(candidate).absoluteFilePath();
    QString base = QDir(dir).absolutePath();
    if (!base.endsWith('/')) base += '/';
    return canonical.startsWith(base);
}

// Extract a ZIP archive to outDir using the bundled minizip.
// Returns false only on hard open failure; partial extraction is non-fatal.
static bool extractZip(const QString &zipPath, const QString &outDir) {
    unzFile uf = unzOpen(zipPath.toUtf8().constData());
    if (!uf) return false;

    unz_global_info gi;
    if (unzGetGlobalInfo(uf, &gi) != UNZ_OK) { unzClose(uf); return false; }

    char entryName[1024]; char buf[16384];
    for (uLong i = 0; i < gi.number_entry; i++) {
        unz_file_info fi;
        if (unzGetCurrentFileInfo(uf, &fi, entryName, sizeof(entryName),
                                  nullptr, 0, nullptr, 0) != UNZ_OK) break;
        size_t nameLen = strlen(entryName);
        if (nameLen == 0) { if (i+1 < gi.number_entry) unzGoToNextFile(uf); continue; }

        QString entry = QString::fromUtf8(entryName);
        entry.replace('\\', '/');
        while (entry.startsWith("./")) entry = entry.mid(2);
        while (entry.startsWith('/')) entry = entry.mid(1);

        // Zip Slip: reject absolute paths and directory traversal
        if (entry.startsWith('/') || entry.contains("../") || entry.endsWith("..") ||
            (entry.size() >= 2 && entry[1] == ':')) {
            if (i+1 < gi.number_entry) unzGoToNextFile(uf);
            continue;
        }
        QString fullOut = outDir + "/" + entry;
        if (!isPathUnderDir(outDir, fullOut)) {
            if (i+1 < gi.number_entry) unzGoToNextFile(uf);
            continue;
        }

        if (entryName[nameLen-1] == '/') {
            QDir().mkpath(fullOut);
        } else {
            QDir().mkpath(QFileInfo(fullOut).absolutePath());
            if (unzOpenCurrentFile(uf) == UNZ_OK) {
                QFile outFile(fullOut);
                if (outFile.open(QIODevice::WriteOnly)) {
                    int n;
                    while ((n = unzReadCurrentFile(uf, buf, sizeof(buf))) > 0)
                        outFile.write(buf, n);
                    outFile.close();
                }
                unzCloseCurrentFile(uf);
            }
        }
        if (i+1 < gi.number_entry && unzGoToNextFile(uf) != UNZ_OK) break;
    }
    unzClose(uf);
    return true;
}

// Detect and rename FLA binary media (.dat files in bin/) by magic bytes.
// JPEG: FF D8 FF;  PNG: 89 50 4E 47;  GIF: 47 49 46 38
// Reference: Adobe XFL spec; open-flash/swf-bitmap approach.
static QStringList extractFLABinaryMedia(const QString &outDir) {
    QStringList extracted;
    QDir bin(outDir + "/bin");
    if (!bin.exists()) return extracted;
    int idx = 0;
    QDirIterator it(bin.path(), {"*.dat"}, QDir::Files);
    while (it.hasNext()) {
        it.next();
        QFile f(it.filePath());
        if (!f.open(QIODevice::ReadOnly)) continue;
        QByteArray hdr = f.read(8); f.close();
        if (hdr.size() < 4) continue;
        const auto *h = reinterpret_cast<const unsigned char *>(hdr.constData());
        QString ext;
        if      (h[0]==0xFF && h[1]==0xD8 && h[2]==0xFF) ext = "jpg";
        else if (h[0]==0x89 && h[1]==0x50 && h[2]==0x4E && h[3]==0x47) ext = "png";
        else if (h[0]==0x47 && h[1]==0x49 && h[2]==0x46 && h[3]==0x38) ext = "gif";
        else continue;
        QString fname = QString("media_%1.%2").arg(idx++, 4, 10, QChar('0')).arg(ext);
        QFile::copy(it.filePath(), outDir + "/" + fname);
        extracted << fname;
    }
    return extracted;
}

// Copy a single file to outDir for reference (ActionScript, MXML, etc.).
static void copyFileForReference(const QString &src, const QString &outDir,
                                  QStringList &exported) {
    QString fname = QFileInfo(src).fileName();
    QFile::copy(src, outDir + "/" + fname);
    exported << fname;
}

// Extract an Adobe ZIP-based package (ANE/AIR/OAM) and return the manifest path if found.
// Reference: Adobe ANE SDK reference, AIR SDK reference, Adobe OAM spec.
static QString extractAdobeZipPackage(const QString &srcPath, const QString &outDir,
                                       const QString &ext) {
    if (!extractZip(srcPath, outDir)) return {};
    QStringList candidates;
    if      (ext == "ane") { candidates << "META-INF/ANE/extension.xml" << "META-INF/extension.xml"; }
    else if (ext == "air") { candidates << "META-INF/AIR/application.xml" << "META-INF/MANIFEST.MF"; }
    else if (ext == "oam") { candidates << "OAMMetadata.xml" << "META-INF/OAM/metadata.xml" << "metadata.xml"; }
    for (const QString &rel : candidates)
        if (QFile::exists(outDir + "/" + rel)) return rel;
    return {};
}

// ---------------------------------------------------------------------------
// SWF header reader
//
// SWF binary format (Adobe SWF specification, public):
//   Bytes 0-2:  signature ("FWS"=uncompressed, "CWS"=zlib, "ZWS"=lzma)
//   Byte  3:    SWF version
//   Bytes 4-7:  file length (uint32 little-endian)
//   [CWS/ZWS: compressed body follows; FWS: RECT + frame_rate + frame_count]
//
// RECT encoding: packed bit stream (Nbits in high 5 bits of first byte, then
//   4 signed values of Nbits each); values are in twips (1/20 pixel).
//
// Referenced from: ruffle swf/src/read.rs (MIT) — approach only, no code copied.
// ---------------------------------------------------------------------------
struct SwfInfo { bool valid=false,compressed=false; int version=0,width=0,height=0,frameRate=0,frameCount=0; };

static SwfInfo readSwfHeader(const QString &path) {
    SwfInfo i;
    QFile f(path); if (!f.open(QIODevice::ReadOnly)) return i;
    QByteArray d = f.read(9); if (d.size() < 4) return i;
    auto b = [&](int x){ return static_cast<unsigned char>(d[x]); };
    if ((b(0)!='F'&&b(0)!='C'&&b(0)!='Z')||b(1)!='W'||b(2)!='S') return i;
    i.valid=true; i.compressed=(b(0)=='C'||b(0)=='Z'); i.version=b(3);
    if (!i.compressed && d.size()>=9) {
        QByteArray rest=f.read(256); d+=rest;
        int off=8; if (off<d.size()) {
            int nb=b(8)>>3, tb=5+4*nb, by=(tb+7)/8;
            if (off+by+4<=d.size()) {
                int bp=off*8+5;
                auto rb=[&](int n)->int{
                    int v=0;
                    for(int x=0;x<n;x++){int bi2=bp/8,bx=7-(bp%8);
                        if(bi2<d.size())v=(v<<1)|((b(bi2)>>bx)&1);else v<<=1;++bp;}
                    return v;};
                auto rs=[&](int n)->int{int v=rb(n);if(v&(1<<(n-1)))v-=(1<<n);return v;};
                int xmin=rs(nb),xmax=rs(nb),ymin=rs(nb),ymax=rs(nb);
                i.width=(xmax-xmin)/20; i.height=(ymax-ymin)/20;
                int ar=(bp+7)/8;
                if(ar+4<=d.size()){i.frameRate=b(ar+1);i.frameCount=b(ar+2)|(b(ar+3)<<8);}
            }
        }
    }
    return i;
}

// FLV header: "FLV" + version(1) + flags(1) + headerSize(4 BE)
// Reference: Adobe FLV specification (public).
struct FlvInfo { bool valid=false; int version=0; bool hasVideo=false,hasAudio=false; };
static FlvInfo readFlvHeader(const QString &path) {
    FlvInfo i; QFile f(path); if (!f.open(QIODevice::ReadOnly)) return i;
    QByteArray h=f.read(9); if (h.size()<9) return i;
    if ((unsigned char)h[0]!='F'||(unsigned char)h[1]!='L'||(unsigned char)h[2]!='V') return i;
    i.valid=true; i.version=(unsigned char)h[3];
    unsigned char fl=(unsigned char)h[4]; i.hasVideo=(fl&0x01)!=0; i.hasAudio=(fl&0x04)!=0;
    return i;
}

// F4V / ISO BMFF ftyp box: size(4) + "ftyp"(4) + majorBrand(4) + minorVer(4) + compatBrands
// Reference: ISO 14496-12 (public), Adobe F4V specification.
struct F4vInfo { bool valid=false; QString majorBrand,compatBrands; };
static F4vInfo readF4vHeader(const QString &path) {
    F4vInfo i; QFile f(path); if (!f.open(QIODevice::ReadOnly)) return i;
    QByteArray h=f.read(32); if (h.size()<12) return i;
    if (h[4]!='f'||h[5]!='t'||h[6]!='y'||h[7]!='p') return i;
    i.valid=true; i.majorBrand=QString::fromLatin1(h.mid(8,4)).trimmed();
    QStringList bb; for(int o=16;o+4<=h.size();o+=4){auto b=QString::fromLatin1(h.mid(o,4)).trimmed();if(!b.isEmpty())bb<<b;}
    i.compatBrands=bb.join(", "); return i;
}

static void writeManifest(const QString &outDir, const QStringList &files, const QString &src) {
    QFile mf(outDir+"/manifest.txt"); if(!mf.open(QIODevice::WriteOnly|QIODevice::Text)) return;
    mf.write("Source: "+src.toUtf8()+"\nExported files:\n");
    for(const auto &f:files) mf.write("  "+f.toUtf8()+"\n");
}

static void openFolder(const QString &path) {
    QDesktopServices::openUrl(QUrl::fromLocalFile(path));
}

// ---------------------------------------------------------------------------
// Native FLA/XFL scene import
//
// Maps XFL timeline → Flare xsheet:
//   DOMLayer        → TXshLevelColumn
//   DOMBitmapItem   → TXshSimpleLevel (PNG loaded via IoCmd::loadResources)
//   DOMFrame span   → TXsheet::setCell for the duration
//
// References (ideas only, no code copied):
//   jpexs-decompiler (GPL-3.0)   — layer/frame/element model understanding
//   fla-viewer (MIT)             — DOMBitmapInstance libraryItemName attribute
//   ocaio.cpp (Flare, MIT)       — xsheet insertion pattern used here
// ---------------------------------------------------------------------------
static void importXFLScene(ToonzScene *scene, TXsheet *xsheet,
                            const XFL::Document &doc,
                            const TFilePath &xflBaseDir) {
    if (doc.timelines.empty()) return;

    // Build libraryItemName → absolute path map for bitmap assets
    QMap<QString, TFilePath> bitmapPaths;
    for (const XFL::BitmapItem &bi : doc.bitmaps) {
        QString href = QString::fromStdString(bi.href);
        href.replace('\\', '/');
        TFilePath fp = xflBaseDir + TFilePath(href.toStdString());
        if (TSystem::doesExistFileOrLevel(fp))
            bitmapPaths[QString::fromStdString(bi.name)] = fp;
    }

    // Load each bitmap; collect TXshSimpleLevel*
    QMap<QString, TXshSimpleLevel *> bitmapLevels;
    for (auto it = bitmapPaths.constBegin(); it != bitmapPaths.constEnd(); ++it) {
        IoCmd::LoadResourceArguments args;
        args.expose = false;
        args.resourceDatas.emplace_back(it.value());
        IoCmd::loadResources(args);
        if (!args.loadedLevels.empty()) {
            if (TXshSimpleLevel *sl = dynamic_cast<TXshSimpleLevel *>(*args.loadedLevels.begin()))
                bitmapLevels[it.key()] = sl;
        }
    }

    const XFL::XFLTimeline &tl = doc.timelines[0];
    int layersImported = 0;

    for (const XFL::XFLLayer &layer : tl.layers) {
        // Skip non-drawable layer types
        if (layer.layerType == "guide" || layer.layerType == "folder") continue;

        // Count useful frames (bitmap or symbol instances)
        bool hasBitmapCells = false;
        for (const XFL::XFLFrame &fr : layer.frames) {
            for (const XFL::FrameElement &el : fr.elements) {
                if (el.type == XFL::FrameElement::BITMAP_INSTANCE &&
                    bitmapLevels.contains(QString::fromStdString(el.libraryItemName)))
                    hasBitmapCells = true;
            }
        }
        // Skip layers with no frames at all
        if (layer.frames.empty()) continue;

        int col = xsheet->getFirstFreeColumnIndex();
        TXshLevelColumn *column = new TXshLevelColumn();
        xsheet->insertColumn(col, column);
        if (!layer.name.empty()) {
            TStageObject *obj = xsheet->getStageObject(TStageObjectId::ColumnId(col));
            if (obj) obj->setName(layer.name);
        }
        ++layersImported;

        if (!hasBitmapCells) {
            // No bitmap data available for this layer (vector/symbol layer):
            // expose a blank note-level placeholder spanning the whole layer
            // so the user can see the layer structure without losing timeline info.
            continue;
        }

        for (const XFL::XFLFrame &frame : layer.frames) {
            for (const XFL::FrameElement &el : frame.elements) {
                if (el.type != XFL::FrameElement::BITMAP_INSTANCE) continue;
                TXshSimpleLevel *sl = bitmapLevels.value(
                    QString::fromStdString(el.libraryItemName), nullptr);
                if (!sl) continue;
                TXshCell cell(sl, TFrameId(1));
                for (int r = frame.index; r < frame.index + frame.duration; ++r)
                    xsheet->setCell(r, col, cell);
                break;
            }
        }
    }

    if (layersImported > 0) {
        xsheet->updateFrameCount();
        TApp::instance()->getCurrentLevel()->notifyLevelChange();
        TApp::instance()->getCurrentScene()->notifyCastChange();
        TApp::instance()->getCurrentXsheet()->notifyXsheetChanged();
    }
}

} // namespace

// ---------------------------------------------------------------------------
// Command handler
// ---------------------------------------------------------------------------

class ImportFlashVectorCommand final : public MenuItemHandler {
public:
    ImportFlashVectorCommand() : MenuItemHandler(MI_ImportFlashVector) {}
    void execute() override;
} g_importFlashVectorLegacy;

void ImportFlashVectorCommand::execute() {
    TApp *app = TApp::instance();
    ToonzScene *scene = app->getCurrentScene()->getScene();

    static GenericLoadFilePopup *loadPopup = nullptr;
    if (!loadPopup) {
        loadPopup = new GenericLoadFilePopup(
            QObject::tr("Import Flash / Animate File"));
        loadPopup->addFilterType("fla");
        loadPopup->addFilterType("xfl");
        loadPopup->addFilterType("swf");
        loadPopup->addFilterType("swc");
        loadPopup->addFilterType("as");
        loadPopup->addFilterType("asc");
        loadPopup->addFilterType("mxml");
        loadPopup->addFilterType("flv");
        loadPopup->addFilterType("f4v");
        loadPopup->addFilterType("air");
        loadPopup->addFilterType("ane");
        loadPopup->addFilterType("oam");
        loadPopup->addFilterType("lwf");
        loadPopup->addFilterType("rsl");
        loadPopup->addFilterType("afl");
        // Adobe extension / plugin formats (issue #52)
        loadPopup->addFilterType("zxp");   // Adobe Extension (CC), ZIP-based
        loadPopup->addFilterType("mxp");   // Adobe Extension Manager package (legacy)
        loadPopup->addFilterType("jsfl");  // JavaScript Flash (JSFL) script
        loadPopup->addFilterType("csx");   // CEP extension (JSON manifest + web content)
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

    // ---- FLA / XFL (ZIP) / SWC / ANE / AIR / OAM -------------------------
    if (ext == "fla" || ext == "swc" ||
        (ext == "xfl" && XFL::isFLAZipBased(fp)) ||
        ext == "ane" || ext == "air" || ext == "oam") {

        if (!extractZip(srcPath, outPath)) {
            DVGui::error(QObject::tr("Failed to extract archive: %1").arg(srcPath));
            return;
        }

        if (ext == "swc") {
            // SWC (Apache Flex SDK, Apache-2.0): ZIP with catalog.xml + library.swf
            // catalog.xml structure: <swc><components><component className="..." name="..."/>
            QFile cat(outPath + "/catalog.xml");
            if (cat.open(QIODevice::ReadOnly | QIODevice::Text)) {
                QString xml = QString::fromUtf8(cat.readAll()); cat.close();
                int n = 0; int pos = 0;
                while ((pos = xml.indexOf("<component ", pos, Qt::CaseInsensitive)) >= 0) { ++n; ++pos; }
                info = QObject::tr("SWC: %1 component(s)").arg(n);
            }
        } else if (ext == "ane") {
            QString mf = extractAdobeZipPackage(srcPath, outPath, "ane");
            info = mf.isEmpty() ? QObject::tr("ANE: extracted")
                                : QObject::tr("ANE: manifest at %1").arg(mf);
        } else if (ext == "air") {
            QString mf = extractAdobeZipPackage(srcPath, outPath, "air");
            info = mf.isEmpty() ? QObject::tr("AIR: extracted")
                                : QObject::tr("AIR: manifest at %1").arg(mf);
        } else if (ext == "oam") {
            QString mf = extractAdobeZipPackage(srcPath, outPath, "oam");
            info = mf.isEmpty() ? QObject::tr("OAM: extracted")
                                : QObject::tr("OAM: metadata at %1").arg(mf);
        } else {
            // FLA / XFL ZIP: parse timeline + extract binary media
            TFilePath xflDir = outDir;
            if (!XFL::isXFLDirectory(xflDir)) {
                try {
                    for (const auto &e : TSystem::readDirectory(outDir, false, false, true))
                        if (XFL::isXFLDirectory(e)) { xflDir = e; break; }
                } catch (...) {}
            }
            if (XFL::isXFLDirectory(xflDir)) {
                XFL::Reader r(xflDir);
                if (r.read()) {
                    const XFL::Document &doc = r.getDocument();
                    info = QObject::tr(
                        "Document: %1 × %2 px  |  %3 fps  |  %4 symbol(s)  |  %5 bitmap(s)  |  %6 timeline(s)")
                        .arg(doc.width).arg(doc.height)
                        .arg(doc.frameRate, 0, 'f', 1)
                        .arg((int)doc.symbols.size())
                        .arg((int)doc.bitmaps.size())
                        .arg((int)doc.timelines.size());
                    if (!doc.timelines.empty()) {
                        TXsheet *xsheet = app->getCurrentXsheet()->getXsheet();
                        importXFLScene(scene, xsheet, doc, xflDir);
                    }
                }
            }
            QStringList bin = extractFLABinaryMedia(outPath);
            exported += bin;
            if (!bin.isEmpty())
                info += QObject::tr("\n  %1 bitmap(s) from FLA binary media").arg(bin.size());
        }

        QDirIterator it(outPath, kAssetFilters,
                        QDir::Files | QDir::NoDotAndDotDot,
                        QDirIterator::Subdirectories);
        QDir base(outPath);
        while (it.hasNext()) { it.next(); QString r = base.relativeFilePath(it.filePath()); if (!exported.contains(r)) exported << r; }

    // ---- XFL directory ----------------------------------------------------
    } else if (ext == "xfl" || (ext.isEmpty() && QFileInfo(srcPath).isDir())) {
        XFL::Reader reader(fp);
        if (!reader.read()) {
            DVGui::error(QObject::tr("Failed to read XFL: %1").arg(reader.getError().c_str()));
            return;
        }
        const XFL::Document &doc = reader.getDocument();
        info = QObject::tr("Document: %1 × %2 px  |  %3 fps  |  %4 symbol(s)  |  %5 bitmap(s)")
            .arg(doc.width).arg(doc.height)
            .arg(doc.frameRate, 0, 'f', 1)
            .arg((int)doc.symbols.size())
            .arg((int)doc.bitmaps.size());
        if (!doc.timelines.empty()) {
            TXsheet *xsheet = app->getCurrentXsheet()->getXsheet();
            importXFLScene(scene, xsheet, doc, fp);
        }
        QDirIterator it(srcPath, kAssetFilters, QDir::Files | QDir::NoDotAndDotDot,
                        QDirIterator::Subdirectories);
        QDir base(srcPath);
        while (it.hasNext()) {
            it.next();
            QString rel = base.relativeFilePath(it.filePath());
            QString dst = outPath + "/" + rel;
            QDir().mkpath(QFileInfo(dst).absolutePath());
            QFile::copy(it.filePath(), dst);
            exported << rel;
        }

    // ---- SWF binary -------------------------------------------------------
    } else if (ext == "swf") {
        SwfInfo swf = readSwfHeader(srcPath);
        if (!swf.valid) {
            DVGui::error(QObject::tr("Not a valid SWF file: %1").arg(srcPath));
            return;
        }
        info = QObject::tr("SWF v%1  |  %2 × %3 px  |  %4 fps  |  %5 frame(s)%6")
            .arg(swf.version).arg(swf.width).arg(swf.height).arg(swf.frameRate)
            .arg(swf.frameCount)
            .arg(swf.compressed ? QObject::tr("  [compressed]") : QString());
        copyFileForReference(srcPath, outPath, exported);

    // ---- ActionScript (.as / .asc) / MXML ---------------------------------
    } else if (ext == "as" || ext == "asc" || ext == "mxml") {
        copyFileForReference(srcPath, outPath, exported);
        info = QObject::tr("%1 file copied for reference.").arg(ext.toUpper());

    // ---- FLV (Flash Video) ------------------------------------------------
    } else if (ext == "flv") {
        FlvInfo flv = readFlvHeader(srcPath);
        if (!flv.valid) {
            DVGui::error(QObject::tr("Not a valid FLV file: %1").arg(srcPath));
            return;
        }
        info = QObject::tr("FLV v%1  |  %2%3")
            .arg(flv.version)
            .arg(flv.hasVideo ? QObject::tr("video") : QString())
            .arg(flv.hasAudio ? QObject::tr(flv.hasVideo ? " + audio" : "audio") : QString());
        copyFileForReference(srcPath, outPath, exported);

    // ---- F4V (Flash H.264 / ISO BMFF) ------------------------------------
    } else if (ext == "f4v") {
        F4vInfo f4v = readF4vHeader(srcPath);
        if (!f4v.valid) {
            DVGui::error(QObject::tr("Not a valid F4V/ISOBMFF file: %1").arg(srcPath));
            return;
        }
        info = QObject::tr("F4V  |  brand: %1%2").arg(f4v.majorBrand)
                   .arg(f4v.compatBrands.isEmpty() ? QString()
                        : QObject::tr("  |  compatible: %1").arg(f4v.compatBrands));
        copyFileForReference(srcPath, outPath, exported);

    // ---- LWF / RSL / AFL — binary reference copy -------------------------
    } else if (ext == "lwf" || ext == "rsl" || ext == "afl") {
        copyFileForReference(srcPath, outPath, exported);
        info = QObject::tr("%1 file copied for reference.").arg(ext.toUpper());

    // ---- ZXP / MXP — Adobe Extension packages (ZIP-based) ----------------
    } else if (ext == "zxp" || ext == "mxp") {
        // ZXP (Creative Cloud) and MXP (Extension Manager) are ZIP archives.
        // Extract contents and copy for reference; show manifest summary.
        if (!extractZip(srcPath, outPath)) {
            DVGui::error(QObject::tr("Failed to extract extension package: %1").arg(srcPath));
            return;
        }
        // Try to find manifest for info display
        QString manifest;
        for (const QString &mf : QStringList{"manifest.xml", "ExtensionManifest.xml", "Manifest.xml"}) {
            QFile f(outPath + "/" + mf);
            if (f.open(QIODevice::ReadOnly | QIODevice::Text)) {
                manifest = QString::fromUtf8(f.readAll()).left(800);
                f.close();
                break;
            }
        }
        info = QObject::tr("%1 extension extracted.").arg(ext.toUpper());
        if (!manifest.isEmpty())
            info += "\n" + manifest.left(300);
        QDirIterator it(outPath, QDir::Files | QDir::NoDotAndDotDot, QDirIterator::Subdirectories);
        QDir base(outPath);
        while (it.hasNext()) {
            it.next();
            exported << base.relativeFilePath(it.filePath());
        }

    // ---- JSFL — JavaScript Flash script (plain text) ----------------------
    } else if (ext == "jsfl") {
        copyFileForReference(srcPath, outPath, exported);
        // Parse function names and JSFL API calls for informative summary
        QFile f(srcPath);
        if (f.open(QIODevice::ReadOnly | QIODevice::Text)) {
            QString src = QString::fromUtf8(f.readAll()); f.close();
            // Extract named function declarations (issue #11, #52)
            QStringList funcNames;
            QRegularExpression funcRe(R"(\bfunction\s+([A-Za-z_$][A-Za-z0-9_$]*)\s*\()");
            QRegularExpressionMatchIterator it = funcRe.globalMatch(src);
            while (it.hasNext()) funcNames << it.next().captured(1);
            // Detect JSFL API root objects
            QStringList apis;
            for (const char *api : {"fl.", "doc.", "timeline.", "layer.", "item.", "dom."}) {
                if (src.contains(QLatin1String(api))) apis << QString(api).chopped(1);
            }
            info = QObject::tr("JSFL script: %1 function(s)").arg(funcNames.size());
            if (!funcNames.isEmpty())
                info += QObject::tr("\n  Functions: ") + funcNames.join(", ");
            if (!apis.isEmpty())
                info += QObject::tr("\n  JSFL APIs: ") + apis.join(", ");
            if (funcNames.isEmpty() && apis.isEmpty())
                info += "\n" + src.left(300);
        } else {
            info = QObject::tr("JSFL script copied for reference.");
        }

    // ---- CSX — CEP extension (JSON/HTML/JS panel) -------------------------
    } else if (ext == "csx") {
        copyFileForReference(srcPath, outPath, exported);
        info = QObject::tr("CSX extension copied for reference.");

    } else {
        DVGui::warning(QObject::tr("Unsupported Flash format: .%1").arg(ext));
        return;
    }

    writeManifest(outPath, exported, srcPath);

    // Auto-load image assets that Flare can handle as levels.
    int imported = 0;
    {
        IoCmd::LoadResourceArguments args;
        for (const QString &rel : exported) {
            QString e = QFileInfo(rel).suffix().toLower();
            if (e == "png" || e == "jpg" || e == "jpeg" || e == "svg")
                args.resourceDatas.emplace_back(TFilePath((outPath + "/" + rel).toStdWString()));
        }
        if (!args.resourceDatas.empty())
            imported = IoCmd::loadResources(args);
    }

    QString msg = QObject::tr("Flash import complete.\n");
    if (!info.isEmpty()) msg += info + "\n";
    msg += QObject::tr("\n%1 file(s) exported to:\n%2").arg(exported.size()).arg(outPath);
    if (imported > 0)
        msg += QObject::tr("\n%1 asset(s) added to the scene.").arg(imported);

    std::vector<QString> btns = {QObject::tr("Open folder"), QObject::tr("OK")};
    int ret = DVGui::MsgBox(DVGui::INFORMATION, msg, btns);
    if (ret == 1) openFolder(outPath);
}
