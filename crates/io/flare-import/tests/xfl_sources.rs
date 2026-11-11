//! Where an XFL document comes from: a folder, its .xfl stub, a .fla zip or a zip wrapped round
//! one, and the names a source refuses to read.

#![allow(clippy::expect_used)]

use std::io::{Cursor, Write};

use flare_document::Document;
use flare_import::{DirSource, FileSource, ImportError, ZipSource, import_bytes, import_path};

mod xfl_support;
use xfl_support::{DOT, MAIN};

fn write_xfl_dir(dir: &std::path::Path) {
    std::fs::create_dir_all(dir.join("LIBRARY")).expect("mkdir");
    std::fs::write(dir.join("DOMDocument.xml"), MAIN).expect("write");
    std::fs::write(dir.join("LIBRARY/Dot.xml"), DOT).expect("write");
    std::fs::write(dir.join("project.xfl"), "PROXY-CS5").expect("write");
}

fn shape_of(doc: &Document) -> (usize, Vec<usize>) {
    let layers = doc.scenes()[0].timeline().layers();
    (
        doc.symbols().len(),
        layers.iter().map(|l| l.spans().len()).collect(),
    )
}

fn zip_files(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default();
    for (name, body) in files {
        zip.start_file(*name, options).expect("entry");
        zip.write_all(body).expect("body");
    }
    zip.finish().expect("finish").into_inner()
}

#[test]
fn directories_zip_files_and_xfl_stubs_import_the_same_document() {
    let dir = tempfile::tempdir().expect("dir");
    let root = dir.path().join("proj");
    write_xfl_dir(&root);

    let from_dir = import_path(&root).expect("dir");
    let from_stub = import_path(&root.join("project.xfl")).expect("stub");

    let fla = dir.path().join("proj.fla");
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&fla).expect("create"));
    let options = zip::write::SimpleFileOptions::default();
    for (name, body) in [("DOMDocument.xml", MAIN), ("LIBRARY/Dot.xml", DOT)] {
        zip.start_file(name, options).expect("entry");
        zip.write_all(body.as_bytes()).expect("body");
    }
    zip.finish().expect("finish");
    let from_zip = import_path(&fla).expect("zip");

    assert_eq!(shape_of(&from_dir.document), shape_of(&from_stub.document));
    assert_eq!(shape_of(&from_dir.document), shape_of(&from_zip.document));
    assert_eq!(from_dir.diagnostics.len(), from_zip.diagnostics.len());
    assert_ne!(
        from_dir.document.id(),
        from_zip.document.id(),
        "every import is a new document"
    );
}

#[test]
fn zip_wrappers_import_their_single_fla_without_keeping_the_wrapper() {
    let inner = zip_files(&[
        ("DOMDocument.xml", MAIN.as_bytes()),
        ("LIBRARY/Dot.xml", DOT.as_bytes()),
    ]);
    let wrapper = zip_files(&[("nested/project.fla", &inner)]);
    let from_bytes = import_bytes(wrapper.clone()).expect("in-memory wrapper");
    assert_eq!(shape_of(&from_bytes.document), (1, vec![2, 1, 0]));
    assert!(from_bytes.document.origin().is_none());

    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("wrapped.zip");
    std::fs::write(&path, wrapper).expect("write");
    let from_path = import_path(&path).expect("path wrapper");
    assert_eq!(
        shape_of(&from_path.document),
        shape_of(&from_bytes.document)
    );
    assert!(from_path.document.origin().is_none());
}

#[test]
fn zip_wrappers_with_multiple_flas_are_refused() {
    let inner = zip_files(&[("DOMDocument.xml", MAIN.as_bytes())]);
    let wrapper = zip_files(&[("one.fla", &inner), ("two.FLA", &inner)]);
    let error = import_bytes(wrapper).expect_err("ambiguous wrapper");
    assert!(error.to_string().contains("more than one .fla"), "{error}");
}

#[test]
fn non_xfl_files_are_recognised_and_explained() {
    let dir = tempfile::tempdir().expect("dir");
    let legacy = dir.path().join("old.fla");
    std::fs::write(
        &legacy,
        [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1, 0, 0],
    )
    .expect("write");
    let e = import_path(&legacy).expect_err("legacy");
    assert!(e.to_string().contains("legacy binary FLA"), "{e}");

    let text = dir.path().join("a.txt");
    std::fs::write(&text, "hello").expect("write");
    assert!(matches!(import_path(&text), Err(ImportError::NotXfl(_))));
    assert!(matches!(
        import_path(&dir.path().join("missing")),
        Err(ImportError::Io(_))
    ));

    let empty_zip = dir.path().join("e.fla");
    zip::ZipWriter::new(std::fs::File::create(&empty_zip).expect("create"))
        .finish()
        .expect("finish");
    assert!(
        matches!(import_path(&empty_zip), Err(ImportError::NotXfl(_))),
        "a zip without DOMDocument.xml"
    );
}

#[test]
fn sources_refuse_names_that_escape_the_document() {
    let dir = tempfile::tempdir().expect("dir");
    let root = dir.path().join("proj");
    write_xfl_dir(&root);
    std::fs::write(dir.path().join("secret.txt"), "secret").expect("write");
    let source = DirSource(root.clone());
    for name in [
        "../secret.txt",
        "/etc/passwd",
        "LIBRARY/../../secret.txt",
        "a\\b",
    ] {
        assert!(source.read(name).is_err(), "{name}");
    }
    assert_eq!(source.read("nope.xml").expect("read"), None);
    assert!(source.read("DOMDocument.xml").expect("read").is_some());

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(dir.path().join("secret.txt"), root.join("LIBRARY/link.xml"))
            .expect("symlink");
        assert!(
            source.read("LIBRARY/link.xml").is_err(),
            "symlinks out of the document are refused"
        );
    }

    let fla = dir.path().join("p.fla");
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&fla).expect("create"));
    zip.start_file("DOMDocument.xml", zip::write::SimpleFileOptions::default())
        .expect("entry");
    zip.write_all(b"<DOMDocument/>").expect("body");
    zip.finish().expect("finish");
    let zipped = ZipSource::open(&fla).expect("open");
    assert!(zipped.read("../x").is_err());
    assert_eq!(zipped.read("missing.xml").expect("read"), None);
}
