//! The sample stroke a brush is shown drawing in the brush picker, so each brush can be judged
//! by the mark it makes rather than by its name. It is drawn by the same library as a real
//! stroke, so the swatch is exactly what the brush paints.

use flare_input::{InputSample, PointerKind};

use super::{BrushStroke, StrokeSides};

/// Pen samples along a swatch.
const SWATCH_SAMPLES: usize = 64;

/// A pen stroke across a box `length` by `height` (in its own units, from the top left): a gentle
/// wave from left to right, pressed lightly at the start, hardest in the middle and lifting at the
/// end, so a swatch shows how the brush answers pressure and how it starts and ends.
pub fn swatch_samples(length: f64, height: f64) -> Vec<InputSample> {
    let (length, height) = (finite(length), finite(height));
    (0..SWATCH_SAMPLES)
        .filter_map(|k| {
            let t = k as f64 / (SWATCH_SAMPLES - 1) as f64;
            let x = t * length;
            let y = height / 2.0 - (t * std::f64::consts::TAU).sin() * height * 0.22;
            let pressure = (t * std::f64::consts::PI).sin().max(0.0).powf(0.7) as f32;
            InputSample::new(x, y, pressure.max(0.05), PointerKind::Pen)
        })
        .collect()
}

/// The pieces of `stroke` drawing the swatch across a `length` by `height` box.
pub fn swatch(stroke: &BrushStroke, length: f64, height: f64) -> Vec<StrokeSides> {
    stroke.pieces(&swatch_samples(length, height), 0.25)
}

fn finite(v: f64) -> f64 {
    if v.is_finite() {
        v.clamp(0.0, 10_000.0)
    } else {
        0.0
    }
}
