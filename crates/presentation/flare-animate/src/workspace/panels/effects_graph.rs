//! The Effects panel's graph view, Tahoma2D's FX schematic in small: each node a box with its
//! inputs down the left edge and its output on the right, wires between them. Drag a box to
//! move it, drag from an output to an input to connect them, click a connected input to cut
//! it, drag empty space to look around. Every change is one undo step, made on release.

use egui::{
    Color32, CornerRadius, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, Ui, Vec2, pos2, vec2,
};
use flare_fx::{EffectGraph, Node, NodeId};
use flare_ui::theme;

use crate::workspace::Workspace;

pub(crate) const NODE_WIDTH: f32 = 118.0;
pub(crate) const HEADER: f32 = 20.0;
pub(crate) const PORT_ROW: f32 = 16.0;
const PORT_RADIUS: f32 = 4.5;
/// How close to a port the pointer must be to take it.
const PORT_REACH: f32 = 9.0;
/// The view's inset from the panel's edge, so a node at 0, 0 is not cut.
const INSET: Vec2 = vec2(12.0, 12.0);

/// What lies under a point of the view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Hit {
    /// A node's output.
    Output(NodeId),
    /// Input `port` of a node.
    Input(NodeId, usize),
    /// A node's box, away from its ports.
    Body(NodeId),
    Empty,
}

/// A node's box, in the view's coordinates where `origin` is the graph's 0, 0.
pub(crate) fn node_rect(node: &Node, origin: Pos2) -> Rect {
    let rows = node.kind().inputs().len().max(1) as f32;
    let [x, y] = node.at();
    Rect::from_min_size(
        origin + vec2(x, y),
        vec2(NODE_WIDTH, HEADER + rows * PORT_ROW + 4.0),
    )
}

/// Where input `port` of a node sits.
pub(crate) fn input_at(node: &Node, origin: Pos2, port: usize) -> Pos2 {
    let r = node_rect(node, origin);
    pos2(r.left(), r.top() + HEADER + PORT_ROW * (port as f32 + 0.5))
}

/// Where a node's output sits; the Output node has none.
pub(crate) fn output_at(node: &Node, origin: Pos2) -> Option<Pos2> {
    if node.kind() == flare_fx::EffectKind::Output {
        return None;
    }
    let r = node_rect(node, origin);
    Some(pos2(r.right(), r.top() + HEADER + PORT_ROW * 0.5))
}

/// What is under `at`: a port before a box, and the node drawn last (on top) first.
pub(crate) fn hit(graph: &EffectGraph, origin: Pos2, at: Pos2) -> Hit {
    for node in graph.nodes().iter().rev() {
        if output_at(node, origin).is_some_and(|p| p.distance(at) <= PORT_REACH) {
            return Hit::Output(node.id());
        }
        for port in 0..node.kind().inputs().len() {
            if input_at(node, origin, port).distance(at) <= PORT_REACH {
                return Hit::Input(node.id(), port);
            }
        }
    }
    graph
        .nodes()
        .iter()
        .rev()
        .find(|n| node_rect(n, origin).contains(at))
        .map_or(Hit::Empty, |n| Hit::Body(n.id()))
}

/// A wire's curve from an output to an input, leaving and arriving level.
fn wire(from: Pos2, to: Pos2, color: Color32) -> Shape {
    let reach = ((to.x - from.x).abs() * 0.5).max(30.0);
    Shape::CubicBezier(egui::epaint::CubicBezierShape::from_points_stroke(
        [from, from + vec2(reach, 0.0), to - vec2(reach, 0.0), to],
        false,
        Color32::TRANSPARENT,
        Stroke::new(1.6, color),
    ))
}

