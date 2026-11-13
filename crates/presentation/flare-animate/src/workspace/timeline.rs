//! The timeline panel: where its parts are, and what a click or drag on it does. The rows of
//! frames are drawn in `timeline_rows.rs`, the layer names and the ruler in `timeline_labels.rs`.

use egui::{Color32, Pos2, Rect, Sense, Stroke, Vec2};
use flare_core::LayerId;
use flare_document::Layer;
use flare_editor::timeline_view;

use super::layer_names;
use super::*;

/// How far each level of folder indents a name, in points.
pub(super) const FOLDER_INDENT: f32 = 12.0;
/// The height of a layer's row, and of the ruler.
pub(super) const ROW: f32 = 20.0;
/// The width of the column of layer names and toggles as it starts: wide enough for the layer
/// buttons above it. Dragging its edge makes it narrower or wider (`timeline_names_edge.rs`).
pub(super) const LABEL_W: f32 = 230.0;

/// Where the parts of the timeline are, in the coordinates of the scrolled content. The name
/// column and the ruler stay in view while the frames scroll under them, as Animate's do, so a
/// layer is still named when its frames far along are shown, and the frame numbers are still
/// there when the layers are scrolled down.
#[derive(Clone, Copy, Debug)]
pub(super) struct Layout {
    /// The whole scrolled content: the ruler's row and one row a layer.
    pub rect: Rect,
    /// The left edge of the first frame.
    pub track_left: f32,
    /// The left edge of the name column, kept at the left of the view.
    pub labels_left: f32,
    /// The right edge of the name column; frames to the left of it are hidden under it.
    pub label_edge: f32,
    /// The top of the ruler, kept at the top of the view.
    pub ruler_top: f32,
    pub per_frame: f32,
}

impl Layout {
    /// The layout of `rect` scrolled so that `view` is what shows of it, with a name column
    /// `label_w` points wide.
    pub(super) fn new(rect: Rect, view: Rect, per_frame: f32, label_w: f32) -> Self {
        let labels_left = rect.min.x + (view.min.x - rect.min.x).max(0.0);
        Self {
            rect,
            track_left: rect.min.x + label_w,
            labels_left,
            label_edge: labels_left + label_w,
            ruler_top: rect.min.y + (view.min.y - rect.min.y).max(0.0),
            per_frame,
        }
    }

    /// The top of the `i`th row of layers.
    pub(super) fn row_top(&self, i: usize) -> f32 {
        self.rect.min.y + ROW * (i as f32 + 1.0)
    }

    /// The row of layers under `p`; none on the ruler, which covers the rows scrolled under it.
    pub(super) fn row_at(&self, p: Pos2) -> Option<usize> {
        if p.y < self.ruler_top + ROW {
            return None;
        }
        usize::try_from(((p.y - self.rect.min.y) / ROW).floor() as i64 - 1).ok()
    }

    /// Whether `p` is in the name column rather than on the frames.
    pub(super) fn on_labels(&self, p: Pos2) -> bool {
        p.x < self.label_edge
    }

    /// The frame under `x`, fractional, so a caller can tell an edge from a middle.
    pub(super) fn frame_at(&self, x: f32) -> f32 {
        (x - self.track_left) / self.per_frame
    }

    /// Where frame `frame` begins.
    pub(super) fn frame_x(&self, frame: f32) -> f32 {
        self.track_left + frame * self.per_frame
    }
}

/// An end of the onion-skin range, as its marker on the ruler.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum OnionEnd {
    Before,
    After,
}

/// How near a marker a press must be, in points, to take it.
const MARKER_REACH: f32 = 5.0;

impl Layout {
    /// Where the onion-skin markers stand on the ruler: the left edge of the earliest ghost and
    /// the right edge of the latest, around `frame`.
    pub(super) fn onion_markers(&self, frame: u32, before: u32, after: u32) -> (f32, f32) {
        let first = frame.saturating_sub(before);
        let last = frame.saturating_add(after).saturating_add(1);
        (self.frame_x(first as f32), self.frame_x(last as f32))
    }

    /// The marker a press at `p` takes, if it is on the ruler near one.
    pub(super) fn onion_marker_at(
        &self,
        p: Pos2,
        frame: u32,
        before: u32,
        after: u32,
    ) -> Option<OnionEnd> {
        if p.y < self.ruler_top || p.y >= self.ruler_top + ROW || self.on_labels(p) {
            return None;
        }
        let (start, end) = self.onion_markers(frame, before, after);
        let (to_start, to_end) = ((p.x - start).abs(), (p.x - end).abs());
        if to_start <= MARKER_REACH && to_start <= to_end {
            Some(OnionEnd::Before)
        } else if to_end <= MARKER_REACH {
            Some(OnionEnd::After)
        } else {
            None
        }
    }

