//! Dragging a layer's row in the timeline to restack it.

use super::layer_names::{press, settle};
use super::*;
use egui::PointerButton;

const ROW: f32 = crate::workspace::timeline::ROW;

fn names(ws: &Workspace) -> Vec<String> {
    ws.document().scenes()[0]
        .timeline()
        .layers()
        .iter()
        .map(|l| l.name().to_owned())
        .collect()
}

/// Presses on the name of the layer on row `from`, drags to the top edge of row `to`, lets go.
fn drag_row(ws: &mut Workspace, ctx: &egui::Context, time: &mut f64, from: usize, to: usize) {
    let l = ws.timeline_layout.expect("the timeline was drawn");
    let x = l.labels_left + 30.0;
    let start = Pos2::new(x, l.row_top(from) + ROW / 2.0);
    let end = Pos2::new(x, l.row_top(to) + 2.0);
    let mut step = |ws: &mut Workspace, events: Vec<Event>| {
        *time += 0.05;
        pass(ws, ctx, input(*time, events));
    };
    step(ws, vec![Event::PointerMoved(start)]);
    step(ws, vec![press(start, PointerButton::Primary, true)]);
    for i in 1..=4 {
        let at = start + (end - start) * (i as f32 / 4.0);
        step(ws, vec![Event::PointerMoved(at)]);
    }
    step(ws, vec![press(end, PointerButton::Primary, false)]);
    step(ws, vec![]);
}

#[test]
fn dragging_a_layer_down_its_name_column_restacks_it_as_one_undo_step() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    flare_ui::icons::install(&ctx);
    ws.perform(MenuAction::NewLayer);
    ws.perform(MenuAction::NewLayer);
    let mut time = 0.0;
    settle(&mut ws, &ctx, &mut time);
    let before = names(&ws);
    assert_eq!(before.len(), 3);
    let steps = ws.active().undo_labels().count();

    // The top layer, dropped under the second.
    drag_row(&mut ws, &ctx, &mut time, 0, 2);
    let after = names(&ws);
    assert_eq!(after, [before[1].as_str(), &before[0], &before[2]]);
    assert_eq!(ws.active().undo_labels().count(), steps + 1);
    let active = ws
        .active_layer_id()
        .and_then(|id| ws.layer(id))
        .map(Layer::name);
    assert_eq!(
        active,
        Some(before[0].as_str()),
        "the dragged layer is the active one"
    );

    // And back up above the first.
    drag_row(&mut ws, &ctx, &mut time, 1, 0);
    assert_eq!(names(&ws), before);
    ws.undo();
    assert_eq!(names(&ws), after);
}

#[test]
fn a_drag_on_the_frames_does_not_restack_layers() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    flare_ui::icons::install(&ctx);
    ws.perform(MenuAction::NewLayer);
    let mut time = 0.0;
    settle(&mut ws, &ctx, &mut time);
    let before = names(&ws);
    let l = ws.timeline_layout.expect("drawn");
    let (start, end) = (
        Pos2::new(l.frame_x(30.5), l.row_top(0) + 10.0),
        Pos2::new(l.frame_x(30.5), l.row_top(1) + 14.0),
    );
    let mut step = |ws: &mut Workspace, events: Vec<Event>| {
        time += 0.05;
        pass(ws, &ctx, input(time, events));
    };
    step(&mut ws, vec![Event::PointerMoved(start)]);
    step(&mut ws, vec![press(start, PointerButton::Primary, true)]);
    step(&mut ws, vec![Event::PointerMoved(end)]);
    step(&mut ws, vec![press(end, PointerButton::Primary, false)]);
    assert_eq!(names(&ws), before);
}
