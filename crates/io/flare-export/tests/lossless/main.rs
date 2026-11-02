//! Opening a file in Flare and exporting it again, with or without an edit, must not lose the
//! file. See docs/decisions/LOSSLESS.md.
#![allow(clippy::expect_used)]

mod edited;
mod fixtures;
mod persist;
mod robust;
mod unchanged;

use flare_commands::Command;
use flare_document::Document;
use flare_export::{Output, export_xfl, fla_bytes};
use flare_history::EditorSession;

/// The fixture `.fla` imported, as Flare would on File > Import.
fn imported_fla() -> Document {
    flare_import::import_bytes(fixtures::fla())
        .expect("imports")
        .document
}

fn imported_folder() -> (tempfile::TempDir, Document) {
    let dir = tempfile::tempdir().expect("dir");
    let project = dir.path().join("Project");
    std::fs::create_dir(&project).expect("dir");
    fixtures::write_folder(&project);
    let document = flare_import::import_path(&project)
        .expect("imports")
        .document;
    (dir, document)
}

fn bytes_of(output: &Output) -> Vec<u8> {
    fla_bytes(output).expect("zip")
}

/// Renames the first layer of the first scene, the way the timeline's rename does.
fn rename_first_layer(session: &mut EditorSession, name: &str) {
    let scene = session.document().scenes()[0].id();
    let layer = session.document().scenes()[0].timeline().layers()[0].id();
    session
        .apply(vec![Command::UpdateLayer {
            scene,
            layer,
            change: flare_commands::LayerChange::Name(name.to_owned()),
        }])
        .expect("renamed");
}

fn export(document: &Document) -> Output {
    export_xfl(document).expect("export")
}