    /// The range an end dragged to `x` makes, kept within `max` a side.
    pub(super) fn onion_dragged(
        &self,
        end: OnionEnd,
        x: f32,
        frame: u32,
        (before, after): (u32, u32),
        max: u32,
    ) -> (u32, u32) {
        let at = self.frame_at(x).round().max(0.0) as i64;
        let frame = i64::from(frame);
        let clamp = |n: i64| u32::try_from(n.clamp(0, i64::from(max))).unwrap_or(0);
        match end {
            // The start marker is the left edge of the earliest ghost.
            OnionEnd::Before => (clamp(frame - at), after),
            // The end marker is the right edge of the latest.
            OnionEnd::After => (before, clamp(at - frame - 1)),
        }
    }
}

impl Workspace {
    pub(super) fn timeline(&mut self, ui: &mut egui::Ui) {
        self.timeline_toolbar(ui);
        ui.separator();

        let available = ui.available_width();
        if ui.rect_contains_pointer(ui.max_rect()) {
            let zoom = ui.input(|i| i.zoom_delta());
            if zoom != 1.0 {
                self.zoom_timeline(zoom);
            }
        }
        egui::ScrollArea::both().auto_shrink(false).show(ui, |ui| {
            let frames = self.scene_frames();
            // The rows shown, with how deep each is in folders; a collapsed folder hides its layers.
            let rows: Vec<(usize, usize)> = self
                .document()
                .scenes()
                .first()
                .map(|s| {
                    timeline_view::visible_layers(s.timeline().layers(), &self.collapsed_folders)
                })
                .unwrap_or_default();
            let height = ROW * (rows.len() as f32 + 1.0);
            let label_w = self.display.timeline_names;
            let track_width = (available - label_w).max(0.0);
            if std::mem::take(&mut self.timeline_fit) {
                self.timeline_zoom = timeline_view::fit_zoom(track_width, u64::from(frames));
            }
            let per_frame = timeline_view::frame_width(self.timeline_zoom);
            let shown = self.timeline_shown(track_width, per_frame);
            let (rect, response) = ui.allocate_exact_size(
                Vec2::new(label_w + per_frame * shown as f32, height),
                Sense::click_and_drag(),
            );
            let painter = ui.painter_at(rect);
            let layout = Layout::new(rect, painter.clip_rect(), per_frame, label_w);
            self.timeline_layout = Some(layout);
            self.names_edge(ui, &layout);
            self.timeline_input(ui, &response, &layout, &rows, shown);
            // The frames first; the names and the ruler go over what has scrolled under them.
            self.timeline_rows(&painter, &layout, &rows, shown);
            self.timeline_labels(ui, &painter, &layout, &rows);
            self.timeline_ruler(&painter, &layout, shown);
            self.layer_name_editor(
                ui,
                &rows,
                layout.row_top(0),
                ROW,
                layout.labels_left + 4.0,
                layout.label_edge - 56.0,
            );
            let head = layout.frame_x(self.frame as f32 + 0.5);
            if head >= layout.label_edge {
                let red = Color32::from_rgb(232, 69, 60);
                let top = layout.ruler_top + 8.0;
                painter.line_segment(
                    [Pos2::new(head, top), Pos2::new(head, rect.max.y)],
                    Stroke::new(1.5, red),
                );
                flare_ui::icons::image("playhead", 16.0, red).paint_at(
                    ui,
                    Rect::from_center_size(Pos2::new(head, top), Vec2::splat(16.0)),
                );
            }
        });
    }

    /// How many frames the timeline has cells for, `track_width` points wide at `per_frame`
    /// points a frame. Empty cells go on to the panel's edge, as Animate's do, past the scene's
    /// last frame, and on to the end of the longest sound, so all of it can be seen and its frames
    /// clicked (and given frames with F5).
    pub(super) fn timeline_shown(&self, track_width: f32, per_frame: f32) -> u32 {
        let sounds_end = self.document().scenes().first().map_or(0, |s| {
            timeline_view::sounds_end(self.document(), s.timeline())
        });
        let content = u64::from(self.scene_frames()).max(sounds_end);
        u32::try_from(timeline_view::shown_frames(track_width, per_frame, content))
            .unwrap_or(u32::MAX)
    }

    /// The layer on the `i`th row shown.
    fn row_layer(&self, rows: &[(usize, usize)], i: usize) -> Option<LayerId> {
        let layers = self.document().scenes().first()?.timeline().layers();
        rows.get(i)
            .and_then(|(layer, _)| layers.get(*layer))
            .map(Layer::id)
    }

