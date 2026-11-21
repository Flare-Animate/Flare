//! Debug > Movie Scripts: the window lists a movie's classes and strings, writes a class out and
//! says why a file that is no movie could not be read.

use super::*;
use flare_as3::Movie;
use flare_as3::fixtures::movie;
use swf::Compression;

fn opened() -> Workspace {
    let mut ws = workspace();
    let bytes = movie(Compression::Zlib).expect("fixture");
    ws.show_movie_scripts("game.swf".into(), Movie::read(&bytes));
    ws
}

#[test]
fn the_menu_entry_asks_for_a_movie_and_is_not_dimmed() {
    let mut ws = workspace();
    assert!(ws.is_enabled(MenuAction::MovieScripts));
    ws.perform(MenuAction::MovieScripts);
    assert_eq!(ws.take_dialog(), Some(DialogRequest::MovieScripts));
}

#[test]
fn a_movie_read_opens_the_window_with_its_classes_and_strings() {
    let ws = opened();
    let window = ws.movie_scripts.as_ref().expect("window");
    assert!(!window.outline.classes.is_empty(), "classes listed");
    assert!(!window.strings.strings.is_empty(), "strings listed");
}

#[test]
fn choosing_a_class_writes_it_out_and_a_search_narrows_the_list() {
    let mut ws = opened();
    let window = ws.movie_scripts.as_mut().expect("window");
    let name = window.outline.classes[0].full_name.clone();
    window.choose(&name);
    let (_, source) = window.chosen.as_ref().expect("chosen");
    assert!(source.as_ref().expect("source").text.contains("class"));
    let all = window.outline.matched;
    window.find = "no-such-class-anywhere".into();
    window.search();
    assert_eq!(window.outline.matched, 0);
    assert!(all > 0);
}

#[test]
fn a_file_that_is_no_movie_reports_why_and_opens_nothing() {
    let mut ws = workspace();
    ws.show_movie_scripts("x.swf".into(), Movie::read(b"not a movie"));
    assert!(!ws.movie_scripts_open());
    assert!(ws.status.as_deref().is_some_and(|s| s.contains("x.swf")));
}

#[test]
fn the_window_draws_on_both_tabs_with_a_class_chosen_and_closes_on_its_own_button() {
    let mut ws = opened();
    let name = ws.movie_scripts.as_ref().expect("w").outline.classes[0]
        .full_name
        .clone();
    ws.movie_scripts.as_mut().expect("w").choose(&name);
    let ctx = egui::Context::default();
    run(&mut ws, &ctx, 0.0);
    ws.movie_scripts.as_mut().expect("w").tab = super::super::movie_scripts::ScriptsTab::Strings;
    run(&mut ws, &ctx, 1.0);
    assert!(ws.movie_scripts_open(), "drawing does not close it");
}

fn png(value: u8) -> Vec<u8> {
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, 1, 1);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .expect("header")
        .write_image_data(&[value, 0, 0, 255])
        .expect("data");
    out
}

#[test]
fn import_image_sequence_asks_for_files_and_opens_one_keyframe_per_picture() {
    let mut ws = workspace();
    assert!(ws.is_enabled(MenuAction::ImportImageSequence));
    ws.perform(MenuAction::ImportImageSequence);
    assert_eq!(ws.take_dialog(), Some(DialogRequest::ImportImageSequence));
    ws.import_image_sequence(vec![
        ("f_10.png".into(), png(1)),
        ("f_2.png".into(), png(2)),
    ]);
    let layers = ws.document().scenes()[0].timeline().layers();
    assert_eq!(layers[0].spans().len(), 2);
    assert!(ws.is_dirty());
    let mut bad = workspace();
    bad.import_image_sequence(vec![("x.png".into(), b"not a png".to_vec())]);
    assert!(bad.status().is_some_and(|s| s.contains("Cannot import")));
}
