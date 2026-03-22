// flashimport.cpp - Native built-in Flash format import (FLA, XFL, SWF, SWC, AS)
// No external tools or third-party processes required.
//
// FLA  = ZIP archive containing an XFL document (DOMDocument.xml + assets)
// XFL  = Unzipped FLA; a directory or ZIP containing DOMDocument.xml
// SWF  = Compiled Flash binary (header + tag stream)
// SWC  = ZIP archive containing library.swf and assets
// AS   = ActionScript source file (imported as plain text for reference)

#include "flare/menubarcommandids.h"
#include "flare/menubar.h"
#include "flare/ocaio.h"
#include "flare/tproject.h"
#include "flare/preferences.h"
#include "flare/tapp.h"
#include "flare/toonzfolders.h"

#include "flareqt/gutil.h"
#include "flareqt/dvdialog.h"
#include "flare/filebrowserpopup.h"

// Native flash infrastructure (include_directories contains ../common/flash)
#include "XFLReader.h"
#include "FSWFStream.h"
#include "Macromedia.h"

#include <QFile>
#include <QDir>
#include <QDateTime>
#include <QFileInfo>
#include <QDesktopServices>
#include <QUrl>

// Minizip for ZIP/FLA/SWC extraction (include_directories contains minizip path)
#include "unzip.h"

#include <fstream>
#include <cstring>

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

// Extract every entry of a ZIP archive to outDir using the bundled minizip.
static bool extractZip(const QString &zipPath, const QString &outDir) {
    unzFile uf = unzOpen(zipPath.toUtf8().constData());
    if (!uf) return false;

    unz_global_info gi;
    if (unzGetGlobalInfo(uf, &gi) != UNZ_OK) { unzClose(uf); return false; }

    char entryName[512];
    char buf[16384];

    for (uLong i = 0; i < gi.number_entry; i++) {
        unz_file_info fi;
        if (unzGetCurrentFileInfo(uf, &fi, entryName, sizeof(entryName),
                                  nullptr, 0, nullptr, 0) != UNZ_OK)
            break;

        QString fullOut = outDir + "/" + QString::fromUtf8(entryName);
        QFileInfo info(fullOut);

        if (entryName[strlen(entryName) - 1] == '/') {
            // Directory entry
            QDir().mkpath(fullOut);
        } else {
            QDir().mkpath(info.absolutePath());
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

        if (i + 1 < gi.number_entry && unzGoToNextFile(uf) != UNZ_OK) break;
    }
    unzClose(uf);
    return true;
}

// Read the SWF file header and extract basic metadata.
struct SwfInfo {
    bool valid = false;
    bool compressed = false;  // zlib-compressed (SWF6+) or LZMA (SWF13+)
    int  version   = 0;
    int  width     = 0;
    int  height    = 0;
    int  frameRate = 0;
    int  frameCount = 0;
};

static SwfInfo readSwfHeader(const QString &path) {
    SwfInfo info;
    QFile f(path);
    if (!f.open(QIODevice::ReadOnly)) return info;

    QByteArray data = f.read(9);  // minimum SWF header size before RECT
    if (data.size() < 4) return info;

    unsigned char sig0 = data[0], sig1 = data[1], sig2 = data[2];
    // Signature: "FWS" (uncompressed), "CWS" (zlib), "ZWS" (LZMA)
    if ((sig0 != 'F' && sig0 != 'C' && sig0 != 'Z') ||
         sig1 != 'W' || sig2 != 'S')
        return info;

    info.valid      = true;
    info.compressed = (sig0 == 'C' || sig0 == 'Z');
    info.version    = static_cast<unsigned char>(data[3]);

    // For uncompressed files we can read frame rect right away.
    // For compressed, we at least have version & file length.
    if (!info.compressed && data.size() >= 9) {
        // After the 8-byte fixed header comes the RECT record (variable bits).
        // Minimum: 1 byte for Nbits, then 4 x Nbits bits.
        // We skip Twips→pixel conversion and just report what we can.
        QByteArray rest = f.read(256);
        data += rest;

        int offset = 8;
        if (offset < data.size()) {
            unsigned char first = static_cast<unsigned char>(data[offset]);
            int nbits = first >> 3;  // high 5 bits = Nbits
            int totalBits = 5 + 4 * nbits;
            int bytesNeeded = (totalBits + 7) / 8;
            if (offset + bytesNeeded + 4 <= data.size()) {
                // Decode RECT via bit stream
                int bitPos = offset * 8 + 5;  // skip Nbits field
                auto readBits = [&](int n) -> int {
                    int val = 0;
                    for (int b = 0; b < n; b++) {
                        int byteIdx = bitPos / 8;
                        int bitIdx  = 7 - (bitPos % 8);
                        if (byteIdx < data.size())
                            val = (val << 1) | ((static_cast<unsigned char>(data[byteIdx]) >> bitIdx) & 1);
                        else
                            val <<= 1;
                        bitPos++;
                    }
                    return val;
                };
                // RECT: Xmin, Xmax, Ymin, Ymax in twips (1/20 pixel)
                auto readSBits = [&](int n) -> int {
                    int val = readBits(n);
                    if (val & (1 << (n - 1))) val -= (1 << n);
                    return val;
                };
                int xmin = readSBits(nbits);
                int xmax = readSBits(nbits);
                int ymin = readSBits(nbits);
                int ymax = readSBits(nbits);
                info.width  = (xmax - xmin) / 20;
                info.height = (ymax - ymin) / 20;

                // After RECT: 2-byte frame rate (8.8 fixed), 2-byte frame count
                int afterRect = (bitPos + 7) / 8;
                if (afterRect + 4 <= data.size()) {
                    info.frameRate =
                        static_cast<unsigned char>(data[afterRect + 1]);  // integer part
                    info.frameCount =
                        static_cast<unsigned char>(data[afterRect + 2]) |
                        (static_cast<unsigned char>(data[afterRect + 3]) << 8);
                }
            }
        }
    }

    return info;
}

// Build a plain-text manifest listing imported files in outDir.
static void writeManifest(const QString &outDir, const QStringList &files,
                          const QString &sourceFile) {
    QFile mf(outDir + "/manifest.txt");
    if (!mf.open(QIODevice::WriteOnly | QIODevice::Text)) return;
    mf.write(("Source: " + sourceFile + "\n").toUtf8());
    mf.write(("Exported files:\n").toUtf8());
    for (const auto &f : files) mf.write(("  " + f + "\n").toUtf8());
    mf.close();
}

// Open the output folder in the system file manager.
static void openFolder(const QString &path) {
    QDesktopServices::openUrl(QUrl::fromLocalFile(path));
}

} // anonymous namespace

