//! Which format a name means, and that saving and loading honour it. See
//! docs/decisions/LOSSLESS.md for what a save of an import writes.
#![allow(clippy::expect_used)]

use std::path::{Path, PathBuf};

use flare_commands::{Command, LayerChange};
use flare_history::EditorSession;
use flare_runtime::{
    FileFormat, SaveError, load, resolve_save_path, save, saves_in_place, xfl_folder,
};

const FLA: &[u8] = include_bytes!("../../../io/flare-import/tests/fixtures/handmade.fla");

fn format(name: &str) -> Option<FileFormat> {
    FileFormat::of_path(Path::new(name))
}

#[test]
fn the_extension_decides_the_format_in_any_case_and_only_the_last_counts() {
    assert_eq!(format("a.fla"), Some(FileFormat::Fla));
    assert_eq!(format("a.FLA"), Some(FileFormat::Fla));
    assert_eq!(format("a.Xfl"), Some(FileFormat::Xfl));
    assert_eq!(format("a.flare"), Some(FileFormat::Flare));
    assert_eq!(format("a.flare.fla"), Some(FileFormat::Fla));
    assert_eq!(format("a.fla.flare"), Some(FileFormat::Flare));
    assert_eq!(format("dir/DOMDocument.xml"), Some(FileFormat::Xfl));
    assert_eq!(format("a"), None);
    assert_eq!(format("a.swf"), None);
    assert_eq!(format("a.fla.bak"), None);
    assert_eq!(format(".fla"), None, "a bare dot-file has no extension");
}

#[test]
fn a_typed_name_that_says_no_format_gets_fla_added() {
    let resolved = |name: &str| resolve_save_path(Path::new(name));
    assert_eq!(
        resolved("drawing"),
        (PathBuf::from("drawing.fla"), FileFormat::Fla)
    );
    assert_eq!(
        resolved("drawing.v2"),
        (PathBuf::from("drawing.v2.fla"), FileFormat::Fla)
    );
    assert_eq!(
        resolved("movie.swf"),
        (PathBuf::from("movie.swf.fla"), FileFormat::Fla)
    );
    assert_eq!(
        resolved("a.FLA"),
        (PathBuf::from("a.FLA"), FileFormat::Fla),
        "a name that says a format is kept as typed"
    );
    assert_eq!(
        resolved("a.flare.fla"),
        (PathBuf::from("a.flare.fla"), FileFormat::Fla)
    );
    assert_eq!(
        resolved("d/a.xfl"),
        (PathBuf::from("d/a.xfl"), FileFormat::Xfl)
    );
}

#[test]
fn a_new_xfl_is_a_folder_named_after_the_file_and_an_existing_one_is_its_own_folder() {
    let dir = tempfile::tempdir().expect("dir");
    let new = dir.path().join("a.xfl");
    assert_eq!(xfl_folder(&new), dir.path().join("a"));
    // The folder of a document that is already there is the folder the stub is in, so a second
    // save writes it back and does not nest another folder in it.
    let existing = dir.path().join("b");
    std::fs::create_dir(&existing).expect("dir");
    std::fs::write(existing.join("DOMDocument.xml"), "<x/>").expect("write");
    std::fs::write(existing.join("b.xfl"), "PROXY-CS5").expect("write");
    assert_eq!(xfl_folder(&existing.join("b.xfl")), existing);
    assert_eq!(xfl_folder(&existing.join("DOMDocument.xml")), existing);
    assert_eq!(xfl_folder(&existing), existing);
    // A new stub beside another document's files does not take that document's folder.
    assert_eq!(
        xfl_folder(&existing.join("other.xfl")),
        existing.join("other")
    );
}

fn imported() -> flare_document::Document {
    flare_import::import_bytes(FLA.to_vec())
        .expect("imports")
        .document
}

fn renamed(name: &str) -> flare_document::Document {
    let mut session = EditorSession::new(imported());
    let scene = session.document().scenes()[0].id();
    let layer = session.document().scenes()[0].timeline().layers()[0].id();
    session
        .apply(vec![Command::UpdateLayer {
            scene,
            layer,
            change: LayerChange::Name(name.to_owned()),
        }])
        .expect("renamed");
    session.document().clone()
}

