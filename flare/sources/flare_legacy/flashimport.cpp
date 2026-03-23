// flashimport.cpp (flare_legacy) — Native Flash format import.
// Mirrors the unified command in flare/flashimport.cpp.
// No external tools required.
//
// SWF tag parsing follows the approach from:
//   - lightspark (LGPL-3.0, github.com/lightspark/lightspark)
//   - open-flash/swf-bitmap (AGPL-3.0, github.com/open-flash/swf-bitmap)
// FLA binary media detection uses magic-byte sniffing per Adobe XFL spec.

#include "flare/menubarcommandids.h"
#include "flare/menubar.h"
#include "flare/tapp.h"
#include "flare/ocaio.h"
#include "flare/tproject.h"
#include "flare/preferences.h"
#include "flare/toonzfolders.h"
#include "flare/toonzscene.h"

#include "flareqt/gutil.h"
#include "flareqt/dvdialog.h"
#include "flare/filebrowserpopup.h"
#include "iocommand.h"
#include "tsystem.h"
#include "tfilepath.h"

// XFL / ZIP / SWF native infrastructure
#include "XFLReader.h"
#include "FSWFStream.h"
#include "Macromedia.h"
#include "unzip.h"

#include <QFile>
#include <QDir>
#include <QDirIterator>
#include <QDateTime>
#include <QFileInfo>
#include <QDesktopServices>
#include <QUrl>
#include <cstring>

using namespace DVGui;

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

// Zip Slip guard: validate resolved path stays under intended directory.
static bool isPathUnderDir(const QString &dir, const QString &candidate) {
    QDir base(dir);
    QString canonical = QFileInfo(candidate).absoluteFilePath();
    QString baseCanonical = base.absolutePath();
    if (!baseCanonical.endsWith('/')) baseCanonical += '/';
    return canonical.startsWith(baseCanonical) || canonical == base.absolutePath();
}

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

        QString entryStr = QString::fromUtf8(entryName);
        // Zip Slip protection
        if (entryStr.startsWith('/') || entryStr.startsWith('\\') ||
            entryStr.contains("..") ||
            (entryStr.length() >= 2 && entryStr[1] == ':')) {
            if (i+1 < gi.number_entry) unzGoToNextFile(uf);
            continue;
        }
        QString fullOut = outDir + "/" + entryStr;
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
    unzClose(uf); return true;
}

// FLA binary media: .dat files in bin/ are raw JPEG/PNG/GIF data.
static QStringList extractFLABinaryMedia(const QString &outDir) {
    QStringList extracted;
    QString binDir = outDir + "/bin";
    QDir bin(binDir);
    if (!bin.exists()) return extracted;
    int idx = 0;
    QDirIterator it(binDir, {"*.dat"}, QDir::Files);
    while (it.hasNext()) {
        it.next();
        QFile f(it.filePath());
        if (!f.open(QIODevice::ReadOnly)) continue;
        QByteArray header = f.read(8);
        f.close();
        if (header.size() < 4) continue;
        const unsigned char *h = reinterpret_cast<const unsigned char *>(header.constData());
        QString ext;
        if (h[0] == 0xFF && h[1] == 0xD8 && h[2] == 0xFF) ext = "jpg";
        else if (h[0] == 0x89 && h[1] == 0x50 && h[2] == 0x4E && h[3] == 0x47) ext = "png";
        else if (h[0] == 0x47 && h[1] == 0x49 && h[2] == 0x46 && h[3] == 0x38) ext = "gif";
        else continue;
        QString fname = QString("media_%1.%2").arg(idx++, 4, 10, QChar('0')).arg(ext);
        QFile::copy(it.filePath(), outDir + "/" + fname);
        extracted << fname;
    }
    return extracted;
}

