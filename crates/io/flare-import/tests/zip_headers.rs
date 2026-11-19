//! An entry in a zip has two names: the one in the local file header beside the data, and the one
//! in the central directory at the end. Tools disagree about which one counts, and an archive
//! whose two names differ is how a payload passes an inspection that sees one name and lands
//! somewhere named by the other. Every reader here goes through the central directory, so the
//! local name is never followed and never becomes a path.
#![allow(clippy::expect_used)]

use std::io::{Cursor, Write as _};
use std::path::Path;

use flare_import::import_bytes;

/// A whole Flash document as a zip of XFL files, the payload a wrapper carries.
fn sample_fla() -> Vec<u8> {
    std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sample.fla"))
        .expect("sample.fla")
}

/// A zip with one entry whose local header and central directory both say `name`.
fn zip_of(name: &str, body: &[u8]) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file(name, zip::write::SimpleFileOptions::default())
        .expect("member");
    zip.write_all(body).expect("member bytes");
    zip.finish().expect("zip").into_inner()
}

/// Rewrites the name in the central directory and leaves the local header as it was, the way an
/// archive whose two headers disagree is built. Both names are eight bytes long, so everything
/// after the name stays where the format put it.
fn rename_in_central_directory(archive: &mut [u8], name: &str) {
    assert_eq!(name.len(), 8, "the fixture names are eight bytes");
    let start = archive
        .windows(4)
        .position(|window| window == b"PK\x01\x02")
        .expect("central directory header");
    archive[start + 46..start + 46 + 8].copy_from_slice(name.as_bytes());
}

#[test]
fn the_central_directory_names_the_entry_and_the_local_name_is_ignored() {
    // The payload is a whole `.fla`, so an importer that took the name beside the data would
    // unwrap it and import the document the archive's other name hides.
    let mut disguised = zip_of("evil.fla", &sample_fla());
    rename_in_central_directory(&mut disguised, "note.txt");

    let mut archive = zip::ZipArchive::new(Cursor::new(&disguised)).expect("archive");
    assert_eq!(
        archive.by_index(0).expect("entry").name().expect("name"),
        "note.txt",
        "the central directory is what every reader here sees"
    );

    assert!(
        import_bytes(disguised).is_err(),
        "the local name was followed instead of the central one"
    );
}

#[test]
fn an_entry_is_read_under_the_name_the_central_directory_gives_it() {
    // The same disagreement the other way round: the central directory names the Flash file, so
    // that is the name the entry has, for this importer and for everything else that reads it.
    let mut named = zip_of("note.txt", &sample_fla());
    rename_in_central_directory(&mut named, "evil.fla");

    let imported = import_bytes(named).expect("the central directory names a .fla");
    assert_eq!(imported.document.scenes().len(), 1);
}
