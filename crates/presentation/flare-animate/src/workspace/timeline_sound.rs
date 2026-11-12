//! A sound's bar on the timeline, with its waveform drawn in it so a person can see where the
//! words and beats fall and key the drawing to them.

use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Shape, Stroke, Vec2};
use flare_audio::Peaks;

/// The bar's fill, the colour Animate's sound layers have always had.
const FILL: Color32 = Color32::from_rgb(214, 140, 60);
/// The waveform, dark on the fill so it reads in both themes.
const WAVE: Color32 = Color32::from_rgb(92, 50, 14);

/// The shape of `peaks` across `bar`, one column a point, for only the columns that are on
/// screen (`clip`): the timeline repaints while playing, and a long sound runs far past the
/// panel's edge. `seconds_per_point` is how much of the sound one point covers. Each column is
/// a line from the lowest to the highest sample under it, about the bar's middle; silence is a
/// one-point line, so a quiet stretch still reads as part of the sound.
pub(super) fn waveform(
    bar: Rect,
    clip: Rect,
    peaks: &Peaks,
    seconds_per_point: f64,
    stroke: Stroke,
) -> Vec<Shape> {
    let visible = bar.intersect(clip);
    let usable_scale = seconds_per_point.is_finite() && seconds_per_point > 0.0;
    if !visible.is_positive() || !usable_scale {
        return Vec::new();
    }
    let middle = bar.center().y;
    let half = (bar.height() / 2.0 - 1.0).max(0.5);
    let first = (visible.min.x - bar.min.x).floor().max(0.0) as u32;
    let last = (visible.max.x - bar.min.x).ceil().max(0.0) as u32;
    (first..last)
        .filter_map(|column| {
            let from = f64::from(column) * seconds_per_point;
            let [low, high] = peaks.between(from, from + seconds_per_point)?;
            let x = bar.min.x + column as f32 + 0.5;
            let top = middle - high * half;
            let bottom = (middle - low * half).max(top + 1.0);
            Some(Shape::line_segment(
                [Pos2::new(x, top), Pos2::new(x, bottom)],
                stroke,
            ))
        })
        .collect()
}

/// Paints a sound's bar: the fill, the waveform once the sound is decoded, and its name.
pub(super) fn sound_bar(
    painter: &Painter,
    bar: Rect,
    name: &str,
    peaks: Option<&Peaks>,
    seconds_per_point: f64,
) {
    painter.rect_filled(bar, 2.0, FILL);
    if let Some(peaks) = peaks {
        painter.extend(waveform(
            bar.shrink2(Vec2::new(0.0, 1.0)),
            painter.clip_rect(),
            peaks,
            seconds_per_point,
            Stroke::new(1.0, WAVE),
        ));
    }
    painter.text(
        bar.left_center() + Vec2::new(3.0, 0.0),
        Align2::LEFT_CENTER,
        name,
        FontId::proportional(10.0),
        Color32::BLACK,
    );
}

#[cfg(test)]
#[path = "timeline_sound_tests.rs"]
mod tests;
