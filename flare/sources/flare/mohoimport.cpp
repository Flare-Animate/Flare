// mohoimport.cpp - Import Moho (Lost Marble) 2D animation projects
// Copyright (c) 2026 Flare Project
//
// Reads a .moho / .mohoproj project and reports what the rig contains, then
// writes a manifest of its structure and gathers whatever bitmaps it can find.
// See common/moho/MohoReader.h for the format and for what is out of scope.
//
// Then builds the scene (MohoPlan.h): every drawable layer becomes an xsheet
// column (bitmap layers load their image, vector layers get a named vector
// level), bones become a pegbar hierarchy the columns are parented to, and
// switch layers gate their children's cells by the switch keys. Mesh
// deformation / Smart Bones are not solved; geometry is not rendered.

#include "flare/menubarcommandids.h"
#include "flare/menubar.h"
#include "flare/filebrowserpopup.h"
#include "flareqt/dvdialog.h"
#include "flareqt/gutil.h"

#include "MohoReader.h"
#include "MohoPlan.h"
#include "tapp.h"
#include "flare/tscenehandle.h"
#include "flare/toonzscene.h"
#include "flare/txsheet.h"
#include "flare/txshcell.h"
#include "flare/txshcolumn.h"
#include "flare/txshlevel.h"
#include "flare/txshsimplelevel.h"
#include "flare/txshleveltypes.h"
#include "flare/tstageobject.h"
#include "flare/tstageobjectid.h"
#include "tvectorimage.h"
#include "tundo.h"
#include "tsystem.h"

#include <QMessageBox>
#include <QFile>
#include <QJsonDocument>
#include <QJsonObject>
#include <QJsonArray>
#ifdef FLARE_WITH_RUST
#include "flare_formats.h"
#endif

