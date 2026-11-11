#![allow(clippy::expect_used)]

use super::*;

#[test]
fn only_the_frames_in_view_are_visited() {
    // Frame 0 starts at x = 100 and each frame is 10 wide; the view is x 250..=330.
    assert_eq!(visible_frames(250.0, 330.0, 100.0, 10.0, 1000), 15..23);
}

#[test]
fn a_view_left_of_the_track_starts_at_the_first_frame() {
    assert_eq!(visible_frames(0.0, 130.0, 100.0, 10.0, 1000), 0..3);
}

#[test]
fn frames_past_the_end_are_not_visited() {
    assert_eq!(visible_frames(0.0, 10_000.0, 0.0, 10.0, 24), 0..24);
    assert_eq!(visible_frames(5_000.0, 6_000.0, 0.0, 10.0, 24), 24..24);
}

#[test]
fn nonsense_widths_visit_nothing() {
    for width in [0.0, -3.0, f32::NAN, f32::INFINITY] {
        assert_eq!(visible_frames(0.0, 100.0, 0.0, width, 24), 0..0, "{width}");
    }
    assert_eq!(visible_frames(f32::NAN, 100.0, 0.0, 10.0, 24), 0..0);
}

fn laid_out(name: &str, width: f32) -> std::sync::Arc<egui::Galley> {
    // Laid out inside a frame, so the font atlas it grows is handed out with that frame.
    let ctx = egui::Context::default();
    let mut galley = None;
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        galley = Some(ui.fonts_mut(|f| layer_name(f, name.to_owned(), width, Color32::WHITE)));
    });
    output.textures_delta.clear();
    galley.expect("the frame ran")
}

/// What the galley shows, which after a cut is not the text it was given.
fn shown(galley: &egui::Galley) -> String {
    galley.rows.iter().map(|r| r.text()).collect()
}

#[test]
fn a_long_layer_name_is_cut_with_an_ellipsis_inside_its_column() {
    let galley = laid_out("Sound: A very long file name that runs on and on.mp3", 80.0);
    assert!(galley.size().x <= 80.0, "{} wider than 80", galley.size().x);
    assert!(galley.elided);
    assert!(shown(&galley).ends_with('…'), "{:?}", shown(&galley));
    assert_eq!(galley.rows.len(), 1);
}

#[test]
fn a_short_layer_name_is_left_whole() {
    let galley = laid_out("Layer 1", 80.0);
    assert!(!galley.elided);
    assert_eq!(shown(&galley), "Layer 1");
}

#[test]
fn no_room_shows_no_more_than_an_ellipsis() {
    let galley = laid_out("Layer 1", -5.0);
    assert!(galley.size().x <= 12.0, "{} wide", galley.size().x);
    assert!(shown(&galley).chars().count() <= 1, "{:?}", shown(&galley));
}