#[test]
fn an_unchanged_fla_is_saved_back_byte_for_byte_and_opens_as_backed_by_the_file() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("a.fla");
    std::fs::write(&path, FLA).expect("write");
    let loaded = load(&path).expect("loads");
    assert!(loaded.backed);
    assert_eq!(loaded.path, path);
    let saved = save(&loaded.document, &path).expect("saves");
    assert_eq!(saved.format, FileFormat::Fla);
    assert_eq!(std::fs::read(&path).expect("read"), FLA);
    let summary = saved.summary.expect("a report");
    assert!(summary.exact);
    assert!(summary.rebuilt.is_empty());
    let names: Vec<_> = std::fs::read_dir(dir.path())
        .expect("dir")
        .flatten()
        .collect();
    assert_eq!(names.len(), 1, "no temporary file or .flare is left");
}

#[test]
fn an_edited_fla_is_replaced_with_the_edit_and_reports_what_was_rebuilt() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("a.fla");
    std::fs::write(&path, FLA).expect("write");
    let saved = save(&renamed("Edited layer"), &path).expect("saves");
    let summary = saved.summary.expect("a report");
    assert!(!summary.exact);
    assert_eq!(summary.rebuilt, ["DOMDocument.xml"]);
    let reopened = load(&path).expect("reopens");
    assert_eq!(
        reopened.document.scenes()[0].timeline().layers()[0].name(),
        "Edited layer"
    );
}

#[test]
fn a_read_only_fla_is_refused_and_stays_as_it_was() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("a.fla");
    std::fs::write(&path, FLA).expect("write");
    let mut permissions = std::fs::metadata(&path).expect("meta").permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(&path, permissions).expect("chmod");
    let error = save(&renamed("x"), &path).expect_err("refused");
    assert!(matches!(error, SaveError::Export { .. }));
    assert!(error.to_string().contains("read-only"), "{error}");
    assert_eq!(std::fs::read(&path).expect("read"), FLA);
}

#[cfg(unix)]
#[test]
fn a_fla_in_a_folder_that_cannot_be_written_fails_and_keeps_the_old_file() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("a.fla");
    std::fs::write(&path, FLA).expect("write");
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o555)).expect("chmod");
    let result = save(&renamed("x"), &path);
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o755)).expect("chmod");
    if std::fs::write(dir.path().join("probe"), b"").is_ok() {
        return; // running as a user the permission does not bind (root): nothing to test
    }
    assert!(result.is_err());
    assert_eq!(std::fs::read(&path).expect("read"), FLA);
}

#[test]
fn saving_xfl_writes_a_folder_with_a_stub_and_saving_it_again_replaces_it_in_place() {
    let dir = tempfile::tempdir().expect("dir");
    let asked = dir.path().join("scene.xfl");
    // A document with no original to carry gets a stub named as the file was.
    let first = save(&imported().with_origin(None), &asked).expect("saves");
    assert_eq!(first.format, FileFormat::Xfl);
    assert_eq!(first.path, dir.path().join("scene").join("scene.xfl"));
    assert!(dir.path().join("scene").join("DOMDocument.xml").is_file());
    assert!(!asked.exists(), "the stub is inside the folder");
    // Opening it by the stub, the folder or the document file is the same document.
    let by_stub = load(&first.path).expect("loads");
    assert!(by_stub.backed);
    assert_eq!(by_stub.path, first.path);
    let by_folder = load(&dir.path().join("scene")).expect("loads");
    assert_eq!(by_folder.path, first.path);
    // Saved again after an edit: same place, the edit is there, nothing nested.
    let again = save(&renamed("Edited").with_origin(None), &first.path).expect("saves");
    assert_eq!(again.path, first.path);
    let reopened = load(&again.path).expect("loads");
    assert_eq!(
        reopened.document.scenes()[0].timeline().layers()[0].name(),
        "Edited"
    );
    let siblings: Vec<_> = std::fs::read_dir(dir.path())
        .expect("dir")
        .flatten()
        .map(|e| e.file_name())
        .collect();
    assert_eq!(siblings.len(), 1, "no staging folder is left: {siblings:?}");
}