namespace {

// Build the human-readable report. Kept separate from the command so the
// wording is not buried in the UI plumbing.
QString describe(const Moho::Document &doc) {
    QString s;
    s += QObject::tr("Moho project (format version %1, MIME %2)")
             .arg(doc.version)
             .arg(doc.mimeType.isEmpty() ? QObject::tr("(none)")
                                         : doc.mimeType);
    if (!doc.containerEntry.isEmpty())
        s += QObject::tr("\n  Read from: %1").arg(doc.containerEntry);
    s += QObject::tr("\n  Canvas: %1 x %2 at %3 fps, frames %4-%5")
             .arg(doc.width)
             .arg(doc.height)
             .arg(doc.fps)
             .arg(doc.startFrame)
             .arg(doc.endFrame);
    s += QObject::tr("\n  Layers: %1").arg(doc.totalLayers);
    for (auto it = doc.layerTypes.constBegin();
         it != doc.layerTypes.constEnd(); ++it)
        s += QObject::tr("\n      %1: %2").arg(it.key()).arg(it.value());

    if (!doc.bones.isEmpty())
        s += QObject::tr("\n  Bones: %1").arg(doc.bones.size());
    if (!doc.switches.isEmpty()) {
        s += QObject::tr("\n  Switch layers: %1").arg(doc.switches.size());
        for (const Moho::Switch &sw : doc.switches) {
            s += QObject::tr("\n      %1 (%2 alternative(s)")
                     .arg(sw.name)
                     .arg(sw.alternatives.size());
            if (!sw.childAtRest.isEmpty()) {
                s += QObject::tr(", at rest: %1").arg(sw.childAtRest);
                if (!sw.childAtEnd.isEmpty() && sw.childAtEnd != sw.childAtRest)
                    s += QObject::tr(", ending on: %1").arg(sw.childAtEnd);
            }
        }
    }
    s += QObject::tr("\n  Animation: %1 keyframe(s) across %2 channel(s)")
             .arg(doc.keyframes)
             .arg(doc.keyframeTracks);
    if (doc.actions > 0)
        s += QObject::tr(", %1 action name(s)").arg(doc.actions);
    if (doc.meshPoints > 0 || doc.shapes > 0)
        s += QObject::tr("\n  Geometry: %1 mesh point(s), %2 shape(s)")
                 .arg(doc.meshPoints)
                 .arg(doc.shapes);

    if (!doc.referencedImages.isEmpty()) {
        s += QObject::tr("\n  Images: %1 referenced, %2 found beside the project, "
                         "%3 missing")
                 .arg(doc.referencedImages.size())
                 .arg(doc.imagesFound)
                 .arg(doc.imagesMissing);
        // Moho keeps bitmaps outside the container, so a .moho on its own is
        // never a complete project. Say so, or the missing artwork looks like
        // a Flare bug.
        if (doc.imagesMissing > 0)
            s += QObject::tr(
                     "\n      Moho stores artwork outside the project file, in a "
                     "sibling folder. Use File > Gather Media in Moho (or supply "
                     "the images folder) to complete it.");
    }

    // Be explicit about the limit, so nobody reads this as a silent success.
    s += QObject::tr(
             "\n\n  Layers become xsheet columns, bones become pegbars and "
             "switch keys drive cells. Mesh deformation, Smart Bones and layer "
             "effects are not solved; vector geometry arrives as empty named "
             "vector levels to redraw or trace.");
    return s;
}

// Rust backend (flare_formats) cross-check: layers, bones, keyframe tracks and
// interpolation parsed independently of the C++ reader. Empty without Rust.
QString rustSummary(const TFilePath &fp) {
#ifdef FLARE_WITH_RUST
    QFile f(fp.getQString());
    if (!f.open(QIODevice::ReadOnly)) return QString();
    const QByteArray data = f.readAll();
    char *js = flare_moho_parse(reinterpret_cast<const uint8_t *>(data.constData()),
                                (size_t)data.size());
    if (!js) return QString();
    const QJsonObject o = QJsonDocument::fromJson(QByteArray(js)).object();
    flare_moho_free(js);
    return QObject::tr("\n  Rust parser: %1 layer(s), %2 bone(s), %3 track(s), "
                       "%4 keyframe(s) with interpolation")
        .arg(o["layers"].toArray().size())
        .arg(o["bones"].toArray().size())
        .arg(o["tracks"].toArray().size())
        .arg(o["keyframe_count"].toInt());
#else
    Q_UNUSED(fp);
    return QString();
#endif
}

// Build the plan into the current scene. Returns the number of columns made.
int buildScene(const TFilePath &fp, const Moho::Document &doc) {
    ToonzScene *scene = TApp::instance()->getCurrentScene()->getScene();
    if (!scene) return 0;
    TXsheet *xsh = scene->getXsheet();
    const Moho::Plan plan = Moho::makePlan(doc);
    const TFilePath dir = fp.getParentDir();

    TUndoManager::manager()->beginBlock();
    // Bones -> pegbars, after any pegbars already in the scene's tree.
    for (int b = 0; b < doc.bones.size(); ++b) {
        TStageObject *peg = xsh->getStageObject(TStageObjectId::PegbarId(b));
        peg->setName(doc.bones[b].name.toStdString());
        const int par = doc.bones[b].parent;
        peg->setParent(par >= 0 && par < doc.bones.size()
                           ? TStageObjectId::PegbarId(par)
                           : TStageObjectId::TableId);
    }
    const int col0 = xsh->getFirstFreeColumnIndex();
    int made = 0;
    for (const Moho::ColumnPlan &cp : plan.columns) {
        const int col = col0 + made;
        TXshLevel *lvl = nullptr;
        if (!cp.imagePath.isEmpty()) {
            const TFilePath img = dir + TFilePath(cp.imagePath.toStdWString());
            if (TSystem::doesExistFileOrLevel(img))
                lvl = scene->loadLevel(img, nullptr, cp.name.toStdWString());
        }
        if (!lvl) {  // vector / missing bitmap: named, editable vector level
            lvl = scene->createNewLevel(PLI_XSHLEVEL, cp.name.toStdWString());
            if (TXshSimpleLevel *sl = lvl ? lvl->getSimpleLevel() : nullptr)
                sl->setFrame(TFrameId(1), new TVectorImage());
        }
        if (!lvl) continue;
        std::vector<TFrameId> fids;
        lvl->getFids(fids);
        const TFrameId fid = fids.empty() ? TFrameId(1) : fids.front();
        for (int r = 0; r < plan.rowCount; ++r)
            if (cp.rows.value(r, true)) xsh->setCell(r, col, TXshCell(lvl, fid));
        TStageObject *obj = xsh->getStageObject(TStageObjectId::ColumnId(col));
        obj->setName(cp.name.toStdString());
        if (cp.parentBone >= 0 && cp.parentBone < doc.bones.size())
            obj->setParent(TStageObjectId::PegbarId(cp.parentBone));
        if (TXshColumn *c = xsh->getColumn(col))
            if (!cp.visible) c->setCamstandVisible(false);
        ++made;
    }
    TUndoManager::manager()->endBlock();
    TApp::instance()->getCurrentScene()->notifySceneChanged();
    TApp::instance()->getCurrentScene()->notifyCastChange();
    return made;
}

}  // namespace

