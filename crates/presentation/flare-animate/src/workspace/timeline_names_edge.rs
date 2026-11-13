//! The edge between the timeline's layer names and its frames, dragged sideways to make the
//! names column wider or narrower. The width is kept with the preferences.

use egui::{CursorIcon, Pos2, Rect, Sense, Stroke};

use super::timeline::{LABEL_W, Layout};
use super::*;

/// The narrowest the names column goes: a short name and its toggles still fit.
pub(super) const MIN_NAMES: f32 = 120.0;
/// The widest it goes, so frames are always left to see.
pub(super) const MAX_NAMES: f32 = 600.0;
/// How far either side of the edge a press takes it, in points.
const EDGE_REACH: f32 = 3.0;

/// `width` kept within what the names column can be; anything not a number is the default.
pub(super) fn names_width(width: f32) -> f32 {
    if width.is_finite() {
        width.clamp(MIN_NAMES, MAX_NAMES)
    } else {
        LABEL_W
    }
}

impl Workspace {
    /// The grip on the edge of the names column: the cursor shows it can be dragged, and a drag
    /// moves the edge with the pointer.
    pub(super) fn names_edge(&mut self, ui: &egui::Ui, l: &Layout) {
        let view = ui.clip_rect();
        let strip = Rect::from_min_max(
            Pos2::new(l.label_edge - EDGE_REACH, view.min.y.max(l.ruler_top)),
            Pos2::new(l.label_edge + EDGE_REACH, view.max.y.min(l.rect.max.y)),
        );
        if !strip.is_positive() {
            return;
        }
        let response = ui.interact(
            strip,
            ui.make_persistent_id("timeline-names-edge"),
            Sense::drag(),
        );
        if response.dragged() {
            let width = self.display.timeline_names + response.drag_delta().x;
            self.display.timeline_names = names_width(width);
        }
        if response.hovered() || response.dragged() {
            ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal);
            ui.painter().vline(
                l.label_edge,
                strip.y_range(),
                Stroke::new(2.0, flare_ui::theme::accent()),
            );
        }
    }
}
