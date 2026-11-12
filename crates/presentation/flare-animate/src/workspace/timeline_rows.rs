//! The timeline's rows of frames: the cells, each layer's spans and sounds, the frames selected,
//! and the span being dragged.

use egui::{Color32, Painter, Pos2, Rect, Stroke, StrokeKind, Vec2};
use flare_editor::timeline_view;

use super::timeline::{Layout, ROW};
use super::*;
use super::{timeline_cells, timeline_sound};

/// The selected frames, Animate's blue.
pub(super) const SELECTED_CELL: Color32 = Color32::from_rgb(64, 140, 240);

/// The tint over the active layer's row.
pub(super) fn active_row() -> Color32 {
    Color32::from_rgba_unmultiplied(80, 140, 255, 40)
}

/// The tint over every other row: the lit text color, faintly, so white on dark and black on light.
pub(super) fn stripe() -> Color32 {
    flare_ui::theme::text_strong().gamma_multiply(6.0 / 255.0)
}

impl Workspace {
    /// Paints the rows of frames in view. Only those: with thousands of layers, drawing all of
    /// them every frame dominated the cost of an idle frame.
    pub(super) fn timeline_rows(
        &self,
        painter: &Painter,
        l: &Layout,
        rows: &[(usize, usize)],
        shown: u32,
    ) {
        let Some(scene) = self.document().scenes().first() else {
            return;
        };
        let rect = l.rect;
        let active = self.active_layer_id();
        let per_second = self.document().frame_rate().per_second();
        let (first, count) = visible_rows(rect.min.y, ROW, painter.clip_rect());
        let all_layers = scene.timeline().layers();
        for (i, (layer_index, _)) in rows.iter().copied().enumerate().skip(first).take(count) {
            let Some(layer) = all_layers.get(layer_index) else {
                continue;
            };
            let y = l.row_top(i);
            let band = Rect::from_min_max(Pos2::new(rect.min.x, y), Pos2::new(rect.max.x, y + ROW));
            if i % 2 == 1 {
                painter.rect_filled(band, 0.0, stripe());
            }
            timeline_cells::grid_lines(painter, l.track_left, y, ROW, l.per_frame, shown);
            if Some(layer.id()) == active {
                painter.rect_filled(band, 0.0, active_row());
            }
            for span in layer.spans() {
                let x0 = l.frame_x(span.start() as f32);
                let x1 = l.frame_x(span.end() as f32);
                let [r, g, b] = timeline_view::span_color(span.content(), span.tween());
                let color = Color32::from_rgb(r, g, b);
                let bar = Rect::from_min_max(
                    Pos2::new(x0, y + 1.0),
                    Pos2::new(x1.max(x0 + 1.0), y + ROW - 1.0),
                );
                let color = if layer.is_visible() {
                    color
                } else {
                    color.gamma_multiply(0.35)
                };
                painter.rect_filled(bar, 0.0, color);
                timeline_cells::span_marks(painter, bar, span, l.per_frame);
                // A tween has an arrow along its span, toward the next keyframe.
                if span.tween() != flare_document::Tween::None && bar.width() > 12.0 {
                    let line = Stroke::new(1.5, Color32::WHITE);
                    let (from, to) = (
                        bar.left_center() + Vec2::new(4.0, 0.0),
                        bar.right_center() - Vec2::new(4.0, 0.0),
                    );
                    painter.line_segment([from, to], line);
                    for tip in [Vec2::new(-4.0, -3.0), Vec2::new(-4.0, 3.0)] {
                        painter.line_segment([to, to + tip], line);
                    }
                }
            }
            for cue in layer.cues() {
                let Some(sound) = self.document().sound(cue.sound) else {
                    continue;
                };
                let length = timeline_view::sound_frames(sound.seconds(), per_second) as f32;
                let x0 = l.frame_x(cue.start as f32);
                let bar = Rect::from_min_max(
                    Pos2::new(x0, y + 2.0),
                    Pos2::new(x0 + length * l.per_frame, y + ROW - 2.0),
                );
                timeline_sound::sound_bar(
                    painter,
                    bar,
                    sound.name(),
                    self.sound_peaks(cue.sound),
                    1.0 / (per_second * f64::from(l.per_frame)),
                );
            }
            // The frames selected, or with none the cell under the playhead on the active layer,
            // in Animate's blue, so a click on any cell (an empty one too) shows where the next
            // frame command acts.
            let selected = match self.frame_range {
                Some(range) => (range.layer == layer.id()).then_some((range.from, range.to)),
                None => (Some(layer.id()) == active).then_some((self.frame, self.frame)),
            };
            if let Some((from, to)) = selected {
                let marked = Rect::from_min_max(
                    Pos2::new(l.frame_x(from as f32) + 1.0, y + 2.0),
                    Pos2::new(l.frame_x(to as f32 + 1.0) - 1.0, y + ROW - 2.0),
                );
                painter.rect_filled(marked, 1.0, SELECTED_CELL.gamma_multiply(0.85));
                painter.rect_stroke(
                    marked,
                    1.0,
                    Stroke::new(1.0, SELECTED_CELL),
                    StrokeKind::Inside,
                );
            }
            if let Some((dragged, start, duration)) = self.span_drag_preview()
                && dragged == layer.id()
            {
                let x0 = l.frame_x(start as f32);
                let ghost = Rect::from_min_max(
                    Pos2::new(x0, y + 2.0),
                    Pos2::new(x0 + duration as f32 * l.per_frame, y + ROW - 2.0),
                );
                painter.rect_stroke(
                    ghost,
                    2.0,
                    Stroke::new(2.0, Color32::from_rgb(40, 120, 255)),
                    StrokeKind::Inside,
                );
            }
        }
    }
}
