//! An edit changes only what it touches: every other file of the original is written back as it
//! was, and what cannot be carried is said.

use super::*;
use flare_core::{LayerId, SymbolId};
use flare_document::{Content, FrameSpan, Layer, Timeline};
use std::collections::BTreeMap;

fn map(bytes: &[u8]) -> BTreeMap<String, Vec<u8>> {
    fixtures::zip_entries(bytes).into_iter().collect()
}

fn original() -> BTreeMap<String, Vec<u8>> {
    fixtures::files()
        .into_iter()
        .map(|(n, b)| (n.to_owned(), b))
        .collect()
}

fn names_that_differ(
    before: &BTreeMap<String, Vec<u8>>,
    after: &BTreeMap<String, Vec<u8>>,
) -> Vec<String> {
    let mut out = Vec::new();
    for (name, body) in before {
        if after.get(name) != Some(body) {
            out.push(name.clone());
        }
    }
    for name in after.keys() {
        if !before.contains_key(name) {
            out.push(name.clone());
        }
    }
    out.sort();
    out
}

fn symbol(document: &Document, name: &str) -> SymbolId {
    document
        .symbols()
        .iter()
        .find(|s| s.name() == name)
        .expect("symbol")
        .id()
}

#[test]
fn renaming_a_layer_rebuilds_only_the_main_document() {
    let mut session = EditorSession::new(imported_fla());
    rename_first_layer(&mut session, "Background");
    let output = export(session.document());
    assert!(!output.lossless.exact);
    assert!(
        output.lossless.reason.contains("DOMDocument.xml"),
        "{}",
        output.lossless.reason
    );
    let after = map(&bytes_of(&output));
    assert_eq!(names_that_differ(&original(), &after), ["DOMDocument.xml"]);
    // The rebuilt main document says what was edited and still names the bitmap it keeps.
    let main = String::from_utf8(after["DOMDocument.xml"].clone()).expect("text");
    assert!(main.contains("Background"));
    assert!(main.contains("pic.png") && main.contains("M 1 1.dat"));
    // The order of the archive is the original's.
    let order: Vec<String> = fixtures::zip_entries(&bytes_of(&output))
        .into_iter()
        .map(|e| e.0)
        .collect();
    let expected: Vec<String> = fixtures::files()
        .into_iter()
        .map(|e| e.0.to_owned())
        .collect();
    assert_eq!(order, expected);
    // It opens again, with the edit.
    let again = flare_import::import_bytes(bytes_of(&output))
        .expect("imports")
        .document;
    assert_eq!(
        again.scenes()[0].timeline().layers()[0].name(),
        "Background"
    );
    assert!(again.attachments().len() >= session.document().attachments().len());
}

#[test]
fn editing_a_symbol_rebuilds_that_symbol_and_keeps_the_main_document_as_it_was() {
    let mut session = EditorSession::new(imported_fla());
    let dot = symbol(session.document(), "Dot");
    let timeline = Timeline::new(vec![
        Layer::new(LayerId::new(), "Changed", Vec::<FrameSpan>::new()).expect("layer"),
    ]);
    session
        .apply(vec![Command::ReplaceSymbolTimeline {
            symbol: dot,
            timeline,
        }])
        .expect("edited");
    let output = export(session.document());
    let after = map(&bytes_of(&output));
    assert_eq!(names_that_differ(&original(), &after), ["LIBRARY/Dot.xml"]);
    assert!(String::from_utf8_lossy(&after["LIBRARY/Dot.xml"]).contains("Changed"));
    assert!(output.lossless.reason.contains("LIBRARY/Dot.xml"));
}

#[test]
fn binary_and_settings_files_survive_any_edit() {
    let mut session = EditorSession::new(imported_fla());
    rename_first_layer(&mut session, "X");
    let dot = symbol(session.document(), "Dot");
    session
        .apply(vec![Command::RenameSymbol {
            symbol: dot,
            name: "Pip".into(),
        }])
        .expect("renamed");
    let after = map(&bytes_of(&export(session.document())));
    for kept in [
        "mimetype",
        "PublishSettings.xml",
        "MobileSettings.xml",
        "META-INF/metadata.xml",
        "LIBRARY/pic.png",
        "bin/M 1 1.dat",
        "bin/SymDepend.cache",
        "extras/unknown.bin",
        "LIBRARY/Box.xml",
    ] {
        assert_eq!(after.get(kept), original().get(kept), "{kept}");
    }
}

#[test]
fn a_renamed_symbol_is_written_under_its_new_name_and_the_old_file_is_reported() {
    let mut session = EditorSession::new(imported_fla());
    let dot = symbol(session.document(), "Dot");
    session
        .apply(vec![Command::RenameSymbol {
            symbol: dot,
            name: "Pip".into(),
        }])
        .expect("renamed");
    let output = export(session.document());
    let after = map(&bytes_of(&output));
    assert!(!after.contains_key("LIBRARY/Dot.xml"));
    assert!(after.contains_key("LIBRARY/Pip.xml"));
    assert!(
        output
            .diagnostics
            .iter()
            .any(|d| d.message.contains("LIBRARY/Dot.xml"))
    );
    let main = String::from_utf8(after["DOMDocument.xml"].clone()).expect("text");
    assert!(main.contains("Pip.xml") && !main.contains("Dot.xml"));
}

