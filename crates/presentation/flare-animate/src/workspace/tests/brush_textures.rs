//! Textured brushes in the workspace: a textured profile commits one shape made of its strands,
//! the preview shows those strands while dragging, and the texture is kept with the preferences.

use super::*;
use flare_input::{InputSample, PointerKind};
use flare_tools::brush::BuiltinBrushProfile;

fn draw(ws: &mut Workspace) {
    for i in 0..=80 {
        let x = f64::from(i) * 3.0 + 20.0;
        let sample = InputSample::new(x, 60.0 + (x * 0.05).sin() * 20.0, 0.8, PointerKind::Pen)
            .expect("sample");
        let steady = ws.steady_freehand(sample, 1.0);
        ws.push_freehand(steady, 1.0);
    }
}

fn subpaths(shape: &Shape) -> usize {
    shape
        .path()
        .iter()
        .filter(|s| matches!(s, flare_document::PathSegment::MoveTo(_)))
        .count()
}

#[test]
fn a_charcoal_stroke_is_one_shape_of_many_strands_and_the_preview_shows_them() {
    let mut ws = workspace();
    ws.set_tool(Tool::Brush);
    ws.brush_size = 30.0;
    ws.pick_brush(crate::workspace::panels::BrushPick::Builtin(
        BuiltinBrushProfile::Charcoal,
    ));
    draw(&mut ws);
    let preview = ws.freehand_preview(1.0);
    assert!(preview.len() > 10, "strands: {}", preview.len());
    assert!(preview.iter().all(|p| p.sides.is_some()));
    ws.finish_freehand(1.0);
    assert_eq!(drawing_count(&ws), 1, "one stroke, one shape");
    let shape = drawn_shape(&ws, 1);
    // One closed piece of outline per strand run the preview showed.
    assert_eq!(subpaths(&shape), preview.len());
}

#[test]
fn a_plain_brush_stays_one_solid_outline() {
    let mut ws = workspace();
    ws.set_tool(Tool::Brush);
    draw(&mut ws);
    assert_eq!(ws.freehand_preview(1.0).len(), 1);
    ws.finish_freehand(1.0);
    assert_eq!(subpaths(&drawn_shape(&ws, 1)), 1);
}

#[test]
fn the_texture_in_hand_is_kept_with_the_preferences_and_edits_a_chosen_custom_brush() {
    let mut ws = workspace();
    ws.brush_shelf.set_texture(9, 0.4);
    assert_eq!(
        ws.brush_shelf.builtin(),
        None,
        "an edit leaves the built-in list"
    );
    let mut fresh = workspace();
    fresh.apply_preferences(&ws.preferences());
    assert_eq!(fresh.brush_shelf.texture(), ws.brush_shelf.texture());

    ws.brush_shelf.keep_nib().expect("room");
    assert_eq!(ws.brush_shelf.presets()[0].bristles, 9);
    ws.brush_shelf.set_texture(3, 0.1);
    assert_eq!(ws.brush_shelf.presets()[0].bristles, 3);
}
