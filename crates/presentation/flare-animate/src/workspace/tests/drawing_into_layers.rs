//! Everything drawn on a layer stays on that layer, as in Animate.

use super::*;
use flare_input::{InputSample, PointerKind};

fn stroke(points: &[(f64, f64)]) -> Vec<InputSample> {
    points
        .iter()
        .map(|&(x, y)| InputSample::new(x, y, 1.0, PointerKind::Mouse).expect("sample"))
        .collect()
}

fn layers(ws: &Workspace) -> &[Layer] {
    ws.document().scenes()[0].timeline().layers()
}

/// How many elements each layer has at `frame`.
fn elements_at(ws: &Workspace, frame: u32) -> Vec<usize> {
    layers(ws)
        .iter()
        .map(|l| l.span_at(frame).map_or(0, |s| s.element_count()))
        .collect()
}

fn draw_second(ws: &mut Workspace) {
    ws.commit_rectangle(Pos2::new(100.0, 10.0), Pos2::new(150.0, 60.0));
}

#[test]
fn shapes_drawn_one_after_another_share_the_layer_and_its_keyframe() {
    let mut ws = workspace();
    rect(&mut ws);
    draw_second(&mut ws);
    assert_eq!(layer_count(&ws), 1, "no layer is made for a shape");
    assert_eq!(layers(&ws)[0].spans().len(), 1);
    let span = &layers(&ws)[0].spans()[0];
    assert_eq!(
        (span.start(), span.duration().get(), span.element_count()),
        (0, 24, 2)
    );
    assert_eq!(ws.active_layer_id(), Some(layers(&ws)[0].id()));
}

#[test]
fn each_drawn_shape_is_one_undo_step_with_the_drawing_label() {
    let mut ws = workspace();
    rect(&mut ws);
    draw_second(&mut ws);
    ws.undo();
    assert_eq!(elements_at(&ws, 0), vec![1]);
    ws.undo();
    assert_eq!(elements_at(&ws, 0), vec![0]);
    assert!(layers(&ws)[0].spans().is_empty());
    ws.redo();
    ws.redo();
    assert_eq!(elements_at(&ws, 0), vec![2]);
}

#[test]
fn drawing_on_a_frame_with_no_keyframe_starts_one_there() {
    let mut ws = workspace();
    rect(&mut ws);
    let first = ws.active_layer_id().expect("layer");
    // A second layer 60 frames long makes the scene long enough to stand at frame 30.
    ws.add_layer();
    let scene = ws.scene_id().expect("scene");
    let other = ws.active_layer_id().expect("layer");
    let span = FrameSpan::new(0, NonZeroU32::new(60).expect("nonzero"), Content::Empty);
    ws.active_mut()
        .apply([Command::InsertSpan {
            scene,
            layer: other,
            span,
        }])
        .expect("insert");
    ws.select_layer(first);
    ws.set_frame(30);
    draw_second(&mut ws);
    let spans = layers(&ws)[0].spans();
    assert_eq!(spans.len(), 2);
    assert_eq!((spans[1].start(), spans[1].element_count()), (30, 1));
    assert_eq!(elements_at(&ws, 0)[0], 1, "the first keyframe is untouched");
}

#[test]
fn a_chosen_layer_takes_the_drawing_and_the_others_do_not() {
    let mut ws = workspace();
    let original = layers(&ws)[0].id();
    ws.add_layer();
    assert_eq!(layer_count(&ws), 2);
    // The new layer is the active one, so it takes the first drawing...
    let added = ws.active_layer_id().expect("active");
    assert_ne!(added, original);
    rect(&mut ws);
    let count = |ws: &Workspace, id| {
        layers(ws)
            .iter()
            .find(|l| l.id() == id)
            .and_then(|l| l.span_at(0))
            .map_or(0, |s| s.element_count())
    };
    assert_eq!((count(&ws, added), count(&ws, original)), (1, 0));
    // ...until another layer is chosen.
    ws.select_layer(original);
    draw_second(&mut ws);
    assert_eq!((count(&ws, added), count(&ws, original)), (1, 1));
    assert_eq!(ws.active_layer_id(), Some(original));
}

#[test]
fn a_document_without_layers_gets_the_translated_first_layer() {
    let mut ws = workspace();
    let scene = ws.scene_id().expect("scene");
    let layer = layers(&ws)[0].id();
    ws.active_mut()
        .apply([Command::RemoveLayer { scene, layer }])
        .expect("remove");
    rect(&mut ws);
    assert_eq!(layer_count(&ws), 1);
    assert_eq!(layers(&ws)[0].name(), "Layer 1");
    assert_eq!(elements_at(&ws, 0), vec![1]);
    ws.undo();
    assert_eq!(layer_count(&ws), 0, "the layer and the shape are one step");
}

#[test]
fn a_layer_that_cannot_take_a_drawing_says_why_and_the_document_stays() {
    for (setup, message) in [
        (0, "locked"),
        (1, "hidden"),
        (2, "folder, mask, guide or camera"),
        (3, "folder, mask, guide or camera"),
    ] {
        let mut ws = workspace();
        let layer = layers(&ws)[0].id();
        match setup {
            0 => ws.toggle_layer_locked(layer),
            1 => ws.toggle_layer_visible(layer),
            2 => ws.toggle_layer_mask(layer),
            _ => ws.toggle_layer_guide(layer),
        }
        let before = ws.document().clone();
        ws.set_tool(Tool::Rectangle);
        rect(&mut ws);
        assert_eq!(ws.document(), &before, "case {setup}");
        let status = ws.status().expect("a message");
        assert!(status.contains(message), "case {setup}: {status}");
    }
}