// ---------------------------------------------------------------------------
// Command
// ---------------------------------------------------------------------------

class ImportMohoProjectCommand final : public MenuItemHandler {
public:
    ImportMohoProjectCommand() : MenuItemHandler(MI_ImportMohoProject) {}
    void execute() override;
} g_importMohoProjectCommand;

void ImportMohoProjectCommand::execute() {
    static GenericLoadFilePopup *loadPopup = nullptr;
    if (!loadPopup) {
        loadPopup =
            new GenericLoadFilePopup(QObject::tr("Import Moho Project"));
        // .moho/.anime are ZIP containers, .mohoproj/.animeproj the bare
        // document, .anme the pre-11 text format (recognised and reported).
        for (const char *ext : {"moho", "mohoproj", "anime", "animeproj", "anme"})
            loadPopup->addFilterType(ext);
    }

    TFilePath fp = loadPopup->getPath();
    if (fp.isEmpty()) return;

    Moho::Document doc;
    if (!Moho::read(fp, doc)) {
        DVGui::error(QObject::tr("Could not read the Moho project:\n\n%1")
                         .arg(doc.error));
        return;
    }

    // Write the manifest and gather the bitmaps beside the project, so the
    // structure is available to other tools and the artwork is not lost.
    // TFilePath is a path *component*, so getParentDir() gives the containing
    // folder rather than getPath().
    const TFilePath outDir = fp.getParentDir() + TFilePath(
        fp.getName() + "_flare_import");
    const int written = Moho::writeManifest(fp, doc, outDir);
    const int columns = buildScene(fp, doc);

    // One dialog, not two. The previous version showed the output directory in
    // the message box and then popped a second modal saying the same thing --
    // and that second one fired unconditionally, so a failed write still
    // reported "Moho structure written to".
    QMessageBox *box =
        new QMessageBox(written > 0 ? QMessageBox::Information
                                    : QMessageBox::Warning,
                        QObject::tr("Moho Project"),
                        describe(doc) + rustSummary(fp));
    if (written > 0) {
        box->setInformativeText(
            QObject::tr("Created %1 column(s), %2 pegbar(s).\nWrote %3 file(s) to:\n%4")
                .arg(columns)
                .arg(doc.bones.size())
                .arg(written)
                .arg(toQString(outDir)));
    } else {
        box->setInformativeText(
            QObject::tr("Could not write the manifest to:\n%1\n\n"
                        "The project was read successfully; only the export "
                        "failed.")
                .arg(toQString(outDir)));
    }
    box->setTextInteractionFlags(Qt::TextSelectableByMouse);
    box->exec();
    box->deleteLater();
}
