//! The Brush's stabilizer and tapers in the workspace: the samples a drag gives are steadied as
//! they come in, the stroke catches up with the pointer when it is let go, the preview takes the
//! library's widths, and the settings are kept with the preferences.

use super::*;
use flare_input::{InputSample, PointerKind};
use flare_tools::brush::{BrushStroke, BuiltinBrushProfile, Taper};

fn mouse(x: f64, y: f64) -> InputSample {
    InputSample::new(x, y, 1.0, PointerKind::Mouse).expect("sample")
}

/// Drags the tool in hand along `points` the way the stage does, steadying each sample.
fn drag(ws: &mut Workspace, points: impl IntoIterator<Item = (f64, f64)>) {
    for (x, y) in points {
        let steady = ws.steady_freehand(mouse(x, y), 1.0);
        ws.push_freehand(steady, 1.0);
    }
}

fn last_bounds(ws: &Workspace) -> (f64, f64, f64, f64) {
    let layer = ws.document().scenes()[0]
        .timeline()
        .layers()
        .last()
        .expect("layer");
    let bounds = flare_tools::ops::span_bounds(
        ws.document(),
        ws.scene_id().expect("scene"),
        SpanRef {
            layer: layer.id(),
            start: layer.spans()[0].start(),
            element: 0,
        },
    )
    .expect("bounds");
    (
        f64::from(bounds.min.x),
        f64::from(bounds.max.x),
        f64::from(bounds.min.y),
        f64::from(bounds.max.y),
    )
}

#[test]
fn a_steadied_stroke_trails_while_drawn_and_ends_where_the_pointer_was_let_go() {
    let mut ws = workspace();
    ws.set_tool(Tool::Brush);
    ws.brush_size = 10.0;
    ws.brush_shelf.choose_builtin(BuiltinBrushProfile::GPen);
    drag(
        &mut ws,
        (0..=100).map(|i| (f64::from(i) * 2.0 + 20.0, 60.0)),
    );
    let trailing = ws.freehand.last().expect("drawn").x;
    assert!(trailing < 219.0, "the line trails the pointer: {trailing}");
    ws.finish_freehand(1.0);
    assert!(ws.freehand.is_empty() && ws.stabilizer.is_none());
    let (left, right, top, bottom) = last_bounds(&ws);
    // The G-pen tapers to nothing at both ends, so no round end reaches past the line.
    assert!((left - 20.0).abs() < 0.5, "{left} {right} {top} {bottom}");
    assert!(
        (right - 220.0).abs() < 0.5,
        "caught up with the pointer: {right}"
    );
    assert!(
        (bottom - top - 10.0).abs() < 0.5,
        "full width in the middle"
    );
}

#[test]
fn a_shaky_hand_is_steadied_only_by_a_brush_that_asks_for_it() {
    let shake = |i: i32| {
        (
            f64::from(i) * 2.0,
            50.0 + if i % 2 == 0 { 4.0 } else { -4.0 },
        )
    };
    let spread = |ws: &Workspace| {
        let ys: Vec<f64> = ws.freehand.iter().skip(10).map(|s| s.y).collect();
        ys.iter().copied().fold(f64::MIN, f64::max) - ys.iter().copied().fold(f64::MAX, f64::min)
    };
    let mut steady = workspace();
    steady.set_tool(Tool::Brush);
    steady.brush_shelf.set_stabilizer(8);
    drag(&mut steady, (0..60).map(shake));
    let mut plain = workspace();
    plain.set_tool(Tool::Brush);
    drag(&mut plain, (0..60).map(shake));
    let mut pencil = workspace();
    pencil.set_tool(Tool::Pencil);
    pencil.brush_shelf.set_stabilizer(8);
    drag(&mut pencil, (0..60).map(shake));
    assert!(spread(&steady) < 2.0, "{}", spread(&steady));
    assert!(spread(&plain) > 7.0 && spread(&pencil) > 7.0);
    // Letting go of a gesture some other way drops what the stabilizer held.
    steady.cancel_drag();
    assert!(steady.stabilizer.is_none() && steady.freehand.is_empty());
}

#[test]
fn the_preview_takes_its_widths_from_the_brush_library() {
    let mut ws = workspace();
    ws.set_tool(Tool::Brush);
    ws.brush_size = 16.0;
    ws.brush_shelf.set_taper(30.0, 30.0);
    let samples: Vec<InputSample> = (0..=50).map(|i| mouse(f64::from(i) * 2.0, 0.0)).collect();
    let preview = ws.previews_of(std::slice::from_ref(&samples), 1.0);
    let expected = ws
        .brush_shelf
        .stroke(16.0)
        .half_widths(&BrushStroke::spine(&samples));
    assert_eq!(preview[0].halves.len(), expected.len());
    for (shown, kept) in preview[0].halves.iter().zip(&expected) {
        assert!((f64::from(*shown) - kept).abs() < 1e-4);
    }
    assert!(preview[0].halves[0] < 2.0, "the tapered start is thin");
    assert!((preview[0].halves[25] - 8.0).abs() < 1e-4);
}

#[test]
fn the_stabilizer_and_tapers_are_kept_with_the_preferences() {
    let mut ws = workspace();
    ws.brush_shelf.set_stabilizer(9);
    ws.brush_shelf.set_taper(12.0, 30.0);
    let saved = ws.preferences_json().expect("json");
    let mut fresh = workspace();
    fresh.restore_preferences(&saved).expect("restored");
    assert_eq!(fresh.brush_shelf.stabilizer(), 9);
    assert_eq!(fresh.brush_shelf.taper(), Taper::new(12.0, 30.0));
    // A chosen profile brings its own back.
    ws.brush_shelf
        .choose_builtin(BuiltinBrushProfile::MappingPen);
    let saved = ws.preferences_json().expect("json");
    let mut fresh = workspace();
    fresh.restore_preferences(&saved).expect("restored");
    assert_eq!(
        fresh.brush_shelf.builtin(),
        Some(BuiltinBrushProfile::MappingPen)
    );
    assert_eq!(fresh.brush_shelf.stabilizer(), 10);
    // An edited file cannot put them out of range.
    let wild = saved
        .replace("\"brush_stabilizer\": 10", "\"brush_stabilizer\": 200")
        .replace(
            "\"brush_builtin_profile\": \"MappingPen\"",
            "\"brush_builtin_profile\": null",
        );
    let mut odd = workspace();
    odd.restore_preferences(&wild).expect("restored");
    assert_eq!(
        odd.brush_shelf.stabilizer(),
        flare_input::MAX_STABILIZER_LEVEL
    );
}
