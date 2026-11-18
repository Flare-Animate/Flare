//! The graph view itself, driven by the pointer: a wire drawn from the Layer's output to an
//! input, a connected input clicked to cut it, a node dragged to a new place.

use egui::PointerButton;

use super::super::layer_names::press;
use super::*;
use crate::workspace::panels::effects_graph::{HEADER, NODE_WIDTH, PORT_ROW};

/// Where the text `wanted` was drawn, as a rectangle, in a pass of `ws`.
fn text_rect(ws: &mut Workspace, ctx: &egui::Context, time: f64, wanted: &str) -> Option<Rect> {
    let mut out = ctx.run_ui(input(time, vec![]), |ui| ws.show(ui));
    out.textures_delta.clear();
    out.shapes.iter().rev().find_map(|s| match &s.shape {
        egui::epaint::Shape::Text(t) if t.galley.text() == wanted => Some(t.visual_bounding_rect()),
        _ => None,
    })
}

/// The workspace with a drawing, an Over node after it and the Effects panel open.
fn opened() -> (egui::Context, Workspace, f64, NodeId) {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    flare_ui::icons::install(&ctx);
    rect(&mut ws);
    let over = ws.add_layer_effect(EffectKind::Over).expect("added");
    ws.perform(MenuAction::ShowEffects);
    let mut time = 0.0;
    super::super::layer_names::settle(&mut ws, &ctx, &mut time);
    (ctx, ws, time, over)
}

/// A drag from `from` to `to` in a few steps.
fn drag(ws: &mut Workspace, ctx: &egui::Context, time: &mut f64, from: Pos2, to: Pos2) {
    let mut events = vec![
        vec![Event::PointerMoved(from)],
        vec![press(from, PointerButton::Primary, true)],
    ];
    for step in 1..=6 {
        events.push(vec![Event::PointerMoved(
            from + (to - from) * (step as f32 / 6.0),
        )]);
    }
    events.push(vec![press(to, PointerButton::Primary, false)]);
    events.push(vec![]);
    for e in events {
        *time += 0.05;
        pass(ws, ctx, input(*time, e));
    }
}

/// The Layer node's output: right of its box, level with its first row.
fn layer_output(ws: &mut Workspace, ctx: &egui::Context, time: f64) -> Pos2 {
    let title = text_rect(ws, ctx, time, "Layer").expect("the Layer node is drawn");
    let top = title.center().y - HEADER / 2.0;
    Pos2::new(
        title.left() - 8.0 + NODE_WIDTH,
        top + HEADER + PORT_ROW / 2.0,
    )
}

/// The input whose label is `name`: just left of the label.
fn input_port(ws: &mut Workspace, ctx: &egui::Context, time: f64, name: &str) -> Pos2 {
    let label = text_rect(ws, ctx, time, name).expect("the input is drawn");
    Pos2::new(label.left() - 9.5, label.center().y)
}

#[test]
fn a_wire_drawn_from_an_output_to_an_input_connects_them_in_one_step() {
    let (ctx, mut ws, mut time, over) = opened();
    let before = steps(&ws);
    let from = layer_output(&mut ws, &ctx, time);
    let to = input_port(&mut ws, &ctx, time, "Under");
    // "Over" is both the node's title and its first input; the first input is a row above Under.
    let to = to - egui::vec2(0.0, PORT_ROW);
    drag(&mut ws, &ctx, &mut time, from, to);
    let graph = effects(&ws).expect("effects");
    let node = graph.node(over).expect("over");
    assert_eq!(node.inputs()[0], Some(NodeId::LAYER), "{:?}", node.inputs());
    assert_eq!(steps(&ws), before + 1);
}

#[test]
fn clicking_a_connected_input_cuts_it() {
    let (ctx, mut ws, mut time, over) = opened();
    let under = input_port(&mut ws, &ctx, time, "Under");
    super::super::layer_names::click(&mut ws, &ctx, &mut time, under, PointerButton::Primary);
    let graph = effects(&ws).expect("effects");
    assert_eq!(graph.node(over).expect("over").inputs()[1], None);
}

#[test]
fn dragging_a_node_moves_it_in_the_view_and_not_the_picture() {
    let (ctx, mut ws, mut time, over) = opened();
    let title = text_rect(&mut ws, &ctx, time, "Under").expect("drawn");
    let before = effects(&ws).expect("effects");
    let start = before.node(over).expect("over").at();
    // The body of the Over node, below its inputs.
    let grab = Pos2::new(title.right() + 20.0, title.bottom() + 4.0);
    drag(
        &mut ws,
        &ctx,
        &mut time,
        grab,
        grab + egui::vec2(30.0, 12.0),
    );
    let after = effects(&ws).expect("effects");
    let moved = after.node(over).expect("over").at();
    assert!(
        (moved[0] - start[0] - 30.0).abs() <= 1.0,
        "{start:?} -> {moved:?}"
    );
    assert!(
        (moved[1] - start[1] - 12.0).abs() <= 1.0,
        "{start:?} -> {moved:?}"
    );
    assert!(after.same_picture(&before));
}
