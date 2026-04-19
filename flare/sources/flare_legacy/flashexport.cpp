// flashexport.cpp — Export Flare scenes as Adobe Animate FLA (XFL-in-ZIP)
//
// Architecture:
//   - Each Flare column becomes a DOMLayer in the XFL timeline.
//   - Each unique (level, frameId) cell is exported once as a PNG into LIBRARY/.
//   - Consecutive duplicate cells are encoded as a single DOMFrame with duration > 1,
//     matching Animate's run-length keyframe model.
//   - The ZIP archive is produced via XFL::writeFLA() which already exists in XFLReader.cpp.
//
// XFL format reference: Adobe Animate XFL specification (public, released by Adobe).
// No code was copied from any third-party project; only the documented tag/attribute
// names from the publicly-available XFL specification are used.

#include "menubarcommandids.h"
#include "menubar.h"
#include "tapp.h"
#include "flare/tscenehandle.h"
#include "flare/txsheethandle.h"
#include "flare/toonzscene.h"
#include "flare/txsheet.h"
#include "flare/txshcell.h"
#include "flare/txshsimplelevel.h"
#include "flare/txshchildlevel.h"
#include "flare/sceneproperties.h"
#include "flare/tcamera.h"
#include "flare/tproject.h"
#include "flare/levelset.h"
#include "toutputproperties.h"
#include "flare/txshlevelcolumn.h"

#include "flareqt/gutil.h"
#include "flareqt/dvdialog.h"
#include "filebrowserpopup.h"
#include "iocommand.h"
#include "exportlevelcommand.h"
#include "tsystem.h"
#include "tfilepath.h"

#include "XFLReader.h"   // XFL::writeFLA()

#include <QXmlStreamWriter>
#include <QFile>
#include <QDir>
#include <QCoreApplication>
#include <QDesktopServices>
#include <QUrl>
#include <QString>
#include <QMap>

#include "tlevel_io.h"
#include "timage_io.h"
#include "tvectorimage.h"

using namespace DVGui;

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

