// mohoimport.cpp - Import Moho (Lost Marble) 2D animation projects
// Copyright (c) 2026 Flare Project
//
// Reads a .moho / .mohoproj project and reports what the rig contains, then
// writes a manifest of its structure and gathers whatever bitmaps it can find.
// See common/moho/MohoReader.h for the format and for what is out of scope.
//
// This deliberately does not attempt to render the rig. Reproducing a Moho
// render means solving the skeleton, then per-layer region-weight deformation,
// then bezier reconstruction, masking, layer effects and bone dynamics; see the
// reader header for why that is a much larger project than reading the file.
// What Flare can do honestly is show the user the rig's structure and collect
// its assets, which is what you need to decide what to do with it.

#include "flare/menubarcommandids.h"
#include "flare/menubar.h"
#include "flare/filebrowserpopup.h"
#include "flareqt/dvdialog.h"
#include "flareqt/gutil.h"

#include "MohoReader.h"
#include "tsystem.h"

#include <QMessageBox>

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
             "\n\n  This reads the project's structure and collects its assets. "
             "Rendering the rig -- pose, mesh deformation, Smart Bones, layer "
             "effects -- is not implemented, so the artwork is not brought into "
             "the scene as editable levels.");
    return s;
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

    // One dialog, not two. The previous version showed the output directory in
    // the message box and then popped a second modal saying the same thing --
    // and that second one fired unconditionally, so a failed write still
    // reported "Moho structure written to".
    QMessageBox *box =
        new QMessageBox(written > 0 ? QMessageBox::Information
                                    : QMessageBox::Warning,
                        QObject::tr("Moho Project"), describe(doc));
    if (written > 0) {
        box->setInformativeText(
            QObject::tr("Wrote %1 file(s) to:\n%2")
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
