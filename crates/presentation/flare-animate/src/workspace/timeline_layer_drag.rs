//! Dragging a layer's row up or down the timeline's name column to restack it, as in Animate. A
//! line shows where it will land; letting go moves it there, a folder with everything in it, into
//! the folder the line is in or out of one, as one undo step (`flare_commands::layers::move_to`).

use std::collections::HashSet;

use egui::{Painter, Pos2, Stroke};
use flare_commands::layers;
use flare_core::LayerId;
use flare_document::Layer;

use super::timeline::{FOLDER_INDENT, Layout, OnionEnd, ROW};
use super::*;

/// What a press on the timeline holds: a span being moved or resized, an onion-skin marker or a
/// layer's row. One at a time, so a press does only one of them.
#[derive(Clone, Copy, Debug)]
pub(super) enum Grab {
    Span(SpanDrag),
    Onion(OnionEnd),
    Layer(LayerDrag),
}

/// A layer's row being dragged, and where the pointer is now, in the timeline's coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct LayerDrag {
    pub layer: LayerId,
    pub y: f32,
}

/// Where a dragged layer lands: just above `before` (the bottom with none), in `folder`. The line
/// that shows it is drawn over row gap `gap`, indented `depth` folders.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct LayerDrop {
    pub before: Option<LayerId>,
    pub folder: Option<LayerId>,
    pub gap: usize,
    pub depth: usize,
}

impl Layout {
    /// The gap between rows nearest `y`: 0 above the first row, `count` below the last.
    pub(super) fn row_gap_at(&self, y: f32, count: usize) -> usize {
        let rows = (y - self.row_top(0)) / ROW;
        if !rows.is_finite() {
            return 0;
        }
        (rows.round().max(0.0) as usize).min(count)
    }
}

/// Where `layer` dropped at row gap `gap` goes. `rows` are the rows shown, as (index in `layers`,
/// depth). The layer and what is in it are passed over, so a drop beside itself leaves it where it
/// is. Below an open folder's row is inside it; anywhere else, the layer joins the folder of the
/// row under the line, so a drop under a folder's last layer takes it out.
pub(super) fn layer_drop(
    layers: &[Layer],
    rows: &[(usize, usize)],
    collapsed: &HashSet<LayerId>,
    layer: LayerId,
    gap: usize,
) -> LayerDrop {
    let mut moving = HashSet::from([layer]);
    for l in layers {
        if l.parent().is_some_and(|p| moving.contains(&p)) {
            moving.insert(l.id());
        }
    }
    let shown = |&&(index, depth): &&(usize, usize)| {
        let l = layers.get(index)?;
        (!moving.contains(&l.id())).then_some((l, depth))
    };
    let gap = gap.min(rows.len());
    let above = rows[..gap].iter().rev().find_map(|row| shown(&row));
    let below = rows[gap..].iter().find_map(|row| shown(&row));
    let (folder, depth) = match above {
        Some((l, depth)) if l.is_folder() && !collapsed.contains(&l.id()) => {
            (Some(l.id()), depth + 1)
        }
        _ => (
            below.and_then(|(l, _)| l.parent()),
            below.map_or(0, |(_, depth)| depth),
        ),
    };
    LayerDrop {
        before: below.map(|(l, _)| l.id()),
        folder,
        gap,
        depth,
    }
}

impl Workspace {
    /// Takes a layer's row by its name column, follows the pointer and lets go of it; true while
    /// one is held, so the press does nothing else.
    pub(super) fn layer_row_drag(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        l: &Layout,
        rows: &[(usize, usize)],
    ) -> bool {
        if response.drag_started()
            && self.timeline_grab.is_none()
            && let Some(p) = ui.input(|i| i.pointer.press_origin())
            && l.on_labels(p)
            && let Some(id) = l.row_at(p).and_then(|i| self.row_layer(rows, i))
        {
            self.timeline_grab = Some(Grab::Layer(LayerDrag { layer: id, y: p.y }));
            self.select_layer(id);
        }
        let Some(Grab::Layer(mut drag)) = self.timeline_grab else {
            return false;
        };
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        if let Some(p) = response.interact_pointer_pos() {
            drag.y = p.y;
            self.timeline_grab = Some(Grab::Layer(drag));
        }
        if response.drag_stopped() || !response.dragged() {
            self.timeline_grab = None;
            self.drop_layer(drag, l, rows);
        }
        true
    }

    /// Where the layer being dragged would land, if one is.
    fn layer_drop_target(&self, l: &Layout, rows: &[(usize, usize)]) -> Option<LayerDrop> {
        let Some(Grab::Layer(drag)) = self.timeline_grab else {
            return None;
        };
        let layers = self.document().scenes().first()?.timeline().layers();
        let gap = l.row_gap_at(drag.y, rows.len());
        Some(layer_drop(
            layers,
            rows,
            &self.collapsed_folders,
            drag.layer,
            gap,
        ))
    }

    fn drop_layer(&mut self, drag: LayerDrag, l: &Layout, rows: &[(usize, usize)]) {
        let Some(scene) = self.scene_id() else { return };
        let gap = l.row_gap_at(drag.y, rows.len());
        let Some(layers) = self
            .document()
            .scenes()
            .first()
            .map(|s| s.timeline().layers())
        else {
            return;
        };
        let to = layer_drop(layers, rows, &self.collapsed_folders, drag.layer, gap);
        match layers::move_to(self.document(), scene, drag.layer, to.before, to.folder) {
            Ok(commands) if commands.is_empty() => {}
            Ok(commands) => {
                self.run_batch(commands, "move the layer");
                self.validate_selection();
            }
            Err(refusal) => self.report(refusal.to_string()),
        }
    }

    /// The line across the timeline where the layer being dragged would land.
    pub(super) fn layer_drop_line(&self, painter: &Painter, l: &Layout, rows: &[(usize, usize)]) {
        let Some(to) = self.layer_drop_target(l, rows) else {
            return;
        };
        let y = l.row_top(to.gap);
        let x = l.labels_left + 4.0 + to.depth as f32 * FOLDER_INDENT;
        let accent = flare_ui::theme::accent();
        painter.line_segment(
            [Pos2::new(x, y), Pos2::new(l.rect.max.x, y)],
            Stroke::new(2.0, accent),
        );
        painter.circle_stroke(Pos2::new(x, y), 3.0, Stroke::new(1.5, accent));
    }
}

#[cfg(test)]
#[path = "timeline_layer_drag_tests.rs"]
mod tests;