    /// What a click, drag or right click on the timeline does.
    fn timeline_input(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        l: &Layout,
        rows: &[(usize, usize)],
        shown: u32,
    ) {
        // The loop bar and the onion-skin markers on the ruler come first: a press on one sets
        // its range, not the playhead.
        if self.loop_bar_input(ui, response, l)
            || self.multi_bar_input(ui, response, l)
            || self.onion_marker_drag(ui, response, l)
        {
            return;
        }
        if response.drag_started()
            && let Some(p) = ui.input(|i| i.pointer.press_origin())
            && !l.on_labels(p)
            && let Some(id) = l.row_at(p).and_then(|i| self.row_layer(rows, i))
            && self.begin_span_drag(id, l.frame_at(p.x), 6.0 / l.per_frame)
        {
            self.select_layer(id);
        }
        if self.span_drag.is_some() {
            if response.dragged()
                && let Some(p) = response.interact_pointer_pos()
            {
                self.update_span_drag(l.frame_at(p.x).floor().max(0.0) as u32);
            }
            if response.drag_stopped() {
                self.end_span_drag();
            }
        } else if (response.clicked() || response.dragged())
            && let Some(p) = response.interact_pointer_pos()
        {
            let in_track = !l.on_labels(p);
            let f = l.frame_at(p.x).floor().max(0.0) as u32;
            let extend = response.clicked() && in_track && ui.input(|i| i.modifiers.shift);
            // Clicking a row (below the ruler) makes that layer the active one.
            let picked_row = l.row_at(p).and_then(|i| rows.get(i).copied());
            let picked = l.row_at(p).and_then(|i| self.row_layer(rows, i));
            if extend && let Some(id) = picked {
                // Shift-click selects the frames between the playhead (or the range's own
                // start) and the clicked one, without moving the playhead.
                let anchor = self
                    .frame_range
                    .filter(|r| r.layer == id)
                    .map_or(self.frame, |r| r.from);
                self.select_frames(id, anchor, f);
            } else if in_track {
                self.set_frame_in_view(f, shown);
                if response.clicked() {
                    self.frame_range = None;
                }
            }
            if let Some(id) = picked {
                self.label_click(response, l, p, id, picked_row.map_or(0, |(_, d)| d));
            }
        }
        // A right click in the layer column opens that layer's menu; one on the frames opens the
        // menu for frames, after picking the frame it is on unless it is inside the frames
        // already selected.
        if response.secondary_clicked() {
            let at = response.interact_pointer_pos();
            let under = at.and_then(|p| Some((p, self.row_layer(rows, l.row_at(p)?)?)));
            self.layer_menu_for = under.filter(|(p, _)| l.on_labels(*p)).map(|(_, id)| id);
            self.frame_menu_open = false;
            if let Some((p, id)) = under.filter(|(p, _)| !l.on_labels(*p)) {
                let f = l.frame_at(p.x).floor().max(0.0) as u32;
                let inside = self
                    .frame_range
                    .is_some_and(|r| r.layer == id && r.from <= f && f <= r.to);
                self.select_layer(id);
                if !inside {
                    self.frame_range = None;
                    self.set_frame_in_view(f, shown);
                }
                self.frame_menu_open = true;
            }
        }
        if let Some(layer) = self.layer_menu_for {
            response.context_menu(|ui| self.layer_menu(ui, layer));
        } else if self.frame_menu_open {
            response.context_menu(|ui| self.frame_menu(ui));
        }
    }

    /// Takes, moves and lets go of an onion-skin marker; true while one is held.
    fn onion_marker_drag(&mut self, ui: &egui::Ui, response: &egui::Response, l: &Layout) -> bool {
        let onion = self.onion;
        if response.drag_started()
            && onion.enabled
            && let Some(p) = ui.input(|i| i.pointer.press_origin())
        {
            self.onion_drag = l.onion_marker_at(p, self.frame, onion.before, onion.after);
        }
        let Some(end) = self.onion_drag else {
            return false;
        };
        if let Some(p) = response.interact_pointer_pos() {
            let range = (onion.before, onion.after);
            let (before, after) = l.onion_dragged(end, p.x, self.frame, range, MAX_ONION_FRAMES);
            self.set_onion_range(before, after);
        }
        if response.drag_stopped() || !response.dragged() {
            self.onion_drag = None;
        }
        true
    }

    /// A click on a layer's row: its folder arrow, its name, its toggles, or just the row.
    fn label_click(
        &mut self,
        response: &egui::Response,
        l: &Layout,
        p: Pos2,
        id: LayerId,
        depth: usize,
    ) {
        // The small toggles at the right edge of the name column.
        let edge = l.label_edge;
        let eye = p.x >= edge - 54.0 && p.x < edge - 36.0;
        let lock = p.x >= edge - 36.0 && p.x < edge - 18.0;
        let outline = p.x >= edge - 18.0 && p.x < edge;
        // The arrow of a folder, indented to its depth, folds it open or shut.
        let arrow_x = l.labels_left + 6.0 + depth as f32 * FOLDER_INDENT;
        let on_arrow = p.x >= arrow_x && p.x < arrow_x + 14.0;
        let is_folder = self.layer(id).is_some_and(Layer::is_folder);
        if response.clicked() && is_folder && on_arrow {
            if !self.collapsed_folders.remove(&id) {
                self.collapsed_folders.insert(id);
            }
            self.select_layer(id);
        } else if response.double_clicked() && layer_names::on_name(p.x, edge) {
            self.begin_layer_rename(id);
        } else if response.clicked() && eye {
            self.toggle_layer_visible(id);
        } else if response.clicked() && lock {
            self.toggle_layer_locked(id);
        } else if response.clicked() && outline {
            self.cycle_layer_outline(id);
        } else {
            self.select_layer(id);
        }
    }
}

#[cfg(test)]
#[path = "timeline_layout_tests.rs"]
mod tests;
