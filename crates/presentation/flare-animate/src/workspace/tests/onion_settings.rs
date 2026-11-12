//! Onion skin's range, tint and colors: set in its popup, by dragging its markers on the ruler,
//! and kept with the preferences.

use super::layer_names::{press, settle};
use super::*;
use crate::workspace::types::{ONION_AFTER_COLOR, ONION_BEFORE_COLOR};
use egui::PointerButton;
use flare_core::Rgba8;

#[test]
fn onion_skin_starts_tinted_blue_before_and_green_after_one_frame_each_side() {
    let ws = workspace();
    let onion = ws.onion_skin();
    assert!(!onion.enabled, "off until asked for");
    assert_eq!((onion.before, onion.after), (1, 1));
    assert!(onion.tinted);
    assert_eq!(onion.tints(), Some([ONION_BEFORE_COLOR, ONION_AFTER_COLOR]));
    assert_eq!(MAX_ONION_FRAMES, 12);
}

#[test]
fn dragging_the_after_marker_on_the_ruler_widens_the_range_and_leaves_the_playhead() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    flare_ui::icons::install(&ctx);
    rect(&mut ws); // 24 frames
    ws.set_frame(5);
    ws.set_onion_skin(true);
    let mut time = 0.0;
    settle(&mut ws, &ctx, &mut time);
    let l = ws.timeline_layout.expect("the timeline was drawn");
    let (_, end) = l.onion_markers(5, 1, 1);
    let y = l.ruler_top + 15.0;
    let (from, to) = (Pos2::new(end, y), Pos2::new(l.frame_x(10.0), y));
    let mut step = |ws: &mut Workspace, events: Vec<Event>| {
        time += 0.05;
        pass(ws, &ctx, input(time, events));
    };
    step(&mut ws, vec![Event::PointerMoved(from)]);
    step(&mut ws, vec![press(from, PointerButton::Primary, true)]);
    for x in [end + 10.0, end + 25.0, to.x] {
        step(&mut ws, vec![Event::PointerMoved(Pos2::new(x, y))]);
    }
    step(&mut ws, vec![press(to, PointerButton::Primary, false)]);
    assert_eq!(
        (ws.onion_skin().before, ws.onion_skin().after),
        (1, 4),
        "out to frame 9"
    );
    assert_eq!(ws.frame(), 5, "the marker moved, not the playhead");
}

/// One pass drawing only the onion settings, as the popup does.
fn settings_pass(
    ws: &mut Workspace,
    ctx: &egui::Context,
    time: f64,
    events: Vec<Event>,
) -> Vec<(String, Pos2)> {
    let mut out = ctx.run_ui(input(time, events), |ui| ws.onion_settings(ui));
    out.textures_delta.clear();
    out.shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::epaint::Shape::Text(t) => Some((
                t.galley.text().to_owned(),
                t.visual_bounding_rect().center(),
            )),
            _ => None,
        })
        .collect()
}

#[test]
fn the_settings_turn_the_tint_off_and_reset_the_colors() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    ws.set_onion_colors(Rgba8::opaque(200, 0, 0), Rgba8::opaque(0, 0, 0));
    let mut time = 0.0;
    let mut click = |ws: &mut Workspace, label: &str| {
        time += 0.1;
        let at = settings_pass(ws, &ctx, time, vec![])
            .into_iter()
            .find(|(text, _)| text == label)
            .map(|(_, p)| p)
            .unwrap_or_else(|| panic!("{label} is not drawn"));
        for events in [
            vec![Event::PointerMoved(at)],
            vec![press(at, PointerButton::Primary, true)],
            vec![press(at, PointerButton::Primary, false)],
        ] {
            time += 0.05;
            settings_pass(ws, &ctx, time, events);
        }
    };
    click(&mut ws, "Reset colors");
    assert_eq!(
        ws.onion_skin().tints(),
        Some([ONION_BEFORE_COLOR, ONION_AFTER_COLOR])
    );
    click(&mut ws, "Tinted");
    assert!(!ws.onion_skin().tinted);
    assert_eq!(ws.onion_skin().tints(), None, "ghosts in their own colors");
}

#[test]
fn the_range_the_tint_and_the_colors_are_kept_with_the_preferences() {
    let mut ws = workspace();
    ws.set_onion_range(5, 9);
    ws.set_onion_outlines(true);
    ws.set_onion_tinted(false);
    ws.set_onion_colors(Rgba8::opaque(200, 10, 10), Rgba8::opaque(10, 10, 200));
    let saved = ws.preferences_json().expect("json");
    let mut next = workspace();
    next.restore_preferences(&saved).expect("restored");
    let onion = next.onion_skin();
    assert_eq!(
        (onion.before, onion.after, onion.outlines, onion.tinted),
        (5, 9, true, false)
    );
    assert_eq!(onion.before_color, Rgba8::opaque(200, 10, 10));
    assert_eq!(onion.after_color, Rgba8::opaque(10, 10, 200));
    assert!(!onion.enabled, "a session starts with it off");
    // A file asking for more than the limit gets the limit.
    let mut edited: serde_json::Value = serde_json::from_str(&saved).expect("json");
    edited["onion_after"] = serde_json::json!(500);
    next.restore_preferences(&edited.to_string())
        .expect("restored");
    assert_eq!(next.onion_skin().after, MAX_ONION_FRAMES);
}

#[test]
fn a_new_color_draws_the_stage_again() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    rect(&mut ws);
    ws.set_frame(3);
    ws.set_onion_skin(true);
    run(&mut ws, &ctx, 0.0);
    run(&mut ws, &ctx, 0.1);
    assert_eq!(
        ws.texture.as_ref().map(|t| t.onion.before_color),
        Some(ONION_BEFORE_COLOR)
    );
    let red = Rgba8::opaque(220, 30, 30);
    ws.set_onion_colors(red, ONION_AFTER_COLOR);
    run(&mut ws, &ctx, 0.2);
    run(&mut ws, &ctx, 0.3);
    assert_eq!(
        ws.texture.as_ref().map(|t| t.onion.before_color),
        Some(red),
        "the stage on show has the new color"
    );
}
