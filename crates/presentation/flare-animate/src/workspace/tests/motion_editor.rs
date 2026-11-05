//! The Motion Editor panel: dragging a handle becomes one validated command, undo restores it,
//! and the wiring (selection, copy and paste, reset) goes through the same command.

use super::tween_inspector::tweened_instance;
use super::*;
use flare_document::SpanInterp;
use flare_tween::{HandleKind, Interpolation};

const SIZE: f64 = 220.0;

fn free_curve(ws: &mut Workspace) -> SpanInterp {
    let curve = SpanInterp::new(
        Interpolation::Bezier,
        flare_tween::Easing::Auto,
        HandleKind::Free,
        [0.3, 0.0, -0.3, 0.0],
        flare_tween::Params::default(),
    )
    .expect("curve");
    ws.set_selection_interp(Some(curve));
    ws.motion_layout(SIZE, SIZE);
    curve
}

fn pixel_of(ws: &Workspace, world: (f64, f64)) -> (f64, f64) {
    ws.motion().view().to_screen(world)
}

#[test]
fn nothing_is_shown_or_grabbed_without_a_tweened_selection() {
    let mut ws = workspace();
    ws.motion_layout(SIZE, SIZE);
    assert!(ws.motion_shown().is_none());
    assert!(!ws.motion_press((100.0, 100.0)));
    ws.motion_drag((10.0, 10.0), false);
    ws.motion_release();
    ws.motion_copy();
    ws.motion_paste();
}

#[test]
fn a_tween_without_a_curve_gets_one_from_the_panel_and_undo_takes_it_away() {
    let mut ws = tweened_instance(0);
    assert!(ws.selection_interp().is_none());
    ws.motion_set_family("bezier");
    assert_eq!(
        ws.selection_interp().map(|i| i.interpolation()),
        Some(Interpolation::Bezier)
    );
    ws.undo();
    assert!(ws.selection_interp().is_none());
}

#[test]
fn an_unknown_family_changes_nothing_and_says_so() {
    let mut ws = tweened_instance(0);
    ws.motion_set_family("wobbly");
    assert!(ws.selection_interp().is_none());
}

#[test]
fn dragging_a_handle_changes_the_document_once_on_release_and_undo_restores_it() {
    let mut ws = tweened_instance(0);
    let before = free_curve(&mut ws);
    let from = pixel_of(&ws, (0.3, 0.0));
    assert!(ws.motion_press(from));
    ws.motion_drag(pixel_of(&ws, (0.5, 0.2)), false);
    // The panel shows the working copy while the document keeps the old curve.
    let working = ws.motion_shown().expect("shown");
    assert_ne!(working, before);
    assert_eq!(ws.selection_interp(), Some(before));
    ws.motion_drag(pixel_of(&ws, (0.6, 0.4)), false);
    ws.motion_release();
    let after = ws.selection_interp().expect("curve");
    assert!((after.handles()[0] - 0.6).abs() < 1e-6 && (after.handles()[1] - 0.4).abs() < 1e-6);
    ws.undo();
    assert_eq!(ws.selection_interp(), Some(before));
    ws.redo();
    assert_eq!(ws.selection_interp(), Some(after));
}

#[test]
fn dragging_an_automatic_handle_turns_it_aligned() {
    let mut ws = tweened_instance(0);
    ws.motion_set_family("bezier");
    ws.motion_layout(SIZE, SIZE);
    let handles = flare_tween::effective_handles(
        ws.selection_interp().expect("curve").handle_kind(),
        ws.selection_interp().expect("curve").handles(),
    )
    .expect("handles");
    assert!(ws.motion_press(pixel_of(&ws, (handles[0], handles[1]))));
    ws.motion_drag(pixel_of(&ws, (0.4, 0.6)), false);
    ws.motion_release();
    assert_eq!(
        ws.selection_interp().expect("curve").handle_kind(),
        HandleKind::Aligned
    );
}

#[test]
fn a_press_away_from_the_handles_and_a_cancelled_drag_change_nothing() {
    let mut ws = tweened_instance(0);
    let before = free_curve(&mut ws);
    assert!(!ws.motion_press(pixel_of(&ws, (0.5, 0.9))));
    ws.motion_release();
    assert_eq!(ws.selection_interp(), Some(before));
    assert!(ws.motion_press(pixel_of(&ws, (0.3, 0.0))));
    ws.motion_drag(pixel_of(&ws, (0.9, 0.9)), false);
    ws.motion_cancel();
    ws.motion_release();
    assert_eq!(ws.selection_interp(), Some(before));
    assert_eq!(ws.motion_shown(), Some(before));
}