static void writeManifest(const QString &outDir, const QStringList &files,
                          const QString &src) {
    QFile mf(outDir + "/manifest.txt");
    if (!mf.open(QIODevice::WriteOnly | QIODevice::Text)) return;
    mf.write(QByteArray("Source: ") + src.toUtf8() + "\n");
    mf.write("Exported files:\n");
    for (const auto &f : files) mf.write(QByteArray("  ") + f.toUtf8() + "\n");
}

static void openFolder(const QString &path) {
    QDesktopServices::openUrl(QUrl::fromLocalFile(path));
}

} // namespace

class ImportFlashVectorCommand final : public MenuItemHandler {
public:
    ImportFlashVectorCommand() : MenuItemHandler(MI_ImportFlashVector) {}
    void execute() override;
} g_importFlashVectorLegacy;

void ImportFlashVectorCommand::execute() {
    ToonzScene *scene = TApp::instance()->getCurrentScene()->getScene();

    static GenericLoadFilePopup *loadPopup = nullptr;
    if (!loadPopup) {
        loadPopup = new GenericLoadFilePopup(
            QObject::tr("Import Flash (FLA / XFL / SWC / AS)"));
        loadPopup->addFilterType("fla");
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

    TFilePath outDir  = makeTempImportDir("flare_flash_import_");
    QString   outPath = outDir.getQString();
    QString   srcPath = fp.getQString();
    QString   ext     = QString::fromStdString(fp.getType()).toLower();
    QStringList exported;
    QString info;

    if (ext == "fla" || ext == "swc" || ext == "xfl") {
        if (!extractZip(srcPath, outPath)) {
            DVGui::error(QObject::tr("Failed to extract archive: %1").arg(srcPath));
            return;
        }
        if (ext != "swc") {
            TFilePath extracted = outDir;
            if (!XFL::isXFLDirectory(extracted)) {
                try {
                    TFilePathSet entries = TSystem::readDirectory(outDir, false, false, true);
                    for (const auto &e : entries)
                        if (XFL::isXFLDirectory(e)) { extracted = e; break; }
                } catch (...) {}
            }
            if (XFL::isXFLDirectory(extracted)) {
                XFL::Reader r(extracted);
                if (r.read()) {
                    const XFL::Document &doc = r.getDocument();
                    info = QObject::tr("Document: %1 × %2 px  |  %3 fps  |  %4 symbol(s)")
                        .arg(doc.width).arg(doc.height)
                        .arg(doc.frameRate, 0, 'f', 1)
                        .arg((int)doc.symbols.size());
                }
            }
            // Extract binary media from FLA's bin/ directory
            QStringList binMedia = extractFLABinaryMedia(outPath);
            exported += binMedia;
            if (!binMedia.isEmpty())
                info += QObject::tr("\n  %1 bitmap(s) extracted from FLA binary media")
                        .arg(binMedia.size());
        }
        QDirIterator it(outPath, kAssetFilters,
                        QDir::Files | QDir::NoDotAndDotDot,
                        QDirIterator::Subdirectories);
        QDir base(outPath);
        while (it.hasNext()) { it.next(); exported << base.relativeFilePath(it.filePath()); }
    } else if (ext == "as") {
        QString dst = outPath + "/" + QFileInfo(srcPath).fileName();
        QFile::copy(srcPath, dst);
        exported << QFileInfo(srcPath).fileName();
        info = QObject::tr("ActionScript source copied for reference.");
    } else {
        DVGui::warning(QObject::tr("Unsupported Flash format: .%1").arg(ext));
        return;
    }

    writeManifest(outPath, exported, srcPath);

    // Only auto-load image formats that Flare can natively handle as levels.
    int imported = 0;
    {
        IoCmd::LoadResourceArguments args;
        for (const QString &rel : exported) {
            QString full = outPath + "/" + rel;
            QString e    = QFileInfo(full).suffix().toLower();
            if (e == "png" || e == "jpg" || e == "jpeg" || e == "svg")
                args.resourceDatas.push_back(
                    IoCmd::LoadResourceArguments::ResourceData(TFilePath(full.toStdWString())));
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