#[test]
fn a_keyframe_that_tweens_refuses_a_second_drawing() {
    let mut ws = workspace();
    rect(&mut ws);
    ws.selected = Some(SpanRef {
        layer: layers(&ws)[0].id(),
        start: 0,
        element: 0,
    });
    ws.set_selection_tween(Tween::Shape);
    assert_eq!(layers(&ws)[0].spans()[0].tween(), Tween::Shape);
    let before = ws.document().clone();
    draw_second(&mut ws);
    assert_eq!(ws.document(), &before);
    assert!(ws.status().expect("a message").contains("tween"));
}

#[test]
fn symmetry_puts_every_reflection_on_the_same_layer() {
    let mut ws = workspace();
    ws.set_tool(Tool::Pencil);
    ws.paint.symmetry = flare_tools::mirror::Symmetry::Both;
    ws.paint.symmetry_center = Some(Pos2::new(100.0, 100.0));
    ws.freehand = stroke(&[(10.0, 10.0), (40.0, 30.0), (60.0, 10.0)]);
    ws.finish_freehand(1.0);
    assert_eq!(layer_count(&ws), 1);
    assert_eq!(elements_at(&ws, 0), vec![4]);
    ws.undo();
    assert_eq!(elements_at(&ws, 0), vec![0], "all four are one undo step");
}

#[test]
fn clicking_picks_the_element_under_the_pointer_and_moving_or_deleting_acts_on_it_alone() {
    let mut ws = workspace();
    rect(&mut ws); // 10..110 x 10..60
    ws.commit_rectangle(Pos2::new(200.0, 10.0), Pos2::new(260.0, 60.0));
    ws.set_tool(Tool::Select);
    ws.select_at(Pos2::new(230.0, 30.0), false);
    let picked = ws.selected.expect("an element");
    assert_eq!((picked.start, picked.element), (0, 1));

    ws.move_selection(Vec2::new(0.0, 100.0));
    let scene = ws.scene_id().expect("scene");
    let top = |element| {
        ops::span_bounds(ws.document(), scene, SpanRef { element, ..picked })
            .expect("bounds")
            .top()
    };
    assert_eq!((top(0), top(1)), (10.0, 110.0));

    ws.delete_selection();
    assert_eq!(
        elements_at(&ws, 0),
        vec![1],
        "the other shape is still there"
    );
    assert!(ws.selected.is_none(), "the picked element is gone");
    ws.undo();
    assert_eq!(elements_at(&ws, 0), vec![2]);
}

#[test]
fn select_all_takes_every_element_and_one_move_is_one_undo_step() {
    let mut ws = workspace();
    rect(&mut ws);
    draw_second(&mut ws);
    ws.set_tool(Tool::Select);
    ws.select_all();
    assert_eq!(ws.selection_all().len(), 2);
    let scene = ws.scene_id().expect("scene");
    let before: Vec<Rect> = ws
        .selection_all()
        .iter()
        .filter_map(|at| ops::span_bounds(ws.document(), scene, *at))
        .collect();
    ws.move_selection(Vec2::new(5.0, 7.0));
    let after: Vec<Rect> = ws
        .selection_all()
        .iter()
        .filter_map(|at| ops::span_bounds(ws.document(), scene, *at))
        .collect();
    for (b, a) in before.iter().zip(&after) {
        assert_eq!(a.min, b.min + Vec2::new(5.0, 7.0));
    }
    ws.undo();
    let undone: Vec<Rect> = ws
        .selection_all()
        .iter()
        .filter_map(|at| ops::span_bounds(ws.document(), scene, *at))
        .collect();
    assert_eq!(undone, before, "both elements went back in one step");
}

#[test]
fn bring_to_front_and_send_to_back_reorder_inside_the_keyframe() {
    let mut ws = workspace();
    rect(&mut ws); // element 0, x 10
    draw_second(&mut ws); // element 1, x 100
    ws.set_tool(Tool::Select);
    ws.select_at(Pos2::new(30.0, 30.0), false);
    assert_eq!(ws.selected.map(|s| s.element), Some(0));
    ws.arrange_selection(true);
    assert_eq!(layer_count(&ws), 1, "no layer is moved");
    assert_eq!(ws.selected.map(|s| s.element), Some(1), "it is on top now");
    ws.arrange_selection(false);
    assert_eq!(ws.selected.map(|s| s.element), Some(0));
}

#[test]
fn duplicate_and_group_work_on_the_picked_elements() {
    let mut ws = workspace();
    rect(&mut ws);
    draw_second(&mut ws);
    ws.set_tool(Tool::Select);
    ws.select_all();
    ws.perform(MenuAction::Group);
    assert_eq!(
        elements_at(&ws, 0),
        vec![1],
        "the two shapes became one instance"
    );
    assert!(ws.selection_has_instance());
    ws.perform(MenuAction::Ungroup);
    assert_eq!(elements_at(&ws, 0), vec![2]);
    ws.select_all();
    ws.perform(MenuAction::Duplicate);
    assert_eq!(elements_at(&ws, 0), vec![4]);
    assert_eq!(layer_count(&ws), 1);
}

#[test]
fn the_pen_and_the_text_tool_draw_into_the_layer_too() {
    let mut ws = workspace();
    rect(&mut ws);
    ws.set_tool(Tool::Pen);
    for p in [(10.0, 100.0), (60.0, 100.0), (60.0, 150.0)] {
        ws.pen_press(Pos2::new(p.0, p.1));
        ws.pen_release();
    }
    ws.pen_finish();
    assert_eq!(layer_count(&ws), 1);
    assert_eq!(elements_at(&ws, 0), vec![2]);
}
