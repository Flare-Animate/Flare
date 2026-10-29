//! Small Flash documents written by other tools' conventions, imported from disk: a folder, a
//! zipped `.fla`, and archives with damaged trailers. They are the same fixtures the first
//! version of Flare used, so what that version could open this one must too.
#![allow(clippy::expect_used)]

use std::path::{Path, PathBuf};

use flare_import::{Severity, import_path};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn a_folder_and_a_zip_of_it_import_the_same_document() {
    let folder = import_path(&fixture("sample_xfl")).expect("folder");
    let zipped = import_path(&fixture("sample.fla")).expect("zip");
    for imported in [&folder, &zipped] {
        assert_eq!(imported.document.scenes().len(), 1);
        assert!(imported.document.symbols().is_empty());
    }
    assert_eq!(folder.document.stage(), zipped.document.stage());
    assert_eq!(folder.document.frame_rate(), zipped.document.frame_rate());
    assert!(folder.diagnostics.is_empty() && zipped.diagnostics.is_empty());
}

#[test]
fn the_handmade_document_brings_its_symbol_tween_and_says_what_it_leaves_behind() {
    let folder = import_path(&fixture("handmade_xfl")).expect("folder");
    let zipped = import_path(&fixture("handmade.fla")).expect("zip");
    for imported in [&folder, &zipped] {
        let doc = &imported.document;
        assert_eq!((doc.stage().width(), doc.stage().height()), (320, 240));
        assert_eq!(
            (doc.frame_rate().numerator(), doc.frame_rate().denominator()),
            (12, 1)
        );
        assert_eq!(doc.symbols().len(), 1);
        assert_eq!(doc.symbols()[0].name(), "Ball");
        let layers = doc.scenes()[0].timeline().layers();
        let ball = layers
            .iter()
            .find(|l| l.name() == "Ball")
            .expect("the Ball layer");
        assert_eq!(ball.spans().len(), 2, "two keyframes");
        assert_eq!(
            ball.spans()[0].tween(),
            flare_document::Tween::Linear,
            "the classic tween came across"
        );
        // The ground's outline asks for square ends and mitered corners, and gets them.
        let outline = layers
            .iter()
            .flat_map(|l| l.spans())
            .find_map(|span| match span.content() {
                flare_document::Content::Shape(shape) => shape.stroke(),
                _ => None,
            })
            .expect("a stroked shape");
        assert_eq!(
            (outline.cap(), outline.join(), outline.width()),
            (
                flare_document::Cap::Square,
                flare_document::Join::Miter,
                3.0
            )
        );
        let unsupported: Vec<&str> = imported
            .diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Unsupported)
            .map(|d| d.message.as_str())
            .collect();
        assert!(
            unsupported.iter().any(|m| m.contains("frame scripts")),
            "scripts are reported, not hidden: {unsupported:?}"
        );
    }
    assert_eq!(
        folder.diagnostics.len(),
        zipped.diagnostics.len(),
        "a folder and a zip of it tell the same story"
    );
}

#[test]
fn archives_with_damaged_trailers_open_or_are_refused_but_never_confuse_the_importer() {
    // A central directory that overstates its size, as real files in the wild have.
    let stale = import_path(&fixture("stale_trailer.fla"));
    // A local header the central directory does not list.
    let orphan = import_path(&fixture("orphan_local.fla"));
    for (name, result) in [("stale", stale), ("orphan", orphan)] {
        match result {
            Ok(imported) => assert_eq!(imported.document.scenes().len(), 1, "{name}"),
            Err(error) => assert!(!error.to_string().is_empty(), "{name}: said nothing"),
        }
    }
}

#[test]
fn every_prefix_of_a_fixture_archive_is_refused_or_imported_without_panicking() {
    let bytes = std::fs::read(fixture("handmade.fla")).expect("fixture");
    for cut in (0..bytes.len()).step_by(7) {
        let _ = flare_import::import_fla_bytes(bytes[..cut].to_vec());
    }
    // And with one byte at a time flipped.
    for at in (0..bytes.len()).step_by(11) {
        let mut damaged = bytes.clone();
        damaged[at] ^= 0xff;
        let _ = flare_import::import_fla_bytes(damaged);
    }
}
