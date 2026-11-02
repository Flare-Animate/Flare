//! An import that was not edited exports as the original.

use super::*;
use flare_export::{write_directory, write_fla};

#[test]
fn the_fixture_is_imported_with_its_source_kept() {
    let document = imported_fla();
    let origin = document.origin().expect("kept");
    assert_eq!(origin.kind(), flare_document::OriginKind::Fla);
    assert_eq!(origin.len(), fixtures::fla().len());
    // What Flare does not model is still source data, as before.
    assert!(!document.attachments().is_empty());
}

#[test]
fn an_unchanged_fla_exports_as_the_same_file_byte_for_byte() {
    let original = fixtures::fla();
    let output = export(&imported_fla());
    assert!(output.lossless.exact, "{}", output.lossless.reason);
    assert_eq!(bytes_of(&output), original);

    let dir = tempfile::tempdir().expect("dir");
    let out = dir.path().join("out.fla");
    write_fla(&output, &out).expect("written");
    assert_eq!(std::fs::read(&out).expect("read"), original);
}

#[test]
fn an_unchanged_fla_exports_as_a_folder_with_every_entry_identical() {
    let output = export(&imported_fla());
    let dir = tempfile::tempdir().expect("dir");
    let out = dir.path().join("Out");
    write_directory(&output, &out).expect("written");
    let tree = fixtures::tree(&out);
    for (name, body) in fixtures::files() {
        assert_eq!(tree.get(name), Some(&body), "{name}");
    }
    // The folder gets the stub Animate needs (named for the source), which an archive has no use for.
    assert_eq!(tree.len(), fixtures::files().len() + 1);
    assert_eq!(tree.get("Document.xfl"), Some(&b"PROXY-CS5".to_vec()));
}

#[test]
fn an_unchanged_folder_exports_as_the_same_tree_byte_for_byte() {
    let (dir, document) = imported_folder();
    let original = fixtures::tree(&dir.path().join("Project"));
    assert!(original.contains_key("empty folder/") && original.contains_key(".hidden"));
    let output = export(&document);
    assert!(output.lossless.exact, "{}", output.lossless.reason);
    assert_eq!(output.file_count(), original.len());
    let out = dir.path().join("Out");
    write_directory(&output, &out).expect("written");
    assert_eq!(fixtures::tree(&out), original);
}

#[test]
fn an_unchanged_folder_exports_as_a_fla_with_identical_entries() {
    let (_dir, document) = imported_folder();
    let output = export(&document);
    let entries = fixtures::zip_entries(&bytes_of(&output));
    let map: std::collections::BTreeMap<_, _> = entries.iter().cloned().collect();
    for (name, body) in fixtures::files() {
        assert_eq!(map.get(name), Some(&body), "{name}");
    }
    assert_eq!(entries[0].0, "mimetype", "the type comes first");
    assert!(!map.contains_key("Project.xfl"), "an archive has no stub");
    // The result is itself a project Flare opens.
    assert!(flare_import::import_bytes(bytes_of(&output)).is_ok());
}

#[test]
fn importing_through_the_stub_or_the_document_file_keeps_the_same_folder() {
    let (dir, document) = imported_folder();
    let project = dir.path().join("Project");
    for pick in ["Project.xfl", "DOMDocument.xml"] {
        let again = flare_import::import_path(&project.join(pick))
            .expect("imports")
            .document;
        assert_eq!(
            again.origin().expect("kept").digest(),
            document.origin().expect("kept").digest(),
            "{pick}"
        );
    }
}

#[test]
fn an_edit_that_is_undone_is_unchanged_again() {
    let original = fixtures::fla();
    let mut session = EditorSession::new(imported_fla());
    assert!(export(session.document()).lossless.exact);
    rename_first_layer(&mut session, "Renamed");
    let edited = export(session.document());
    assert!(!edited.lossless.exact, "{}", edited.lossless.reason);
    assert_ne!(bytes_of(&edited), original);
    assert!(session.undo());
    let back = export(session.document());
    assert!(back.lossless.exact, "{}", back.lossless.reason);
    assert_eq!(bytes_of(&back), original);
    // Redo is an edit again.
    assert!(session.redo());
    assert!(!export(session.document()).lossless.exact);
}

#[test]
fn an_edit_that_changes_nothing_in_effect_is_unchanged() {
    // Renaming a layer to the name it has is the same document.
    let original = fixtures::fla();
    let mut session = EditorSession::new(imported_fla());
    let name = session.document().scenes()[0].timeline().layers()[0]
        .name()
        .to_owned();
    rename_first_layer(&mut session, &name);
    assert_eq!(bytes_of(&export(session.document())), original);
}

#[test]
fn a_project_that_was_not_imported_says_so_and_is_not_called_lossless() {
    let output = export(&imported_fla().with_origin(None));
    assert!(!output.lossless.exact);
    assert!(output.lossless.reason.contains("not imported from a file"));
    assert!(output.exact.is_none() && output.kept.is_empty());
}

#[test]
fn a_different_document_with_a_source_is_never_the_original() {
    // A document built elsewhere that carries this source is not what the import made.
    let origin = imported_fla().origin().cloned();
    let other = flare_import::import_path(
        &{
            let dir = tempfile::tempdir().expect("dir");
            let p = dir.path().join("a.xfl");
            std::fs::write(&p, "x").expect("w");
            std::fs::write(dir.path().join("DOMDocument.xml"), r#"<DOMDocument frameRate="12" width="5" height="5"><timelines><DOMTimeline name="S"><layers/></DOMTimeline></timelines></DOMDocument>"#).expect("w");
            std::mem::forget(dir);
            p
        },
    )
    .expect("imports")
    .document
    .with_origin(origin);
    let output = export(&other);
    assert!(!output.lossless.exact);
}
