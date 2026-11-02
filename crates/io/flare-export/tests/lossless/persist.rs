//! The source survives Save and opening the project again.

use super::*;
use flare_storage::{load, save};

#[test]
fn a_project_saved_and_opened_again_still_exports_the_original() {
    let original = fixtures::fla();
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("p.flare");
    save(&imported_fla(), &path).expect("saved");
    let loaded = load(&path).expect("loaded");
    let output = export(&loaded);
    assert!(output.lossless.exact, "{}", output.lossless.reason);
    assert_eq!(bytes_of(&output), original);
}

#[test]
fn a_folder_project_saved_and_opened_again_still_exports_the_same_tree() {
    let (dir, document) = imported_folder();
    let before = fixtures::tree(&dir.path().join("Project"));
    let path = dir.path().join("p.flare");
    save(&document, &path).expect("saved");
    let loaded = load(&path).expect("loaded");
    let out = dir.path().join("Out");
    flare_export::write_directory(&export(&loaded), &out).expect("written");
    assert_eq!(fixtures::tree(&out), before);
}

#[test]
fn an_edit_saved_and_undone_after_opening_is_the_original_again() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("p.flare");
    save(&imported_fla(), &path).expect("saved");
    let mut session = EditorSession::new(load(&path).expect("loaded"));
    let name = session.document().scenes()[0].timeline().layers()[0]
        .name()
        .to_owned();
    rename_first_layer(&mut session, "After reopen");
    assert!(!export(session.document()).lossless.exact);
    save(session.document(), &path).expect("saved edited");
    let mut reopened = EditorSession::new(load(&path).expect("loaded"));
    assert!(!export(reopened.document()).lossless.exact);
    // The edit is still an edit of the import, and undoing it by hand makes it the import again.
    rename_first_layer(&mut reopened, &name);
    let output = export(reopened.document());
    assert!(output.lossless.exact, "{}", output.lossless.reason);
    assert_eq!(bytes_of(&output), fixtures::fla());
}

#[test]
fn an_edited_project_keeps_the_untouched_files_after_reopening() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("p.flare");
    let mut session = EditorSession::new(imported_fla());
    rename_first_layer(&mut session, "Saved edit");
    save(session.document(), &path).expect("saved");
    let output = export(&load(&path).expect("loaded"));
    let entries: std::collections::BTreeMap<_, _> = fixtures::zip_entries(&bytes_of(&output))
        .into_iter()
        .collect();
    for (name, body) in fixtures::files() {
        if name != "DOMDocument.xml" {
            assert_eq!(entries.get(name), Some(&body), "{name}");
        }
    }
}

#[test]
fn a_project_without_a_source_has_none_after_saving_and_stays_small() {
    let dir = tempfile::tempdir().expect("dir");
    let with = dir.path().join("with.flare");
    let without = dir.path().join("without.flare");
    let document = imported_fla();
    save(&document, &with).expect("saved");
    save(&document.clone().with_origin(None), &without).expect("saved");
    assert!(load(&without).expect("loaded").origin().is_none());
    assert!(load(&with).expect("loaded").origin().is_some());
    let (with, without) = (
        std::fs::metadata(with).expect("meta").len(),
        std::fs::metadata(without).expect("meta").len(),
    );
    assert!(without < with, "{without} < {with}");
    assert!(
        without < 64 * 1024,
        "a small project stays small: {without}"
    );
}