impl Workspace {
    /// The graph view, `height` tall: draws `graph` and turns presses on it into edits.
    pub(super) fn effects_graph(&mut self, ui: &mut Ui, graph: &EffectGraph, height: f32) {
        let (rect, response) =
            ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, CornerRadius::same(3), theme::field());
        painter.rect_stroke(
            rect,
            CornerRadius::same(3),
            Stroke::new(1.0, theme::border()),
            StrokeKind::Inside,
        );
        let origin = rect.min + INSET + self.effects.pan;
        let dragged = self.effects.dragging;
        // Where each node is drawn: where it is, or where it is being dragged to.
        let shown_origin = |id: NodeId| match dragged {
            Some((node, by)) if node == id => origin + by,
            _ => origin,
        };
        let reached: Vec<NodeId> = graph.order();
        // Wires first, under the boxes.
        for node in graph.nodes() {
            for (port, from) in node.inputs().iter().enumerate() {
                let Some(source) = from.and_then(|f| graph.node(f)) else {
                    continue;
                };
                let (Some(a), b) = (
                    output_at(source, shown_origin(source.id())),
                    input_at(node, shown_origin(node.id()), port),
                ) else {
                    continue;
                };
                let live = reached.contains(&node.id()) && reached.contains(&source.id());
                let color = if live {
                    theme::accent()
                } else {
                    theme::text_disabled()
                };
                painter.add(wire(a, b, color));
            }
        }
        let pointer = response.interact_pointer_pos().or(response.hover_pos());
        if let (Some(from), Some(at)) = (self.effects.wiring.and_then(|id| graph.node(id)), pointer)
            && let Some(start) = output_at(from, shown_origin(from.id()))
        {
            painter.add(wire(start, at, theme::text()));
        }
        for node in graph.nodes() {
            self.draw_effect_node(
                &painter,
                node,
                shown_origin(node.id()),
                reached.contains(&node.id()),
            );
        }
        self.effects_graph_input(ui, &response, graph, origin);
    }

    fn draw_effect_node(&self, painter: &egui::Painter, node: &Node, origin: Pos2, live: bool) {
        let rect = node_rect(node, origin);
        let selected = self.effects.selected == Some(node.id());
        let edge = if selected {
            theme::accent()
        } else {
            theme::border()
        };
        let fill = if live {
            theme::panel_raised()
        } else {
            theme::panel()
        };
        painter.rect_filled(rect, CornerRadius::same(4), fill);
        painter.rect_stroke(
            rect,
            CornerRadius::same(4),
            Stroke::new(if selected { 1.6 } else { 1.0 }, edge),
            StrokeKind::Inside,
        );
        let title = self.tr(&format!("fx-kind-{}", node.kind().name()));
        let text = if live {
            theme::text_strong()
        } else {
            theme::text_dim()
        };
        painter.text(
            rect.left_top() + vec2(8.0, HEADER / 2.0),
            egui::Align2::LEFT_CENTER,
            title,
            egui::FontId::proportional(12.0),
            text,
        );
        for (port, name) in node.kind().inputs().iter().enumerate() {
            let at = input_at(node, origin, port);
            let connected = node.inputs().get(port).copied().flatten().is_some();
            let dot = if connected {
                theme::accent()
            } else {
                theme::text_disabled()
            };
            painter.circle(at, PORT_RADIUS, theme::field(), Stroke::new(1.4, dot));
            painter.text(
                at + vec2(PORT_RADIUS + 5.0, 0.0),
                egui::Align2::LEFT_CENTER,
                self.tr(&format!("fx-input-{name}")),
                egui::FontId::proportional(11.0),
                theme::text_dim(),
            );
        }
        if let Some(at) = output_at(node, origin) {
            painter.circle(
                at,
                PORT_RADIUS,
                theme::accent(),
                Stroke::new(1.4, theme::accent()),
            );
        }
    }

    /// Presses on the view: dragging a node, drawing a wire, cutting one, looking around.
    fn effects_graph_input(
        &mut self,
        ui: &Ui,
        response: &egui::Response,
        graph: &EffectGraph,
        origin: Pos2,
    ) {
        // A drag is known only once the pointer has moved a little, so what it took hold of is
        // what was under the press, and it moves by how far the pointer is from there.
        let pressed = ui.input(|i| i.pointer.press_origin());
        if response.drag_started()
            && let Some(at) = pressed.or(response.interact_pointer_pos())
        {
            self.effects.press = Some((at, self.effects.pan));
            match hit(graph, origin, at) {
                Hit::Output(id) => self.effects.wiring = Some(id),
                Hit::Body(id) | Hit::Input(id, _) => {
                    self.effects.selected = Some(id);
                    self.effects.dragging = Some((id, Vec2::ZERO));
                }
                Hit::Empty => {}
            }
        }
        if response.dragged()
            && let (Some((start, pan)), Some(now)) =
                (self.effects.press, response.interact_pointer_pos())
        {
            let by = now - start;
            match &mut self.effects.dragging {
                Some((_, moved)) => *moved = by,
                None if self.effects.wiring.is_none() => self.effects.pan = pan + by,
                None => {}
            }
        }
        if response.drag_stopped() {
            self.effects.press = None;
            if let Some((id, by)) = self.effects.dragging.take()
                && by != Vec2::ZERO
                && let Some([x, y]) = graph.node(id).map(Node::at)
            {
                self.move_layer_effect(id, [(x + by.x).round(), (y + by.y).round()]);
            }
            if let Some(from) = self.effects.wiring.take()
                && let Some(at) = response
                    .interact_pointer_pos()
                    .or(ui.ctx().pointer_latest_pos())
                && let Hit::Input(to, port) = hit(graph, origin, at)
                && to != from
            {
                self.connect_layer_effects(from, to, port);
            }
        }
        if response.clicked()
            && let Some(at) = response.interact_pointer_pos()
        {
            match hit(graph, origin, at) {
                Hit::Input(id, port)
                    if graph
                        .node(id)
                        .and_then(|n| n.inputs().get(port).copied().flatten())
                        .is_some() =>
                {
                    self.disconnect_layer_effect(id, port);
                }
                Hit::Body(id) | Hit::Input(id, _) | Hit::Output(id) => {
                    self.effects.selected = Some(id);
                }
                Hit::Empty => self.effects.selected = None,
            }
        }
        if response.hovered()
            && ui.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace))
            && ui.memory(|m| m.focused().is_none())
            && let Some(id) = self.effects.selected
            && graph.node(id).is_some_and(|n| !n.kind().is_fixed())
        {
            self.remove_layer_effect(id);
        }
    }
}
