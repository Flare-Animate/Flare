//! Placing the figure on the stage: its layer and keyframe, and moving it as a picture.

use super::*;

#[test]
fn placing_the_figure_adds_an_unlocked_layer_on_top_as_one_undo_step() {
    let mut ws = workspace();
    let before = layer_count(&ws);
    ws.set_mannequin_pose("run");
    assert!(ws.place_mannequin());
    assert_eq!(layer_count(&ws), before + 1);
    let top = ws.document().scenes()[0].timeline().layers()[0].clone();
    assert_eq!(top.name(), "Mannequin");
    assert!(!top.is_locked());
    assert_eq!(
        top.view_opacity(),
        flare_tools::mannequin::REFERENCE_OPACITY
    );
    assert!(matches!(top.spans()[0].content(), Content::Bitmap(_)));
    assert!(ws.status().is_some_and(|s| s.contains("Mannequin")));
    ws.perform(MenuAction::Undo);
    assert_eq!(layer_count(&ws), before);
}

#[test]
fn the_placed_figure_is_selected_and_moves_and_scales_like_any_picture() {
    let mut ws = workspace();
    assert!(ws.place_mannequin());
    assert_eq!(ws.tool(), Tool::FreeTransform, "its handles show at once");
    let span = ws.selected.expect("selected");
    let top = ws.document().scenes()[0].timeline().layers()[0].id();
    assert_eq!((span.layer, span.element), (top, 0));
    let bounds = |ws: &Workspace| ws.selection_with_bounds().expect("bounds").2[0];
    let placed = bounds(&ws);

    ws.move_selection(Vec2::new(30.0, -20.0));
    let moved = bounds(&ws);
    assert!(
        (moved.min - placed.min - Vec2::new(30.0, -20.0)).length() < 0.5,
        "{moved:?}"
    );

    ws.set_selection_bounds(Rect::from_min_size(moved.min, moved.size() * 0.5));
    let scaled = bounds(&ws);
    assert!(
        (scaled.width() - moved.width() * 0.5).abs() < 1.0,
        "{scaled:?}"
    );
    assert!((scaled.height() - moved.height() * 0.5).abs() < 1.0);

    // A press on it on the stage picks it again, as the Selection tool does; the tools that
    // paint or erase shapes do not take it for one.
    ws.selected = None;
    assert_eq!(ws.pick(scaled.center()), Some(span));
    assert_eq!(ws.hit(scaled.center()), None);
    ws.select_at(scaled.center(), false);
    assert_eq!(ws.selected, Some(span));

    // Each change is its own undo step.
    ws.perform(MenuAction::Undo);
    ws.selected = Some(span);
    assert!((bounds(&ws).width() - moved.width()).abs() < 1.0);
}

#[test]
fn the_placed_figure_is_dragged_on_the_stage_with_the_selection_tool() {
    let mut ws = workspace();
    assert!(ws.place_mannequin());
    let bounds = |ws: &Workspace| ws.selection_with_bounds().expect("bounds").2[0];
    let placed = bounds(&ws);
    ws.selected = None;
    ws.set_tool(Tool::Select);
    let from = placed.center();
    ws.begin_drag(from);
    assert!(ws.selected.is_some(), "the press picked the picture");
    ws.update_drag(from + Vec2::new(40.0, 10.0));
    ws.end_drag();
    let moved = bounds(&ws);
    assert!(
        (moved.min - placed.min - Vec2::new(40.0, 10.0)).length() < 0.5,
        "{moved:?}"
    );
    assert!((moved.size() - placed.size()).length() < 0.5, "only moved");
}

/// The first layer's spans, as (start, length, elements).
fn top_spans(ws: &Workspace) -> Vec<(u32, u32, usize)> {
    ws.document().scenes()[0].timeline().layers()[0]
        .spans()
        .iter()
        .map(|s| (s.start(), s.duration().get(), s.element_count()))
        .collect()
}

#[test]
fn several_figures_share_a_keyframe_or_each_take_one_on_the_same_layer() {
    let mut ws = workspace();
    let layers = layer_count(&ws);
    assert!(ws.place_mannequin());
    assert_eq!(
        layer_count(&ws),
        layers + 1,
        "the first on a layer of its own"
    );
    assert_eq!(top_spans(&ws), vec![(0, 24, 1)]);

    // A second in the same frames: it joins the keyframe and is the one selected.
    ws.set_mannequin_place_into(PlaceInto::ThisKeyframe);
    ws.set_mannequin_pose("run");
    assert!(ws.place_mannequin());
    assert_eq!(layer_count(&ws), layers + 1, "no new layer");
    assert_eq!(top_spans(&ws), vec![(0, 24, 2)]);
    assert_eq!(ws.selected.map(|s| (s.start, s.element)), Some((0, 1)));

    // Another pose from frame 10: a keyframe of its own on the same layer.
    ws.set_frame(10);
    ws.set_mannequin_place_into(PlaceInto::NewKeyframe);
    ws.set_mannequin_pose("jump");
    assert!(ws.place_mannequin());
    assert_eq!(top_spans(&ws), vec![(0, 10, 2), (10, 14, 1)]);
    assert_eq!(ws.selected.map(|s| (s.start, s.element)), Some((10, 0)));

    // Each placing is its own undo step.
    ws.perform(MenuAction::Undo);
    assert_eq!(top_spans(&ws), vec![(0, 24, 2)]);
}

