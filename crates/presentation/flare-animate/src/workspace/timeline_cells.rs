//! The frame cells of the timeline: a line between frames, a dot where a keyframe starts and a box
//! on the last frame a held keyframe covers, as Animate draws them, so frames can be told apart.

use egui::{Color32, Painter, Pos2, Rect, Stroke, StrokeKind, Vec2};
use flare_document::{Content, FrameSpan};

/// Narrower than this, a frame has no room for lines between frames.
const MIN_CELL: f32 = 5.0;

/// The frames, out of `frames`, whose columns touch the visible range `from_x..=to_x` when frame 0
/// starts at `track_x` and each is `per_frame` wide.
pub(super) fn visible_frames(
    from_x: f32,
    to_x: f32,
    track_x: f32,
    per_frame: f32,
    frames: u32,
) -> std::ops::Range<u32> {
    if !(per_frame > 0.0 && per_frame.is_finite()) || !from_x.is_finite() || !to_x.is_finite() {
        return 0..0;
    }
    let first = ((from_x - track_x) / per_frame).floor().max(0.0);
    let last = ((to_x - track_x) / per_frame).ceil().max(0.0);
    let (first, last) = (first.min(f32::from(u16::MAX) * 1e3), last.min(f32::MAX));
    (first as u32).min(frames)..(last as u32).min(frames)
}

/// The lines between the frames of one row of empty cells; every fifth is stronger so they can be
/// counted.
pub(super) fn grid_lines(
    painter: &Painter,
    track_x: f32,
    y: f32,
    row: f32,
    per_frame: f32,
    frames: u32,
) {
    let clip = painter.clip_rect();
    let ink = flare_ui::theme::text_strong();
    for f in visible_frames(clip.min.x, clip.max.x, track_x, per_frame, frames) {
        let fifth = f % 5 == 0;
        if !fifth && per_frame < MIN_CELL {
            continue;
        }
        let x = track_x + f as f32 * per_frame;
        let alpha = if fifth { 30.0 } else { 14.0 } / 255.0;
        painter.vline(x, y..=y + row, Stroke::new(1.0, ink.gamma_multiply(alpha)));
    }
}

/// The marks on a span's bar: a line between its frames, a dot on its first frame (hollow when the
/// keyframe holds nothing) and a box on its last when it holds more than one.
pub(super) fn span_marks(painter: &Painter, bar: Rect, span: &FrameSpan, per_frame: f32) {
    let line = Color32::from_black_alpha(80);
    let frames = span.duration().get();
    if per_frame >= MIN_CELL {
        let clip = painter.clip_rect();
        let shown = visible_frames(clip.min.x, clip.max.x, bar.min.x, per_frame, frames);
        for f in shown.filter(|f| *f > 0) {
            let x = bar.min.x + f as f32 * per_frame;
            painter.vline(x, bar.y_range(), Stroke::new(1.0, line));
        }
    }
    let middle = bar.center().y;
    let dot = Pos2::new(bar.min.x + per_frame.min(14.0) / 2.0, middle);
    if matches!(span.content(), Content::Empty) {
        painter.circle_stroke(dot, 2.5, Stroke::new(1.0, Color32::BLACK));
    } else {
        painter.circle_filled(dot, 2.5, Color32::BLACK);
    }
    if frames > 1 && per_frame >= MIN_CELL {
        let last = Rect::from_min_size(
            Pos2::new(bar.max.x - per_frame, bar.min.y),
            Vec2::new(per_frame, bar.height()),
        )
        .shrink2(Vec2::new(1.5, 3.0));
        painter.rect_stroke(
            last,
            0.0,
            Stroke::new(1.0, Color32::BLACK),
            StrokeKind::Inside,
        );
    }
}

/// A layer's name laid out on one line no wider than `max_width`, ending in an ellipsis when it is
/// cut, so a long name (a sound layer's file name) stops before the toggles beside it.
pub(super) fn layer_name(
    fonts: &mut egui::epaint::text::FontsView<'_>,
    name: String,
    max_width: f32,
    color: Color32,
) -> std::sync::Arc<egui::Galley> {
    let mut job =
        egui::text::LayoutJob::simple_singleline(name, egui::FontId::proportional(12.0), color);
    job.wrap = egui::text::TextWrapping::truncate_at_width(max_width.max(0.0));
    fonts.layout_job(job)
}

#[cfg(test)]
#[path = "timeline_cells_tests.rs"]
mod tests;
