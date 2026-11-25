//! File > Import > Import to Stage and Import to Library: a Flash movie brought into the open
//! document, from a file on disk or from bytes as the browser has them.

use super::*;

/// A Flash movie of one drawn rectangle, as Flare publishes it.
fn movie() -> Vec<u8> {
    let mut source = workspace();
    rect(&mut source);
    flare_publish::publish_swf(source.document(), &flare_publish::PublishOptions::default())
        .expect("publish")
        .bytes
}

fn symbol_names(ws: &Workspace) -> Vec<String> {
    ws.document()
        .symbols()
        .iter()
        .map(|s| s.name().to_owned())
        .collect()
}

#[test]
fn import_to_stage_puts_the_movie_on_a_new_layer_as_one_undo_step() {
    let mut ws = workspace();
    let layers = layer_count(&ws);
    assert!(ws.is_enabled(MenuAction::ImportToStage));
    ws.perform(MenuAction::ImportToStage);
    assert_eq!(ws.take_dialog(), Some(DialogRequest::ImportToStage));
    ws.import_into_document_bytes("intro.swf", movie(), true);
    assert_eq!(layer_count(&ws), layers + 1);
    let top = &ws.document().scenes()[0].timeline().layers()[0];
    assert_eq!(top.name(), "intro");
    assert!(symbol_names(&ws).contains(&"intro".to_owned()));
    assert!(
        ws.status().is_some_and(|s| s.contains("Imported")),
        "{:?}",
        ws.status()
    );
    // The movie's rectangle is drawn on the stage, through the symbol.
    let scene = ws.document().scenes()[0].id();
    let drawn = flare_eval::evaluate_frame(ws.document(), scene, 0).expect("evaluates");
    assert!(
        !drawn.shapes.is_empty(),
        "the movie's drawing is on the stage"
    );
    ws.perform(MenuAction::Undo);
    assert_eq!(layer_count(&ws), layers);
    assert!(!symbol_names(&ws).contains(&"intro".to_owned()));
}

#[test]
fn import_to_library_from_a_file_leaves_the_stage_alone() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("loop.swf");
    std::fs::write(&path, movie()).expect("write");
    let mut ws = workspace();
    let layers = layer_count(&ws);
    ws.perform(MenuAction::ImportToLibrary);
    assert_eq!(ws.take_dialog(), Some(DialogRequest::ImportToLibrary));
    ws.import_into_document(&path, false);
    assert_eq!(layer_count(&ws), layers);
    assert!(symbol_names(&ws).contains(&"loop".to_owned()));
    assert_eq!(
        ws.path(),
        None,
        "the document is still this one, not the movie"
    );
    // Twice gives the second its own name.
    ws.import_into_document(&path, false);
    assert!(symbol_names(&ws).contains(&"loop 2".to_owned()));
}

#[test]
fn a_file_that_is_not_a_movie_says_so_and_changes_nothing() {
    let mut ws = workspace();
    let before = ws.document().clone();
    ws.import_into_document_bytes("junk.swf", b"not a movie".to_vec(), true);
    assert!(
        ws.status().is_some_and(|s| s.contains("Cannot import")),
        "{:?}",
        ws.status()
    );
    assert_eq!(ws.document(), &before);
    let dir = tempfile::tempdir().expect("dir");
    ws.import_into_document(&dir.path().join("missing.swf"), false);
    assert!(ws.status().is_some_and(|s| s.contains("Cannot import")));
    assert_eq!(ws.document(), &before);
}

#[test]
fn the_import_items_are_named_as_in_animate() {
    let ws = workspace();
    assert_eq!(ws.label(MenuAction::ImportToStage), "Import to Stage...");
    assert_eq!(
        ws.label(MenuAction::ImportToLibrary),
        "Import to Library..."
    );
}
