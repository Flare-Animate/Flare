use super::*;

#[test]
fn the_default_is_larger_than_the_design_size_and_offered() {
    const { assert!(DEFAULT > 1.0) };
    assert!(SIZES.contains(&DEFAULT));
    assert!(SIZES.contains(&1.0));
    assert!(SIZES.iter().all(|s| (MIN..=MAX).contains(s)));
}

#[test]
fn a_size_out_of_range_or_not_a_number_is_brought_back() {
    assert_eq!(clamp(9.0), MAX);
    assert_eq!(clamp(0.1), MIN);
    assert_eq!(clamp(f32::NAN), DEFAULT);
    assert_eq!(clamp(f32::INFINITY), DEFAULT);
    assert_eq!(clamp(1.25), 1.25);
}

#[test]
fn a_large_window_gets_the_size_chosen() {
    assert_eq!(effective(1.5, [1920.0, 1080.0]), 1.5);
    assert_eq!(effective(1.15, [1280.0, 800.0]), 1.15);
}

#[test]
fn a_small_window_keeps_room_for_the_whole_layout() {
    // 1000 across fits 960 at 1.04 and no more.
    let zoom = effective(1.5, [1000.0, 900.0]);
    assert!((zoom - 1000.0 / 960.0).abs() < 1e-4, "{zoom}");
    assert!(1000.0 / zoom >= LAYOUT_MIN[0] - 1e-3);
    // Height binds too.
    let zoom = effective(2.0, [3000.0, 660.0]);
    assert!((zoom - 1.1).abs() < 1e-4, "{zoom}");
}

#[test]
fn a_tiny_window_is_not_shrunk_below_the_design_size_and_a_small_choice_is_kept() {
    assert_eq!(effective(1.5, [400.0, 700.0]), 1.0);
    assert_eq!(effective(0.9, [400.0, 700.0]), 0.9);
    assert_eq!(effective(0.9, [3000.0, 2000.0]), 0.9);
    assert_eq!(effective(1.5, [f32::NAN, 0.0]), 1.0);
}

#[test]
fn apply_sets_the_zoom_from_the_window_and_settles() {
    let ctx = egui::Context::default();
    // One pass in a window `size` points across at the zoom in force, as an integration gives it.
    let pass = |size: egui::Vec2, chosen: f32| {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            ..egui::RawInput::default()
        };
        let mut output = ctx.run_ui(input, |ui| apply(ui.ctx(), chosen));
        output.textures_delta.clear();
    };
    pass(egui::vec2(1600.0, 1000.0), 1.25);
    pass(egui::vec2(1600.0, 1000.0) / 1.25, 1.25);
    assert!((ctx.zoom_factor() - 1.25).abs() < 1e-4);
    // A window too small for that keeps the layout's room: 1100 native across is 1100 / 960.
    pass(egui::vec2(1100.0, 1000.0) / 1.25, 1.25);
    pass(egui::vec2(1100.0, 1000.0) / (1100.0 / 960.0), 1.25);
    assert!((ctx.zoom_factor() - 1100.0 / 960.0).abs() < 1e-3);
    // Back to the design size; a new zoom takes hold at the next pass.
    pass(egui::vec2(1280.0, 800.0), 1.0);
    pass(egui::vec2(1280.0, 800.0), 1.0);
    assert!((ctx.zoom_factor() - 1.0).abs() < 1e-4);
}

#[test]
fn sizes_are_named_as_percentages() {
    assert_eq!(label(1.15), "115%");
    assert_eq!(label(1.0), "100%");
    assert_eq!(label(f32::NAN), "115%");
}