namespace {

// Return a sanitised layer name for use in XML and file paths.
static QString layerName(TXshCellColumn *col, int colIndex) {
    TXshCell first = col->getCell(col->getFirstRow());
    TXshSimpleLevel *sl = first.getSimpleLevel();
    if (sl) {
        std::wstring name = sl->getName();
        if (!name.empty())
            return QString::fromStdWString(name);
    }
    return QStringLiteral("Layer_%1").arg(colIndex + 1);
}

// Unique key for a (level pointer, frameId) pair.
struct CellKey {
    TXshSimpleLevel *sl;
    TFrameId         fid;
    bool operator<(const CellKey &o) const {
        if (sl != o.sl) return sl < o.sl;
        return fid < o.fid;
    }
};

// Per-cell export result.
struct CellAsset {
    QString libraryName;  // e.g. "layer_0_f0001"
    QString pngRelPath;   // e.g. "LIBRARY/layer_0_f0001.png"
    int     width  = 0;
    int     height = 0;
};

// Export a single cell frame as PNG and record the asset.
// Returns false if the cell cannot be rendered (empty, unsupported type).
static bool exportCellPng(TXshSimpleLevel *sl,
                          const TFrameId  &fid,
                          const QString   &libDir,
                          const QString   &assetName,
                          CellAsset       &out) {
    TImageP image = IoCmd::exportedImage("png", *sl, fid);
    if (!image) return false;

    TRasterP raster;
    if (TRasterImageP ri = (TRasterImageP)(image))
        raster = ri->getRaster();
    else if (TToonzImageP ti = (TToonzImageP)(image))
        raster = ti->getRaster();

    if (raster) {
        out.width  = raster->getLx();
        out.height = raster->getLy();
    }

    QString pngName   = assetName + ".png";
    QString pngFull   = libDir + "/" + pngName;
    out.libraryName   = assetName;
    out.pngRelPath    = "LIBRARY/" + pngName;

    TFilePath pngPath(pngFull.toStdString());
    TLevelWriterP lw(pngPath);
    TImageWriterP iw = lw->getFrameWriter(fid);
    if (!iw) return false;
    iw->setFilePath(pngPath);
    iw->save(image);
    return true;
}

// ---------------------------------------------------------------------------
// XFL DOMDocument.xml builder
// ---------------------------------------------------------------------------

struct LayerData {
    QString name;
    // Sequence of keyframe descriptors:
    //   index   = first row of this keyframe
    //   duration= how many timeline rows it occupies
    //   asset   = if empty, it is a blank keyframe
    struct KF {
        int     index;
        int     duration;
        QString assetName;   // libraryName of the bitmap, or "" for blank
        int     imgW = 0;
        int     imgH = 0;
    };
    QList<KF> keyframes;
};

// Hex colour string from TPixel (e.g. "#ffffff")
static QString pixelToHex(const TPixel32 &px) {
    return QStringLiteral("#%1%2%3")
        .arg((int)px.r, 2, 16, QLatin1Char('0'))
        .arg((int)px.g, 2, 16, QLatin1Char('0'))
        .arg((int)px.b, 2, 16, QLatin1Char('0'));
}

static void writeDOMDocument(const QString  &xflDir,
                             int             sceneW,
                             int             sceneH,
                             double          fps,
                             const TPixel32 &bgColor,
                             const QList<LayerData> &layers,
                             const QMap<QString, CellAsset> &assets) {
    QFile f(xflDir + "/DOMDocument.xml");
    if (!f.open(QIODevice::WriteOnly | QIODevice::Text)) return;

    QXmlStreamWriter x(&f);
    x.setAutoFormatting(true);
    x.writeStartDocument("1.0");

    x.writeStartElement("DOMDocument");
    x.writeAttribute("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance");
    x.writeAttribute("xmlns",     "http://ns.adobe.com/xfl/2008/");
    x.writeAttribute("creatorInfo", "Flare Animation");
    x.writeAttribute("platform",  "");
    x.writeAttribute("versionInfo", "Flare 1.0");
    x.writeAttribute("majorVersion", "20");
    x.writeAttribute("minorVersion", "0");
    x.writeAttribute("buildNumber", "0");
    x.writeAttribute("width",       QString::number(sceneW));
    x.writeAttribute("height",      QString::number(sceneH));
    x.writeAttribute("frameRate",   QString::number((int)fps));
    x.writeAttribute("backgroundColor", pixelToHex(bgColor));
    x.writeAttribute("currentTimeline", "1");
    x.writeAttribute("xflVersion",  "2.971");

    // <folders/>
    x.writeEmptyElement("folders");

    // <media>
    x.writeStartElement("media");
    for (const CellAsset &a : assets) {
        x.writeStartElement("DOMBitmapItem");
        x.writeAttribute("name",        a.libraryName);
        x.writeAttribute("href",        a.pngRelPath);
        x.writeAttribute("linkageExportForAS", "false");
        x.writeAttribute("linkageImportForRS", "false");
        x.writeAttribute("isSharedToAll", "false");
        x.writeAttribute("bitmapDataHRes", "72");
        x.writeAttribute("bitmapDataVRes", "72");
        x.writeAttribute("compressionType", "lossless");
        x.writeEndElement(); // DOMBitmapItem
    }
    x.writeEndElement(); // media

    // <symbols/>
    x.writeEmptyElement("symbols");

    // <timelines>
    x.writeStartElement("timelines");
    x.writeStartElement("DOMTimeline");
    x.writeAttribute("name", "Scene 1");

    x.writeStartElement("layers");

    // Layers are written bottom-to-top in XFL (last Flare column = bottom XFL layer)
    for (int li = layers.size() - 1; li >= 0; --li) {
        const LayerData &ld = layers[li];
        x.writeStartElement("DOMLayer");
        x.writeAttribute("name",    ld.name);
        x.writeAttribute("color",   "#4FFF4F");

        x.writeStartElement("frames");

        for (const LayerData::KF &kf : ld.keyframes) {
            x.writeStartElement("DOMFrame");
            x.writeAttribute("index",    QString::number(kf.index));
            x.writeAttribute("duration", QString::number(kf.duration));
            if (!kf.assetName.isEmpty())
                x.writeAttribute("keyFrame", "true");

            if (!kf.assetName.isEmpty()) {
                x.writeStartElement("elements");
                x.writeStartElement("DOMBitmapInstance");
                x.writeAttribute("libraryItemName", kf.assetName);
                x.writeAttribute("selected", "false");

                // Centre the bitmap on the stage
                x.writeStartElement("matrix");
                x.writeStartElement("Matrix");
                x.writeAttribute("a",  "1");
                x.writeAttribute("b",  "0");
                x.writeAttribute("c",  "0");
                x.writeAttribute("d",  "1");
                x.writeAttribute("tx", QString::number(-(kf.imgW / 2)));
                x.writeAttribute("ty", QString::number(-(kf.imgH / 2)));
                x.writeEndElement(); // Matrix
                x.writeEndElement(); // matrix

                x.writeStartElement("transformationPoint");
                x.writeStartElement("Point");
                x.writeAttribute("x", "0");
                x.writeAttribute("y", "0");
                x.writeEndElement(); // Point
                x.writeEndElement(); // transformationPoint

                x.writeEndElement(); // DOMBitmapInstance
                x.writeEndElement(); // elements
            }

            x.writeEndElement(); // DOMFrame
        }

        x.writeEndElement(); // frames
        x.writeEndElement(); // DOMLayer
    }

    x.writeEndElement(); // layers
    x.writeEndElement(); // DOMTimeline
    x.writeEndElement(); // timelines

    x.writeEndElement(); // DOMDocument
    x.writeEndDocument();
}

// ---------------------------------------------------------------------------
// Core export logic
// ---------------------------------------------------------------------------

static bool doExportFLA(ToonzScene *scene, TXsheet *xsheet,
                        const TFilePath &flaPath) {
    // Gather scene properties
    TOutputProperties *oprop = scene->getProperties()->getOutputProperties();
    double fps   = oprop->getFrameRate();
    int    totalFrames = xsheet->getFrameCount();
    int    sceneW = scene->getCurrentCamera()->getRes().lx;
    int    sceneH = scene->getCurrentCamera()->getRes().ly;
    TPixel32 bgColor = scene->getProperties()->getBgColor();

    // Create temp XFL directory
    QString tmpName = "flare_fla_export_" +
                      QString::number(QDateTime::currentMSecsSinceEpoch());
    TFilePath xflDir = TSystem::getTempDir() + TFilePath(tmpName.toStdString());
    try { TSystem::mkDir(xflDir); } catch (...) {}
    QString xflDirQ = xflDir.getQString();

    QString libDirQ = xflDirQ + "/LIBRARY";
    QDir().mkpath(libDirQ);

    QMap<CellKey, CellAsset> assetMap;   // key → exported asset
    QList<LayerData>         layers;

    int assetSeq = 0;

    for (int col = 0; col < xsheet->getColumnCount(); col++) {
        if (xsheet->isColumnEmpty(col)) continue;

        TXshCellColumn *column = xsheet->getColumn(col)->getCellColumn();
        if (!column) continue;
        if (column->getColumnType() != TXshCellColumn::eLevelType) continue;

        LayerData ld;
        ld.name = layerName(column, col);

        // Walk every timeline row, building run-length keyframes
        int row = 0;
        while (row < totalFrames) {
            TXshCell cell = column->getCell(row);
            TXshSimpleLevel *sl = cell.getSimpleLevel();

            // Find run length: how many consecutive rows have this same cell
            int run = 1;
            while (row + run < totalFrames) {
                TXshCell next = column->getCell(row + run);
                if (next.getSimpleLevel() != sl || next.getFrameId() != cell.getFrameId())
                    break;
                ++run;
            }

            LayerData::KF kf;
            kf.index    = row;
            kf.duration = run;

            if (sl) {
                CellKey key{sl, cell.getFrameId()};
                if (!assetMap.contains(key)) {
                    QString assetName =
                        QStringLiteral("asset_%1").arg(assetSeq++, 4, 10, QLatin1Char('0'));
                    CellAsset a;
                    exportCellPng(sl, cell.getFrameId(), libDirQ, assetName, a);
                    assetMap.insert(key, a);
                }
                const CellAsset &a = assetMap.value(key);
                kf.assetName = a.libraryName;
                kf.imgW      = a.width  > 0 ? a.width  : sceneW;
                kf.imgH      = a.height > 0 ? a.height : sceneH;
            }
            // blank frame: kf.assetName stays empty

            ld.keyframes.append(kf);
            row += run;
        }

        if (!ld.keyframes.isEmpty())
            layers.append(ld);
    }

    if (layers.isEmpty()) {
        TSystem::rmDirTree(xflDir);
        return false;
    }

    // Build flat asset list preserving insertion order (QMap iterates by key)
    QMap<QString, CellAsset> assetsByName;
    for (const CellAsset &a : assetMap)
        if (!a.libraryName.isEmpty())
            assetsByName.insert(a.libraryName, a);

    // Write DOMDocument.xml
    writeDOMDocument(xflDirQ, sceneW, sceneH, fps, bgColor, layers, assetsByName);

    // Pack into FLA ZIP
    bool ok = XFL::writeFLA(xflDir, flaPath);

    // Clean up temp dir
    try { TSystem::rmDirTree(xflDir); } catch (...) {}

    return ok;
}

} // namespace