#[test]
fn a_figure_is_not_placed_into_a_locked_layer_and_the_reason_is_shown() {
    let mut ws = workspace();
    let layer = ws.active_layer_id().expect("layer");
    ws.toggle_layer_locked(layer);
    ws.set_mannequin_place_into(PlaceInto::ThisKeyframe);
    let layers = layer_count(&ws);
    assert!(!ws.place_mannequin());
    assert_eq!(layer_count(&ws), layers);
    assert!(
        ws.status().is_some_and(|s| s.contains("locked")),
        "{:?}",
        ws.status()
    );
}

#[test]
fn the_figure_stands_in_the_middle_of_the_stage_unless_moved() {
    let mut ws = workspace();
    let stage = ws.document().stage();
    let (w, h) = (f64::from(stage.width()), f64::from(stage.height()));
    let at = ws.mannequin_on_stage().expect("placement");
    assert_eq!(at.center, [w / 2.0, h / 2.0]);
    assert_eq!(at.height, h * 0.8);
    ws.mannequin.stage_at = Some(([10.0, 20.0], 99.0));
    let at = ws.mannequin_on_stage().expect("placement");
    assert_eq!((at.center, at.height), ([10.0, 20.0], 99.0));
    ws.mannequin.stage_at = Some(([10.0, 20.0], -1.0));
    assert!(
        ws.mannequin_on_stage().is_none(),
        "a bad size places nothing"
    );
}

#[test]
fn the_figure_is_kept_with_the_preferences_and_a_bad_one_is_ignored() {
    let mut ws = workspace();
    ws.set_mannequin_pose("sit");
    ws.set_mannequin_body(Body::Female);
    let preferences = ws.preferences();
    let mut next = workspace();
    next.apply_preferences(&preferences);
    assert_eq!(next.mannequin(), ws.mannequin());
    assert!(
        preferences.mannequin.len() < 2048,
        "small: {}",
        preferences.mannequin.len()
    );

    let mut bad = preferences.clone();
    bad.mannequin = r#"{"version": 1, "body": "robot"}"#.into();
    let mut kept = workspace();
    let default = *kept.mannequin();
    kept.apply_preferences(&bad);
    assert_eq!(*kept.mannequin(), default);
}

#[test]
fn the_view_menu_places_the_figure_under_the_drawing_and_on_a_layer_without_changing_the_panels_choice()
 {
    let mut ws = workspace();
    let before = layer_count(&ws);
    ws.set_mannequin_place_into(PlaceInto::ThisKeyframe);
    let drawing = ws.document().scenes()[0].timeline().layers()[0].id();

    assert!(ws.is_enabled(MenuAction::PlaceMannequinUnder));
    ws.perform(MenuAction::PlaceMannequinUnder);
    let layers = ws.document().scenes()[0].timeline().layers().to_vec();
    assert_eq!(layers.len(), before + 1);
    let guide = layers.last().expect("bottom layer");
    assert!(
        guide.is_reference() && guide.is_locked(),
        "a locked guide at the bottom"
    );
    assert_eq!(layers[0].id(), drawing, "the drawing layer stays on top");
    assert!(
        ws.tool() != Tool::FreeTransform,
        "the tool in hand is left alone"
    );
    assert_eq!(
        ws.mannequin.into,
        PlaceInto::ThisKeyframe,
        "the panel's choice stays"
    );

    ws.perform(MenuAction::PlaceMannequinOnLayer);
    let top = ws.document().scenes()[0].timeline().layers()[0].clone();
    assert_eq!(top.name(), "Mannequin");
    assert!(
        !top.is_reference(),
        "this one is an ordinary picture on top"
    );
    assert_eq!(ws.mannequin.into, PlaceInto::ThisKeyframe);
}

#[test]
fn the_view_menu_shows_the_figure_on_the_stage_and_hides_it_again() {
    let mut ws = workspace();
    assert!(!ws.mannequin.on_stage);
    ws.perform(MenuAction::ToggleMannequinOnStage);
    assert!(ws.mannequin.on_stage);
    ws.perform(MenuAction::ToggleMannequinOnStage);
    assert!(!ws.mannequin.on_stage);
}
