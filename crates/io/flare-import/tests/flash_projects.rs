//! The Flash files that point at other files: a component library (.swc), a Flash project
//! (.flp), FutureSplash (.spl) and Generator templates (.swt), and the Adobe MP4 profiles.
//! Fixtures are built here, so a test fails when the reading is wrong and not when a foreign
//! tool changes.
#![allow(clippy::expect_used)]

use std::io::Write as _;
use std::path::Path;

use flare_import::{
    ImportError, PathKind, Severity, classify, foreign_kind, import_bytes, import_path,
};

mod swf_support;
use swf_support::moving_square;

/// A zip holding `members`, the way a component library is packed.
fn zip_of(members: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default();
    for (name, contents) in members {
        zip.start_file(*name, options).expect("member");
        zip.write_all(contents).expect("member bytes");
    }
    zip.finish().expect("zip").into_inner()
}

/// A Flash project file listing `entries`, in Flash 8's own shapes.
fn flp_of(default_file: Option<&str>, files: &[&str]) -> String {
    let mut out = String::from(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n\
         <flash_project name=\"Test\" version=\"1\">\n  \
         <project projectName=\"Test\">\n    <files>\n",
    );
    if let Some(default_file) = default_file {
        out.push_str(&format!(
            "      <project flashDefaultFile=\"{default_file}\"/>\n"
        ));
    }
    for file in files {
        out.push_str(&format!(
            "      <file filename=\"{file}\" filetype=\"Flash\"/>\n"
        ));
    }
    out.push_str("    </files>\n  </project>\n</flash_project>\n");
    out
}

/// Writes `contents` into a temp folder as `name`, and gives the folder back.
fn folder_with(name: &str, contents: &[u8]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join(name), contents).expect("write");
    dir
}

#[test]
fn a_swcs_compiled_library_is_imported_and_said_so() {
    let swc = zip_of(&[
        ("catalog.xml", b"<component/>".to_vec()),
        ("library.swf", moving_square()),
    ]);
    let imported = import_bytes(swc).expect("import");
    assert_eq!(imported.document.symbols().len(), 1);
    assert!(
        imported
            .diagnostics
            .iter()
            .any(|note| { note.severity == Severity::Warning && note.message.contains(".swc") }),
        "the not-kept reason names the .swc: {:?}",
        imported.diagnostics
    );
}

#[test]
fn a_zip_without_a_compiled_library_still_fails_as_an_fla() {
    let zip = zip_of(&[("catalog.xml", b"<component/>".to_vec())]);
    assert!(matches!(import_bytes(zip), Err(ImportError::NotXfl(_))));
}

#[test]
fn an_flp_imports_the_file_flash_opens_by_default() {
    let dir = folder_with("movie.swf", &moving_square());
    std::fs::write(
        dir.path().join("project.flp"),
        flp_of(Some("movie.swf"), &[]),
    )
    .expect("write flp");
    let imported = import_path(&dir.path().join("project.flp")).expect("import");
    assert_eq!(imported.document.symbols().len(), 1);
    assert!(
        imported
            .diagnostics
            .iter()
            .any(|note| note.message.contains("default file is movie.swf"))
    );
}

#[test]
fn an_flp_without_a_default_takes_its_first_listed_document() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("first.swf"), &moving_square()).expect("write");
    std::fs::write(dir.path().join("second.swf"), &moving_square()).expect("write");
    std::fs::write(
        dir.path().join("project.flp"),
        flp_of(None, &["code.as", "first.swf", "second.swf"]),
    )
    .expect("write flp");
    let imported = import_path(&dir.path().join("project.flp")).expect("import");
    assert_eq!(imported.document.symbols().len(), 1);
    assert!(
        imported
            .diagnostics
            .iter()
            .any(|note| note.message.contains("1 more file(s)")),
        "the other file is counted: {:?}",
        imported.diagnostics
    );
}

#[test]
fn an_flp_that_lists_no_document_to_import_says_so() {
    let dir = folder_with(
        "project.flp",
        flp_of(None, &["code.as", "art.png"]).as_bytes(),
    );
    let error = import_path(&dir.path().join("project.flp")).expect_err("refused");
    match error {
        ImportError::Foreign(message) => {
            assert!(message.contains("lists no .fla"), "{message}");
        }
        other => panic!("expected a named refusal, got {other:?}"),
    }
}

#[test]
fn a_text_that_is_not_a_flash_project_is_refused_by_what_it_is() {
    let dir = folder_with("project.flp", b"<html>not a project</html>");
    let error = import_path(&dir.path().join("project.flp")).expect_err("refused");
    match error {
        ImportError::Foreign(message) => {
            assert!(message.contains("no <flash_project> header"), "{message}");
        }
        other => panic!("expected a named refusal, got {other:?}"),
    }
}

#[test]
fn a_default_file_that_is_missing_is_named() {
    let dir = folder_with("project.flp", flp_of(Some("gone.swf"), &[]).as_bytes());
    let error = import_path(&dir.path().join("project.flp")).expect_err("refused");
    match error {
        ImportError::Io(_) | ImportError::NotSwf(_) | ImportError::NotXfl(_) => {}
        other => panic!("expected the missing file's own error, got {other:?}"),
    }
}

#[test]
fn a_project_that_names_another_project_is_refused() {
    let dir = folder_with("project.flp", flp_of(Some("other.flp"), &[]).as_bytes());
    let error = import_path(&dir.path().join("project.flp")).expect_err("refused");
    match error {
        ImportError::Foreign(message) => {
            assert!(
                message.contains("lists no .fla"),
                "the nested project is not a document to import: {message}"
            );
        }
        other => panic!("expected a named refusal, got {other:?}"),
    }
}

#[test]
fn future_splash_and_generator_templates_open_like_movies() {
    for extension in ["spl", "swt"] {
        let dir = folder_with(&format!("movie.{extension}"), &moving_square());
        let path = dir.path().join("movie.swf").with_extension(extension);
        let imported = import_path(&path).unwrap_or_else(|error| panic!("{extension}: {error}"));
        assert_eq!(imported.document.symbols().len(), 1, "{extension}");
    }
}

#[test]
fn the_kinds_are_offered_and_the_rest_are_named() {
    let kind = |name: &str| classify(Path::new(name), false);
    for name in ["a.spl", "a.SWT", "a.swc", "a.flp", "a.f4v", "a.f4p"] {
        assert_eq!(kind(name), Some(PathKind::Import), "{name}");
    }
    for name in [
        "a.abc", "a.asc", "a.aso", "a.swd", "a.amf", "a.f4a", "a.f4b", "a.f4f", "a.f4m",
    ] {
        assert_eq!(kind(name), None, "{name} is not offered");
    }
    assert_eq!(foreign_kind(Path::new("a.f4v")), Some("video"));
    assert_eq!(foreign_kind(Path::new("a.f4p")), Some("video"));
}

#[test]
fn a_flp_read_whole_says_the_files_it_names_cannot_be_reached() {
    let error = import_bytes(flp_of(Some("movie.swf"), &[]).into_bytes()).expect_err("refused");
    match error {
        ImportError::Foreign(message) => {
            assert!(message.contains(".flp"), "{message}");
            assert!(message.contains("open the .fla or .swf"), "{message}");
        }
        other => panic!("expected a named refusal, got {other:?}"),
    }
}