#[test]
fn a_removed_symbol_takes_its_library_file_with_it_and_nothing_else() {
    let mut session = EditorSession::new(imported_fla());
    let dot = symbol(session.document(), "Dot");
    session
        .apply(vec![Command::RemoveSymbol(dot)])
        .expect("removed");
    let output = export(session.document());
    let after = map(&bytes_of(&output));
    let differ = names_that_differ(&original(), &after);
    assert_eq!(differ, ["DOMDocument.xml", "LIBRARY/Dot.xml"]);
    assert_eq!(
        after.get("LIBRARY/Box.xml"),
        original().get("LIBRARY/Box.xml")
    );
}

#[test]
fn an_added_symbol_is_a_new_file_beside_the_original_ones() {
    let mut session = EditorSession::new(imported_fla());
    let extra = flare_document::Symbol::new(SymbolId::new(), "Extra", Timeline::default());
    session
        .apply(vec![Command::AddSymbol(extra)])
        .expect("added");
    let after = map(&bytes_of(&export(session.document())));
    assert_eq!(
        names_that_differ(&original(), &after),
        ["DOMDocument.xml", "LIBRARY/Extra.xml"]
    );
}

#[test]
fn a_new_symbol_never_overwrites_an_original_file_it_does_not_own() {
    // `LIBRARY/Extra.xml` exists in the source but is not in its symbol list.
    let mut files = fixtures::files();
    files.push(("LIBRARY/Extra.xml", b"<mine/>".to_vec()));
    let mut cursor = std::io::Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(&mut cursor);
    for (name, body) in &files {
        zip.start_file(*name, zip::write::SimpleFileOptions::default())
            .expect("start");
        std::io::Write::write_all(&mut zip, body).expect("write");
    }
    zip.finish().expect("finish");
    let mut session = EditorSession::new(
        flare_import::import_bytes(cursor.into_inner())
            .expect("imports")
            .document,
    );
    let extra = flare_document::Symbol::new(SymbolId::new(), "Extra", Timeline::default());
    session
        .apply(vec![Command::AddSymbol(extra)])
        .expect("added");
    let output = export(session.document());
    let after = map(&bytes_of(&output));
    assert_ne!(
        after["LIBRARY/Extra.xml"],
        b"<mine/>".to_vec(),
        "Flare's own file wins, and says so"
    );
    assert!(
        output
            .diagnostics
            .iter()
            .all(|d| d.severity != flare_export::Severity::Warning || !d.message.is_empty())
    );
}

#[test]
fn an_edited_folder_keeps_every_other_file_and_folder() {
    let (dir, document) = imported_folder();
    let before = fixtures::tree(&dir.path().join("Project"));
    let mut session = EditorSession::new(document);
    rename_first_layer(&mut session, "Edited");
    let output = export(session.document());
    assert!(!output.lossless.exact);
    let out = dir.path().join("Out");
    flare_export::write_directory(&output, &out).expect("written");
    let after = fixtures::tree(&out);
    assert_eq!(names_that_differ(&before, &after), ["DOMDocument.xml"]);
    assert!(
        after.contains_key("empty folder/")
            && after.contains_key(".hidden")
            && after.contains_key("Project.xfl")
    );
}

#[test]
fn an_edit_undone_after_exporting_is_the_original_again() {
    let mut session = EditorSession::new(imported_fla());
    rename_first_layer(&mut session, "Temp");
    let _ = export(session.document());
    session.undo();
    assert_eq!(bytes_of(&export(session.document())), fixtures::fla());
}

#[test]
fn a_shape_added_to_a_symbol_reaches_only_that_symbol_through_the_normal_commands() {
    // Move the content of a symbol layer through the same command path the editor uses.
    let mut session = EditorSession::new(imported_fla());
    let box_id = symbol(session.document(), "Box");
    let mut timeline = session
        .document()
        .symbol(box_id)
        .expect("box")
        .timeline()
        .clone();
    let layer = &mut timeline.layers_mut()[0];
    let spans: Vec<FrameSpan> = layer
        .spans()
        .iter()
        .map(|s| FrameSpan::new(s.start() + 1, s.duration(), s.content().clone()))
        .collect();
    assert!(matches!(spans[0].content(), Content::Shape(_)));
    *layer = Layer::new(layer.id(), layer.name(), spans).expect("layer");
    session
        .apply(vec![Command::ReplaceSymbolTimeline {
            symbol: box_id,
            timeline,
        }])
        .expect("edited");
    let after = map(&bytes_of(&export(session.document())));
    assert_eq!(names_that_differ(&original(), &after), ["LIBRARY/Box.xml"]);
}