// ---------------------------------------------------------------------------
// Command: Import Flash (FLA / XFL / SWC / SWF / AS) — fully native
// ---------------------------------------------------------------------------

class ImportFlashVectorCommand final : public MenuItemHandler {
public:
    ImportFlashVectorCommand() : MenuItemHandler(MI_ImportFlashVector) {}
    void execute() override;
} ImportFlashVectorCommand;

void ImportFlashVectorCommand::execute() {
    ToonzScene *scene = TApp::instance()->getCurrentScene()->getScene();

    static GenericLoadFilePopup *loadPopup = nullptr;
    if (!loadPopup) {
        loadPopup = new GenericLoadFilePopup(
            QObject::tr("Import Flash File (FLA / XFL / SWF / SWC / AS)"));
        loadPopup->addFilterType("fla");
        loadPopup->addFilterType("swf");
        loadPopup->addFilterType("xfl");
        loadPopup->addFilterType("swc");
        loadPopup->addFilterType("as");
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
    QString   ext     = fp.getType().toLower().c_str();
    QStringList exported;
    QString info;

    // ---- FLA / XFL / SWC : ZIP-based container ----
    if (ext == "fla" || ext == "swc" || (ext == "xfl" && XFL::isFLAZipBased(fp))) {

        if (!extractZip(srcPath, outPath)) {
            DVGui::error(QObject::tr("Failed to extract archive: %1").arg(srcPath));
            return;
        }

        // For XFL/FLA: parse the document structure and report
        XFL::Reader reader(fp);
        if (ext != "swc") {
            // Re-point reader at extracted directory
            TFilePath extractedXfl = outDir;
            if (!XFL::isXFLDirectory(extractedXfl)) {
                // Look one level deep
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
                    info = QObject::tr(
                        "Document: %1 × %2 px  |  %3 fps  |  %4 symbol(s)")
                        .arg(doc.width).arg(doc.height)
                        .arg(doc.frameRate, 0, 'f', 1)
                        .arg(static_cast<int>(doc.symbols.size()));
                }
            }
        }

        // Collect extracted files for auto-import
        QDir qout(outPath);
        QStringList filters = {"*.png","*.jpg","*.jpeg","*.svg","*.xml","*.as"};
        QFileInfoList list = qout.entryInfoList(filters,
                                QDir::Files | QDir::NoDotAndDotDot | QDir::Recursive);
        for (const auto &fi : list)
            exported << qout.relativeFilePath(fi.absoluteFilePath());

    // ---- XFL directory ----
    } else if (ext == "xfl" && QFileInfo(srcPath).isDir()) {
        XFL::Reader reader(fp);
        if (!reader.read()) {
            DVGui::error(QObject::tr("Failed to read XFL: %1").arg(reader.getError().c_str()));
            return;
        }
        const XFL::Document &doc = reader.getDocument();
        info = QObject::tr(
            "Document: %1 × %2 px  |  %3 fps  |  %4 symbol(s)")
            .arg(doc.width).arg(doc.height)
            .arg(doc.frameRate, 0, 'f', 1)
            .arg(static_cast<int>(doc.symbols.size()));

        // Copy assets to output dir
        QDir src(srcPath);
        QStringList filters = {"*.png","*.jpg","*.jpeg","*.svg","*.xml","*.as"};
        QFileInfoList list = src.entryInfoList(filters,
                                QDir::Files | QDir::NoDotAndDotDot | QDir::Recursive);
        for (const auto &fi : list) {
            QString rel = src.relativeFilePath(fi.absoluteFilePath());
            QString dst = outPath + "/" + rel;
            QDir().mkpath(QFileInfo(dst).absolutePath());
            QFile::copy(fi.absoluteFilePath(), dst);
            exported << rel;
        }

    // ---- SWF binary ----
    } else if (ext == "swf") {
        SwfInfo swf = readSwfHeader(srcPath);
        if (!swf.valid) {
            DVGui::error(QObject::tr("Not a valid SWF file: %1").arg(srcPath));
            return;
        }
        info = QObject::tr(
            "SWF v%1  |  %2 × %3 px  |  %4 fps  |  %5 frame(s)%6")
            .arg(swf.version)
            .arg(swf.width).arg(swf.height)
            .arg(swf.frameRate)
            .arg(swf.frameCount)
            .arg(swf.compressed ? QObject::tr("  [compressed]") : QString());

        // Copy the SWF itself to output
        QString dstSwf = outPath + "/" + QFileInfo(srcPath).fileName();
        QFile::copy(srcPath, dstSwf);
        exported << QFileInfo(srcPath).fileName();

    // ---- ActionScript source ----
    } else if (ext == "as") {
        QString dstAs = outPath + "/" + QFileInfo(srcPath).fileName();
        QFile::copy(srcPath, dstAs);
        exported << QFileInfo(srcPath).fileName();
        info = QObject::tr("ActionScript source copied for reference.");

    } else {
        DVGui::warning(QObject::tr("Unsupported Flash format: .%1").arg(ext));
        return;
    }

    writeManifest(outPath, exported, srcPath);

    // Auto-load importable asset types into the scene
    int imported = 0;
    for (const QString &rel : exported) {
        QString full = outPath + "/" + rel;
        QString e = QFileInfo(full).suffix().toLower();
        if (e == "png" || e == "jpg" || e == "jpeg" || e == "svg") {
            if (scene->loadLevel(TFilePath(full.toStdString())))
                imported++;
        }
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

// ---------------------------------------------------------------------------
// Command: Import Flash Container (same handler, kept for menu compatibility)
// ---------------------------------------------------------------------------

class ImportFlashContainerCommand final : public MenuItemHandler {
public:
    ImportFlashContainerCommand() : MenuItemHandler(MI_ImportFlashContainer) {}
    void execute() override;
} ImportFlashContainerCommand;

void ImportFlashContainerCommand::execute() {
    // Delegate to the unified import command
    ImportFlashVectorCommand cmd;
    cmd.execute();
}

