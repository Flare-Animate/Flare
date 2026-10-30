use super::*;
use crate::MenuEntry;
use flare_document::Tween;

/// A rectangle on frames 0 to 9 and another at frame 10 that is 100 units to the right.
fn two_keyframes() -> Workspace {
    let mut ws = workspace();
    draw_and_select(&mut ws);
    ws.set_frame(10);
    ws.frame_edit(FrameOp::InsertKeyframe);
    ws.set_tool(Tool::Select);
    ws.begin_drag(Pos2::new(50.0, 30.0));
    ws.update_drag(Pos2::new(150.0, 30.0));
    ws.end_drag();
    ws.set_frame(0);
    ws.select_layer(ws.active_layer_id().expect("layer"));
    ws.begin_drag(Pos2::new(50.0, 30.0));
    ws.end_drag();
    ws
}

fn left_edge(ws: &Workspace, frame: u32) -> f32 {
    let scene = ws.document().scenes()[0].id();
    let evaluated = flare_eval::evaluate_frame(ws.document(), scene, frame).expect("frame");
    ops::shape_bounds(&evaluated.shapes[0].shape)
        .expect("bounds")
        .min
        .x
}

#[test]
fn a_shape_tween_is_made_from_the_menu_on_a_selected_shape_and_moves_the_drawing() {
    let mut ws = two_keyframes();
    assert!(ws.selection_holds_shape());
    assert!(ws.is_enabled(MenuAction::ToggleShapeTween));
    assert!(
        !ws.is_enabled(MenuAction::ToggleMotionTween),
        "that is for instances"
    );
    assert_eq!(ws.label(MenuAction::ToggleShapeTween), "Create Shape Tween");
    let still = left_edge(&ws, 5);
    ws.perform(MenuAction::ToggleShapeTween);
    assert_eq!(ws.selection_tween(), Some(Tween::Shape));
    assert_eq!(ws.label(MenuAction::ToggleShapeTween), "Remove Shape Tween");
    let first = left_edge(&ws, 0);
    let middle = left_edge(&ws, 5);
    let last = left_edge(&ws, 10);
    assert!(
        first < middle && middle < last,
        "{first} < {middle} < {last}"
    );
    assert!(
        (middle - (first + last) / 2.0).abs() < 1e-3,
        "even speed lands in the middle"
    );
    assert!(
        (still - first).abs() < 1e-3,
        "before the tween the drawing was held"
    );
    ws.undo();
    assert_eq!(
        ws.selection_tween(),
        Some(Tween::None),
        "one undo step undoes it"
    );
    ws.redo();
    ws.perform(MenuAction::ToggleShapeTween);
    assert_eq!(
        ws.selection_tween(),
        Some(Tween::None),
        "the same command removes it"
    );
}

#[test]
fn the_ease_of_the_selected_tween_is_set_clamped_and_undone() {
    let mut ws = two_keyframes();
    ws.perform(MenuAction::ToggleShapeTween);
    let even = left_edge(&ws, 5);
    ws.set_selection_ease(100);
    assert_eq!(ws.selection_ease(), Some(100));
    assert!(
        left_edge(&ws, 5) > even,
        "an ease-out is further along at the middle"
    );
    ws.set_selection_ease(-100);
    assert!(left_edge(&ws, 5) < even, "an ease-in is not as far");
    ws.undo();
    assert_eq!(ws.selection_ease(), Some(100));
    let revision = ws.active().revision();
    ws.set_selection_ease(100);
    assert_eq!(ws.active().revision(), revision, "no change is no edit");
}

#[test]
fn a_selection_that_cannot_tween_leaves_the_commands_off() {
    let mut ws = workspace();
    assert!(!ws.is_enabled(MenuAction::ToggleShapeTween));
    assert!(!ws.is_enabled(MenuAction::ToggleMotionTween));
    assert_eq!(ws.selection_ease(), None);
    ws.set_selection_ease(50);
    assert!(!ws.is_dirty());
}

#[test]
fn the_menus_offer_a_motion_tween_and_a_shape_tween_and_no_dimmed_stand_in() {
    let ws = workspace();
    let entries = crate::all_entries();
    let labels: Vec<String> = entries
        .iter()
        .filter_map(|e| match e {
            MenuEntry::Action(a) => Some(ws.label(*a)),
            _ => None,
        })
        .collect();
    assert!(
        labels.iter().any(|l| l == "Create Motion Tween"),
        "{labels:?}"
    );
    assert!(
        labels.iter().any(|l| l == "Create Shape Tween"),
        "{labels:?}"
    );
    let dimmed = entries
        .iter()
        .any(|e| matches!(e, MenuEntry::Mock(name) if name.contains("Tween")));
    assert!(!dimmed, "a tween command that exists is not a mock");
}

#[test]
fn the_properties_panel_draws_the_tween_controls_for_a_shape_an_instance_and_nothing() {
    let ctx = egui::Context::default();
    for build in [
        |ws: &mut Workspace| draw_and_select(ws),
        |ws: &mut Workspace| {
            draw_and_select(ws);
            ws.convert_selection_to_symbol();
        },
        |_: &mut Workspace| {},
    ] {
        let mut ws = workspace();
        build(&mut ws);
        run(&mut ws, &ctx, 0.0);
        run(&mut ws, &ctx, 0.1);
    }
}

#[test]
fn a_tweened_bar_in_the_timeline_has_its_own_color() {
    use crate::timeline_view::span_color;
    let ws = two_keyframes();
    let id = ws.active_layer_id().expect("the layer with the drawing");
    let layers = ws.document().scenes()[0].timeline().layers();
    let layer = layers.iter().find(|l| l.id() == id).expect("layer");
    let content = layer.spans()[0].content().clone();
    let held = span_color(&content, Tween::None);
    let shape = span_color(&content, Tween::Shape);
    let motion = span_color(&content, Tween::Linear);
    assert!(held != shape && shape != motion && held != motion);
}
