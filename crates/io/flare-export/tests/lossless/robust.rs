//! Damaged and hostile sources never panic, and never produce a copy that is not the source.

use super::*;

/// A deterministic stream of numbers, so a failure can be reproduced.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

/// The property that matters: if a file imports at all, exporting the import unchanged is that
/// very file, and says so. Otherwise it was refused, which is fine, but never half-opened.
fn check(bytes: Vec<u8>, what: &str) -> bool {
    let Ok(imported) = flare_import::import_bytes(bytes.clone()) else {
        return false;
    };
    let Some(origin) = imported.document.origin() else {
        // Not kept: then no export may claim to be the original.
        let output = export(&imported.document);
        assert!(
            !output.lossless.exact,
            "{what}: claimed lossless without a source"
        );
        return true;
    };
    assert_eq!(origin.len(), bytes.len(), "{what}");
    let output = export(&imported.document);
    assert!(output.lossless.exact, "{what}: {}", output.lossless.reason);
    assert_eq!(bytes_of(&output), bytes, "{what}");
    true
}

#[test]
fn a_file_cut_at_any_length_is_refused_or_exported_unchanged() {
    let whole = fixtures::fla();
    let mut imported = 0;
    for len in 0..=whole.len() {
        if check(whole[..len].to_vec(), &format!("cut at {len}")) {
            imported += 1;
        }
    }
    assert!(imported >= 1, "the whole file imports");
    assert!(imported < whole.len(), "most cuts are refused");
}

#[test]
fn a_file_damaged_at_random_is_refused_or_exported_unchanged() {
    let whole = fixtures::fla();
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    let mut imported = 0;
    for round in 0..600 {
        let mut bytes = whole.clone();
        for _ in 0..=(rng.next() % 4) {
            let at = (rng.next() as usize) % bytes.len();
            bytes[at] = rng.next() as u8;
        }
        if check(bytes, &format!("round {round}")) {
            imported += 1;
        }
    }
    // Damage to the stored bitmaps and settings is invisible to the importer, which is the point:
    // those files come back exactly as damaged as they were read.
    assert!(imported > 0);
}

#[test]
fn damaged_input_never_claims_to_be_lossless_without_its_source() {
    // A document whose source was dropped says so, whatever happened to the file.
    let imported = flare_import::import_bytes(fixtures::fla())
        .expect("imports")
        .document;
    let output = export(&imported.with_origin(None));
    assert!(!output.lossless.exact);
}

#[test]
fn an_archive_with_unsafe_entry_names_is_kept_whole_but_never_unpacked_outside() {
    let mut cursor = std::io::Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(&mut cursor);
    let options = zip::write::SimpleFileOptions::default();
    for (name, body) in fixtures::files() {
        zip.start_file(name, options).expect("start");
        std::io::Write::write_all(&mut zip, &body).expect("write");
    }
    zip.start_file("../escape.txt", options).expect("start");
    std::io::Write::write_all(&mut zip, b"gotcha").expect("write");
    zip.finish().expect("finish");
    let bytes = cursor.into_inner();

    let document = flare_import::import_bytes(bytes.clone())
        .expect("imports")
        .document;
    // Unchanged: the file is written back whole, as the bytes it was.
    let output = export(&document);
    assert!(output.lossless.exact);
    assert_eq!(bytes_of(&output), bytes);
    // Unpacking it into a folder is refused rather than writing outside, and writes nothing.
    let dir = tempfile::tempdir().expect("dir");
    let target = dir.path().join("inside").join("Out");
    let error = flare_export::write_directory(&output, &target).expect_err("refused");
    assert!(error.to_string().contains("cannot be unpacked"), "{error}");
    assert!(!target.exists() && !dir.path().join("escape.txt").exists());
    assert!(!dir.path().join("inside").join("escape.txt").exists());

    // Edited: everything safe is carried, the unsafe entry is left out and reported.
    let mut session = EditorSession::new(document);
    rename_first_layer(&mut session, "E");
    let output = export(session.document());
    let names: Vec<String> = fixtures::zip_entries(&bytes_of(&output))
        .into_iter()
        .map(|e| e.0)
        .collect();
    assert!(!names.iter().any(|n| n.contains("escape")));
    assert!(names.contains(&"bin/M 1 1.dat".to_owned()));
    assert!(
        output
            .diagnostics
            .iter()
            .any(|d| d.severity == flare_export::Severity::Warning && d.message.contains("escape")),
        "{:?}",
        output.diagnostics
    );
    assert!(output.lossless.reason.contains("could not be carried over"));
}