// ---------------------------------------------------------------------------
// Command handler
// ---------------------------------------------------------------------------

class FlashExportCommand final : public MenuItemHandler {
public:
    FlashExportCommand() : MenuItemHandler(MI_ExportFlash) {}
    void execute() override;
} g_flashExportCommand;

void FlashExportCommand::execute() {
    ToonzScene *scene  = TApp::instance()->getCurrentScene()->getScene();
    TXsheet    *xsheet = TApp::instance()->getCurrentXsheet()->getXsheet();
    if (!scene || !xsheet) return;

    if (xsheet->getFrameCount() == 0 || xsheet->getColumnCount() == 0) {
        DVGui::warning(QObject::tr("The scene has no content to export."));
        return;
    }

    static GenericSaveFilePopup *savePopup = nullptr;
    if (!savePopup) {
        savePopup = new GenericSaveFilePopup(
            QObject::tr("Export as Adobe Animate FLA"));
        savePopup->addFilterType("fla");
    }

    if (!scene->isUntitled())
        savePopup->setFolder(scene->getScenePath().getParentDir());
    else
        savePopup->setFolder(
            TProjectManager::instance()->getCurrentProject()->getScenesPath());

    TFilePath fp = savePopup->getPath();
    if (fp.isEmpty()) return;
    if (fp.getType().empty()) fp = fp.withType("fla");

    // Show a brief progress indication
    DVGui::ProgressDialog progress(
        QObject::tr("Exporting FLA…"), QObject::tr("Cancel"), 0, 0);
    progress.setWindowTitle(QObject::tr("Export Flash / Adobe Animate"));
    progress.setWindowModality(Qt::WindowModal);
    progress.show();
    QCoreApplication::processEvents();

    bool ok = doExportFLA(scene, xsheet, fp);

    progress.close();

    if (!ok) {
        DVGui::error(QObject::tr(
            "FLA export failed.\n\n"
            "Make sure the scene has visible level columns and all frames "
            "can be rendered."));
        return;
    }

    std::vector<QString> btns = {
        QObject::tr("OK"),
        QObject::tr("Open containing folder")
    };
    int ret = DVGui::MsgBox(
        DVGui::INFORMATION,
        QObject::tr("Exported to:\n%1").arg(fp.getQString()),
        btns, 0);
    if (ret == 1) {
        TFilePath folder = fp.getParentDir();
        if (TSystem::isUNC(folder))
            QDesktopServices::openUrl(QUrl(folder.getQString()));
        else
            QDesktopServices::openUrl(QUrl::fromLocalFile(folder.getQString()));
    }
}
