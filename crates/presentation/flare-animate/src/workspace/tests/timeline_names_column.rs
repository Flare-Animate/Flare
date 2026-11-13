//! The timeline's column of layer names: its edge drags to make it wider or narrower, and the
//! layer buttons sit over it at the left of the toolbar.

use super::layer_names::{press, settle};
use super::*;
use crate::workspace::timeline_names_edge::{MAX_NAMES, MIN_NAMES, names_width};
use egui::PointerButton;

fn drawn() -> (egui::Context, Workspace, f64) {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    flare_ui::icons::install(&ctx);
    let mut time = 0.0;
    settle(&mut ws, &ctx, &mut time);
    (ctx, ws, time)
}

/// Presses on the names column's edge on the first row and drags it `dx` points sideways.
fn drag_edge(ws: &mut Workspace, ctx: &egui::Context, time: &mut f64, dx: f32) {
    let l = ws.timeline_layout.expect("drawn");
    let start = Pos2::new(l.label_edge, l.row_top(0) + 10.0);
    let mut step = |ws: &mut Workspace, events: Vec<Event>| {
        *time += 0.05;
        pass(ws, ctx, input(*time, events));
    };
    step(ws, vec![Event::PointerMoved(start)]);
    step(ws, vec![press(start, PointerButton::Primary, true)]);
    for i in 1..=4 {
        let at = start + Vec2::new(dx * i as f32 / 4.0, 0.0);
        step(ws, vec![Event::PointerMoved(at)]);
    }
    let end = start + Vec2::new(dx, 0.0);
    step(ws, vec![press(end, PointerButton::Primary, false)]);
    step(ws, vec![]);
}

/// Where the first text drawn that starts with `wanted` is, left edge and middle height.
fn text_at(ws: &mut Workspace, ctx: &egui::Context, time: f64, wanted: &str) -> Option<Pos2> {
    let mut out = ctx.run_ui(input(time, vec![]), |ui| ws.show(ui));
    out.textures_delta.clear();
    out.shapes.iter().find_map(|s| match &s.shape {
        egui::epaint::Shape::Text(t) if t.galley.text().starts_with(wanted) => {
            Some(t.visual_bounding_rect().left_center())
        }
        _ => None,
    })
}

#[test]
fn the_width_is_kept_within_its_bounds_and_nonsense_is_the_default() {
    assert_eq!(names_width(200.0), 200.0);
    assert_eq!(names_width(10.0), MIN_NAMES);
    assert_eq!(names_width(5000.0), MAX_NAMES);
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(names_width(bad), timeline::LABEL_W, "{bad}");
    }
}

#[test]
fn dragging_the_edge_widens_and_narrows_the_names_and_the_frames_follow() {
    let (ctx, mut ws, mut time) = drawn();
    let before = ws.display.timeline_names;
    let track = ws.timeline_layout.expect("drawn").track_left;
    drag_edge(&mut ws, &ctx, &mut time, 60.0);
    assert_eq!(ws.display.timeline_names, before + 60.0);
    let l = ws.timeline_layout.expect("drawn");
    assert_eq!(l.track_left, track + 60.0, "the frames start further right");
    assert_eq!(l.label_edge - l.labels_left, before + 60.0);

    drag_edge(&mut ws, &ctx, &mut time, -1000.0);
    assert_eq!(
        ws.display.timeline_names, MIN_NAMES,
        "no narrower than its least"
    );
    assert!(!ws.is_dirty(), "the view is not the document");
}

#[test]
fn the_width_is_kept_with_the_preferences() {
    let mut ws = workspace();
    ws.display.timeline_names = 310.0;
    let kept = ws.preferences();
    assert_eq!(kept.timeline_names_width, 310.0);
    let mut fresh = workspace();
    fresh.apply_preferences(&kept);
    assert_eq!(fresh.display.timeline_names, 310.0);
    let mut odd = kept.clone();
    odd.timeline_names_width = f32::NAN;
    fresh.apply_preferences(&odd);
    assert_eq!(fresh.display.timeline_names, timeline::LABEL_W);
}

#[test]
fn the_layer_buttons_sit_over_the_names_and_playback_starts_over_the_frames() {
    let (ctx, mut ws, mut time) = drawn();
    time += 0.1;
    let folder = text_at(&mut ws, &ctx, time, "Folder").expect("the Folder menu");
    let frame = text_at(&mut ws, &ctx, time, "Frame ").expect("the frame field");
    let l = ws.timeline_layout.expect("drawn");
    assert!(
        folder.x < l.label_edge,
        "the layer buttons are over the names"
    );
    assert!(frame.x > l.track_left, "the transport is over the frames");

    // A narrow column pushes the rest along rather than under the layer buttons.
    ws.display.timeline_names = MIN_NAMES;
    settle(&mut ws, &ctx, &mut time);
    let folder = text_at(&mut ws, &ctx, time, "Folder").expect("the Folder menu");
    let frame = text_at(&mut ws, &ctx, time, "Frame ").expect("the frame field");
    assert!(frame.x > folder.x + 40.0);
}