#[test]
fn an_xfl_made_from_a_fla_that_held_its_own_stub_keeps_that_stub_and_reopens_by_it() {
    let dir = tempfile::tempdir().expect("dir");
    let saved = save(&imported(), &dir.path().join("scene.xfl")).expect("saves");
    assert_eq!(saved.path, dir.path().join("scene").join("handmade.xfl"));
    assert!(load(&saved.path).expect("loads").backed);
}

#[test]
fn an_xfl_save_never_replaces_a_folder_that_is_not_an_xfl_document() {
    let dir = tempfile::tempdir().expect("dir");
    let other = dir.path().join("scene");
    std::fs::create_dir(&other).expect("dir");
    std::fs::write(other.join("precious.txt"), "keep").expect("write");
    let error = save(&imported(), &dir.path().join("scene.xfl")).expect_err("refused");
    assert!(error.to_string().contains("not an XFL folder"), "{error}");
    assert_eq!(
        std::fs::read_to_string(other.join("precious.txt")).expect("read"),
        "keep"
    );
}

#[test]
fn the_three_formats_convert_into_each_other_and_back() {
    let dir = tempfile::tempdir().expect("dir");
    let source = dir.path().join("a.fla");
    std::fs::write(&source, FLA).expect("write");
    let document = load(&source).expect("loads").document;
    let flare = dir.path().join("b.flare");
    save(&document, &flare).expect("flare");
    let from_flare = load(&flare).expect("loads");
    assert!(from_flare.backed);
    let xfl = save(&from_flare.document, &dir.path().join("c.xfl")).expect("xfl");
    let from_xfl = load(&xfl.path).expect("loads");
    assert!(from_xfl.backed);
    let fla = dir.path().join("d.fla");
    save(&from_xfl.document, &fla).expect("fla");
    let last = load(&fla).expect("loads").document;
    assert_eq!(last.scenes().len(), document.scenes().len());
    assert_eq!(last.symbols().len(), document.symbols().len());
}

#[test]
fn a_file_called_fla_that_is_not_an_animate_document_is_refused() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("m.fla");
    // A file called .fla that is not an Animate document at all is refused, not saved over.
    std::fs::write(&path, b"not a document").expect("write");
    assert!(load(&path).is_err());
}

#[test]
fn a_movie_or_a_pre_cs5_fla_is_never_overwritten_by_a_rebuilt_copy() {
    let dir = tempfile::tempdir().expect("dir");
    let movie = dir.path().join("m.swf");
    std::fs::write(&movie, b"FWS").expect("write");
    assert!(matches!(
        save(&imported(), &movie),
        Err(SaveError::Movie(_))
    ));
    assert_eq!(std::fs::read(&movie).expect("read"), b"FWS");
    let legacy = dir.path().join("old.fla");
    let mut bytes = flare_import::legacy::CFB_MAGIC.to_vec();
    bytes.extend_from_slice(b"the rest of an old binary document");
    std::fs::write(&legacy, &bytes).expect("write");
    assert!(matches!(
        save(&imported(), &legacy),
        Err(SaveError::LegacyFla(_))
    ));
    assert_eq!(std::fs::read(&legacy).expect("read"), bytes);
}

#[test]
fn only_a_document_with_its_original_of_the_right_kind_is_saved_in_place() {
    let kept = imported();
    assert!(saves_in_place(&kept, Path::new("a.fla")));
    assert!(
        !saves_in_place(&kept, Path::new("a.xfl")),
        "a .fla's original is no folder"
    );
    assert!(saves_in_place(&kept, Path::new("a.flare")));
    assert!(!saves_in_place(
        &kept.clone().with_origin(None),
        Path::new("a.fla")
    ));
    assert!(!saves_in_place(&kept, Path::new("a.swf")));
}
