use super::*;
use flare_input::{InputSample, PointerKind};
use flare_tools::brush::{Nib, NibShape};

fn paint(ws: &mut Workspace, from: (f64, f64), to: (f64, f64)) {
    ws.set_tool(Tool::Brush);
    ws.freehand = (0..=20)
        .map(|i| {
            let t = f64::from(i) / 20.0;
            InputSample::new(
                from.0 + (to.0 - from.0) * t,
                from.1 + (to.1 - from.1) * t,
                1.0,
                PointerKind::Mouse,
            )
            .expect("sample")
        })
        .collect();
    ws.finish_freehand(1.0);
}

fn size_of(ws: &Workspace) -> (f64, f64) {
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
        },
    )
    .expect("bounds");
    (f64::from(bounds.width()), f64::from(bounds.height()))
}

#[test]
fn a_flat_nib_paints_a_thinner_line_than_a_round_one_along_its_long_axis() {
    let mut round = workspace();
    round.brush_size = 20.0;
    paint(&mut round, (10.0, 50.0), (110.0, 50.0));
    let mut flat = workspace();
    flat.brush_size = 20.0;
    flat.brush_nib = Nib::new(NibShape::Round, 0.0, 10.0);
    paint(&mut flat, (10.0, 50.0), (110.0, 50.0));
    assert!(size_of(&round).1 > 19.0, "{:?}", size_of(&round));
    assert!(size_of(&flat).1 < 4.0, "{:?}", size_of(&flat));
}

#[test]
fn the_nib_and_size_are_kept_with_the_preferences_and_junk_is_brought_into_range() {
    let mut ws = workspace();
    ws.brush_size = 33.0;
    ws.brush_nib = Nib::new(NibShape::Square, 45.0, 30.0);
    let saved = ws.preferences_json().expect("json");
    let mut fresh = workspace();
    fresh.restore_preferences(&saved).expect("restored");
    assert_eq!(fresh.brush_size, 33.0);
    assert_eq!(fresh.brush_nib.shape(), NibShape::Square);
    assert!((fresh.brush_nib.angle_degrees() - 45.0).abs() < 1e-4);
    assert!((fresh.brush_nib.flatness_percent() - 30.0).abs() < 1e-4);
    let mut odd = workspace();
    odd.restore_preferences(
        r#"{"brush_size": 9000, "brush_nib_angle": -90, "brush_nib_flatness": 0}"#,
    )
    .expect("loads");
    assert_eq!(odd.brush_size, 200.0);
    assert!((odd.brush_nib.angle_degrees() - 270.0).abs() < 1e-4);
    assert!((odd.brush_nib.flatness_percent() - 1.0).abs() < 1e-4);
    // A file from before the nib has none of it: the plain round brush.
    let mut old = workspace();
    old.restore_preferences(r#"{"smoothing": 3}"#).expect("old");
    assert_eq!(old.brush_nib, Nib::ROUND);
}

#[test]
fn the_preview_follows_the_nib_and_the_properties_panel_draws_its_controls() {
    let mut ws = workspace();
    ws.brush_size = 20.0;
    ws.brush_nib = Nib::new(NibShape::Round, 0.0, 10.0);
    ws.set_tool(Tool::Brush);
    let along: Vec<InputSample> = (0..5)
        .map(|i| InputSample::new(f64::from(i) * 10.0, 5.0, 1.0, PointerKind::Mouse).expect("s"))
        .collect();
    let across: Vec<InputSample> = (0..5)
        .map(|i| InputSample::new(5.0, f64::from(i) * 10.0, 1.0, PointerKind::Mouse).expect("s"))
        .collect();
    let previews = ws.previews_of(&[along, across]);
    assert!(previews[0].halves.iter().all(|h| *h < 2.0));
    assert!(previews[1].halves.iter().all(|h| *h > 9.0));
    let ctx = egui::Context::default();
    run(&mut ws, &ctx, 0.0);
}