#[test]
fn snapping_puts_the_handle_on_the_grid() {
    let mut ws = tweened_instance(0);
    free_curve(&mut ws);
    assert!(ws.motion_press(pixel_of(&ws, (0.3, 0.0))));
    ws.motion_drag(pixel_of(&ws, (0.52, 0.37)), true);
    ws.motion_release();
    let h = ws.selection_interp().expect("curve").handles();
    assert!(
        (h[0] - 0.5).abs() < 1e-9 && (h[1] - 0.4).abs() < 1e-9,
        "{h:?}"
    );
}

#[test]
fn a_family_without_handles_cannot_be_grabbed() {
    let mut ws = tweened_instance(0);
    ws.motion_set_family("quad");
    ws.motion_layout(SIZE, SIZE);
    assert!(!ws.motion_press(pixel_of(&ws, (0.33, 0.0))));
}

#[test]
fn changing_the_handle_kind_keeps_the_curve_where_it_is_drawn() {
    let mut ws = tweened_instance(0);
    free_curve(&mut ws);
    ws.motion_set_handle_kind(HandleKind::Aligned);
    let now = ws.selection_interp().expect("curve");
    assert_eq!(now.handle_kind(), HandleKind::Aligned);
    assert_eq!(&now.handles()[..2], &[0.3, 0.0]);
    ws.undo();
    assert_eq!(
        ws.selection_interp().expect("curve").handle_kind(),
        HandleKind::Free
    );
}

#[test]
fn easing_family_copy_paste_and_reset_each_are_one_undo_step() {
    let mut ws = tweened_instance(0);
    let curve = free_curve(&mut ws);
    ws.motion_copy();
    ws.motion_reset();
    assert_eq!(ws.selection_interp(), Some(SpanInterp::default()));
    ws.motion_paste();
    assert_eq!(ws.selection_interp(), Some(curve));
    ws.motion_set_easing("in");
    assert_eq!(ws.selection_interp().expect("curve").easing().name(), "in");
    ws.undo();
    assert_eq!(ws.selection_interp(), Some(curve));
    ws.undo();
    assert_eq!(ws.selection_interp(), Some(SpanInterp::default()));
}

#[test]
fn pasting_with_nothing_copied_does_nothing() {
    let mut ws = tweened_instance(0);
    ws.motion_paste();
    assert!(ws.selection_interp().is_none());
}

#[test]
fn the_panel_draws_the_curve_and_its_controls_and_nothing_when_idle_samples_again() {
    let mut ws = tweened_instance(0);
    free_curve(&mut ws);
    let ctx = egui::Context::default();
    flare_ui::icons::install(&ctx);
    let mut text = Vec::new();
    for step in 0..3 {
        let mut out = ctx.run_ui(input(f64::from(step) * 0.1, vec![]), |ui| {
            ws.motion_editor_panel(ui)
        });
        out.textures_delta.clear();
        text = out
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::epaint::Shape::Text(t) => Some(t.galley.text().to_owned()),
                _ => None,
            })
            .collect();
    }
    for wanted in ["Copy curve", "Paste curve", "Reset"] {
        assert!(text.iter().any(|t| t == wanted), "{wanted}: {text:?}");
    }
    let before = ws.motion().resamples();
    assert!(before >= 1);
    let mut out = ctx.run_ui(input(0.5, vec![]), |ui| ws.motion_editor_panel(ui));
    out.textures_delta.clear();
    assert_eq!(before, ws.motion().resamples());
}

#[test]
fn the_panel_explains_what_to_select_when_nothing_is_tweened() {
    let mut ws = workspace();
    let ctx = egui::Context::default();
    flare_ui::icons::install(&ctx);
    let mut out = ctx.run_ui(input(0.0, vec![]), |ui| ws.motion_editor_panel(ui));
    out.textures_delta.clear();
    let text: Vec<String> = out
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::epaint::Shape::Text(t) => Some(t.galley.text().to_owned()),
            _ => None,
        })
        .collect();
    assert!(
        text.iter().any(|t| t.contains("tweened keyframe")),
        "{text:?}"
    );
}
