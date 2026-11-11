use super::*;
use flare_input::{InputSample, PointerKind};
use flare_tools::brush::{BrushShelf, BuiltinBrushProfile, MAX_BRUSHES, Nib, NibShape};

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
            element: 0,
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
    flat.brush_shelf = BrushShelf::default().with_loose_nib(Nib::new(NibShape::Round, 0.0, 10.0));
    paint(&mut flat, (10.0, 50.0), (110.0, 50.0));
    assert!(size_of(&round).1 > 19.0, "{:?}", size_of(&round));
    assert!(size_of(&flat).1 < 4.0, "{:?}", size_of(&flat));
}

#[test]
fn the_nib_and_size_are_kept_with_the_preferences_and_junk_is_brought_into_range() {
    let mut ws = workspace();
    ws.brush_size = 33.0;
    ws.brush_shelf = BrushShelf::default().with_loose_nib(Nib::new(NibShape::Square, 45.0, 30.0));
    let saved = ws.preferences_json().expect("json");
    let mut fresh = workspace();
    fresh.restore_preferences(&saved).expect("restored");
    assert_eq!(fresh.brush_size, 33.0);
    assert_eq!(fresh.brush_shelf.nib().shape(), NibShape::Square);
    assert!((fresh.brush_shelf.nib().angle_degrees() - 45.0).abs() < 1e-4);
    assert!((fresh.brush_shelf.nib().flatness_percent() - 30.0).abs() < 1e-4);
    let mut odd = workspace();
    odd.restore_preferences(
        r#"{"brush_size": 9000, "brush_nib_angle": -90, "brush_nib_flatness": 0}"#,
    )
    .expect("loads");
    assert_eq!(odd.brush_size, 200.0);
    assert!((odd.brush_shelf.nib().angle_degrees() - 270.0).abs() < 1e-4);
    assert!((odd.brush_shelf.nib().flatness_percent() - 1.0).abs() < 1e-4);
    // A file from before the nib has none of it: the plain round brush.
    let mut old = workspace();
    old.restore_preferences(r#"{"smoothing": 3}"#).expect("old");
    assert_eq!(old.brush_shelf.nib(), Nib::ROUND);
}

#[test]
fn built_in_brush_profiles_and_pressure_response_round_trip_in_preferences() {
    let mut ws = workspace();
    ws.brush_shelf
        .choose_builtin(BuiltinBrushProfile::GreasePencil);
    let saved = ws.preferences_json().expect("json");
    let mut fresh = workspace();
    fresh.restore_preferences(&saved).expect("restored");
    assert_eq!(
        fresh.brush_shelf.builtin(),
        Some(BuiltinBrushProfile::GreasePencil)
    );
    assert_eq!(
        fresh.brush_shelf.pressure_min(),
        BuiltinBrushProfile::GreasePencil.preset().pressure_min()
    );

    fresh.brush_shelf.set_pressure_curve(0.23, 1.65);
    fresh.brush_light = 0.23;
    fresh.brush_pressure_gamma = 1.65;
    let edited = fresh.preferences_json().expect("json");
    let mut restored = workspace();
    restored.restore_preferences(&edited).expect("restored");
    assert_eq!(restored.brush_shelf.builtin(), None);
    assert!((restored.brush_shelf.pressure_min() - 0.23).abs() < 1e-4);
    assert!((restored.brush_shelf.pressure_gamma() - 1.65).abs() < 1e-4);
}

#[test]
fn the_preview_follows_the_nib_and_the_properties_panel_draws_its_controls() {
    let mut ws = workspace();
    ws.brush_size = 20.0;
    ws.brush_shelf = BrushShelf::default().with_loose_nib(Nib::new(NibShape::Round, 0.0, 10.0));
    ws.set_tool(Tool::Brush);
    let along: Vec<InputSample> = (0..5)
        .map(|i| InputSample::new(f64::from(i) * 10.0, 5.0, 1.0, PointerKind::Mouse).expect("s"))
        .collect();
    let across: Vec<InputSample> = (0..5)
        .map(|i| InputSample::new(5.0, f64::from(i) * 10.0, 1.0, PointerKind::Mouse).expect("s"))
        .collect();
    let previews = ws.previews_of(&[along, across], 1.0);
    assert!(previews[0].halves.iter().all(|h| *h < 2.0));
    assert!(previews[1].halves.iter().all(|h| *h > 9.0));
    let ctx = egui::Context::default();
    run(&mut ws, &ctx, 0.0);
}

#[test]
fn custom_brushes_are_made_chosen_edited_and_deleted() {
    let mut ws = workspace();
    ws.brush_shelf
        .set_nib(Nib::new(NibShape::Square, 30.0, 40.0));
    ws.brush_shelf.keep_nib().ok();
    // The new brush is the one in hand, so changing the nib would edit it: go back to the plain
    // brush to make a second.
    ws.brush_shelf.choose(None);
    ws.brush_shelf
        .set_nib(Nib::new(NibShape::Round, 10.0, 20.0));
    ws.brush_shelf.keep_nib().ok();
    let names: Vec<_> = ws
        .brush_shelf
        .presets()
        .iter()
        .map(|b| b.name.clone())
        .collect();
    assert_eq!(names, ["Custom brush 1", "Custom brush 2"]);
    assert_eq!(ws.brush_shelf.selected(), Some(1));
    // Choosing the first puts its nib in hand; the plain brush is a circle.
    ws.brush_shelf.choose(Some(0));
    assert_eq!(ws.brush_shelf.nib().shape(), NibShape::Square);
    ws.brush_shelf.choose(None);
    assert_eq!(ws.brush_shelf.nib(), Nib::ROUND);
    assert_eq!(ws.brush_shelf.selected(), None);
    // Changing the nib while one is chosen edits that brush, not the others.
    ws.brush_shelf.choose(Some(1));
    ws.brush_shelf
        .set_nib(Nib::new(NibShape::Round, 80.0, 50.0));
    assert!((ws.brush_shelf.presets()[1].angle - 80.0).abs() < 1e-4);
    assert_eq!(ws.brush_shelf.presets()[1].name, "Custom brush 2");
    assert!((ws.brush_shelf.presets()[0].angle - 30.0).abs() < 1e-4);
    // With none chosen, changing the nib edits no brush.
    ws.brush_shelf.choose(None);
    ws.brush_shelf.set_nib(Nib::new(NibShape::Square, 5.0, 5.0));
    assert!((ws.brush_shelf.presets()[0].angle - 30.0).abs() < 1e-4);
    // Delete removes the chosen one and falls back to the plain brush; with none chosen it is a no-op.
    ws.brush_shelf.delete_selected();
    assert_eq!(ws.brush_shelf.presets().len(), 2);
    ws.brush_shelf.choose(Some(0));
    ws.brush_shelf.delete_selected();
    assert_eq!(ws.brush_shelf.presets().len(), 1);
    assert_eq!(ws.brush_shelf.nib(), Nib::ROUND);
    // A place that is not there means the plain brush.
    ws.brush_shelf.choose(Some(9));
    assert_eq!(ws.brush_shelf.selected(), None);
}

#[test]
fn only_so_many_custom_brushes_are_kept_and_the_names_stay_sensible() {
    let mut ws = workspace();
    for _ in 0..MAX_BRUSHES + 5 {
        ws.new_custom_brush();
    }
    assert_eq!(ws.brush_shelf.presets().len(), MAX_BRUSHES);
    assert!(ws.status.is_some(), "the limit is said");
    let mut loaded = workspace();
    let many: Vec<String> = (0..MAX_BRUSHES + 10)
        .map(|i| format!(r#"{{"name": "b{i}"}}"#))
        .collect();
    loaded
        .restore_preferences(&format!(r#"{{"brushes": [{}]}}"#, many.join(",")))
        .expect("loads");
    assert_eq!(loaded.brush_shelf.presets().len(), MAX_BRUSHES);
    let mut odd = workspace();
    odd.restore_preferences(
        r#"{"brushes": [{"name": "   ", "flatness": 0, "angle": 720},
                         {"name": "a\u0007b", "square": true}]}"#,
    )
    .expect("loads");
    let kept = odd.brush_shelf.presets();
    assert_eq!(kept[0].name, "Custom brush");
    assert!((kept[0].flatness - 1.0).abs() < 1e-4 && kept[0].angle < 1e-4);
    assert_eq!(kept[1].name, "ab");
}

#[test]
fn custom_brushes_and_the_chosen_one_are_kept_with_the_preferences() {
    let mut ws = workspace();
    ws.brush_shelf
        .set_nib(Nib::new(NibShape::Square, 20.0, 60.0));
    ws.brush_shelf.keep_nib().ok();
    ws.brush_shelf.choose(None);
    ws.brush_shelf
        .set_nib(Nib::new(NibShape::Round, 70.0, 25.0));
    ws.brush_shelf.keep_nib().ok();
    ws.brush_shelf.choose(Some(0));
    let saved = ws.preferences_json().expect("json");
    let mut fresh = workspace();
    fresh.restore_preferences(&saved).expect("restored");
    assert_eq!(fresh.brush_shelf.presets(), ws.brush_shelf.presets());
    assert_eq!(fresh.brush_shelf.selected(), Some(0));
    assert_eq!(fresh.brush_shelf.nib().shape(), NibShape::Square);
    // A chosen place past the end is the plain brush.
    let mut past = workspace();
    past.restore_preferences(r#"{"brushes": [], "brush_selected": 3}"#)
        .expect("loads");
    assert_eq!(past.brush_shelf.selected(), None);
    assert_eq!(past.brush_shelf.nib(), Nib::ROUND);
}

#[test]
fn the_brush_cursor_shows_the_nib_at_its_size_only_while_painting_with_the_brush() {
    let mut ws = workspace();
    ws.brush_size = 20.0;
    ws.pointer_stage = Some(Pos2::new(100.0, 50.0));
    ws.set_tool(Tool::Select);
    assert!(ws.brush_cursor().is_none());
    ws.set_tool(Tool::Brush);
    let round = ws.brush_cursor().expect("cursor");
    assert!(
        round
            .iter()
            .all(|p| ((p.x - 100.0).hypot(p.y - 50.0) - 10.0).abs() < 1e-3)
    );
    ws.brush_shelf.set_nib(Nib::new(NibShape::Round, 0.0, 10.0));
    let flat = ws.brush_cursor().expect("cursor");
    let tall = flat.iter().map(|p| (p.y - 50.0).abs()).fold(0.0, f32::max);
    assert!(tall < 1.5, "{tall}");
    ws.pointer_stage = None;
    assert!(ws.brush_cursor().is_none(), "no pointer over the stage");
}
