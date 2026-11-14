//! One look of the network: a square of the picture brought to 192 by 192 pixels, and the
//! seventeen points it answers with, put back in the picture's pixels.

use rten::Model;
use rten_tensor::NdTensor;
use rten_tensor::prelude::*;

use crate::{Pixels, PoseError};

/// The side of the square the network looks at.
pub(crate) const SIDE: usize = 192;
/// How many points the network finds: nose, eyes, ears, then shoulders, elbows, wrists, hips,
/// knees and ankles, the person's left before their right.
pub(crate) const POINTS: usize = 17;
/// Most samples taken across a pixel of the square each way, when the square covers many.
const MOST_SAMPLES: usize = 4;

/// A square of the picture, in its pixels; it may reach past the picture's edges, which are
/// seen as black.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Window {
    pub(crate) left: f64,
    pub(crate) top: f64,
    pub(crate) side: f64,
}

impl Window {
    /// The whole picture, in the middle of a square as wide as its longest side.
    pub(crate) fn whole([w, h]: [u32; 2]) -> Self {
        let (w, h) = (f64::from(w), f64::from(h));
        let side = w.max(h);
        Self {
            left: (w - side) / 2.0,
            top: (h - side) / 2.0,
            side,
        }
    }
}

/// A point found: where, in the picture's pixels, and how sure (0 to 1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Point {
    pub(crate) at: [f64; 2],
    pub(crate) score: f64,
}

pub(crate) type Points = [Point; POINTS];

/// The colour at `(x, y)` of the picture, between pixels, against black.
fn sample(pixels: Pixels<'_>, x: f64, y: f64) -> [f64; 3] {
    let [w, h] = pixels.size;
    let (x, y) = (x - 0.5, y - 0.5);
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (x - x0, y - y0);
    let mut out = [0.0; 3];
    for (dy, wy) in [(0.0, 1.0 - fy), (1.0, fy)] {
        for (dx, wx) in [(0.0, 1.0 - fx), (1.0, fx)] {
            let (px, py) = (x0 + dx, y0 + dy);
            if px < 0.0 || py < 0.0 || px >= f64::from(w) || py >= f64::from(h) {
                continue;
            }
            let i = (py as usize * w as usize + px as usize) * 4;
            let alpha = f64::from(pixels.rgba[i + 3]) / 255.0;
            for (c, v) in out.iter_mut().enumerate() {
                *v += f64::from(pixels.rgba[i + c]) * alpha * wx * wy;
            }
        }
    }
    out
}

/// The square `window` as the network takes it: 192 by 192 colours from 0 to 255, each the
/// average of the picture under it.
fn square(pixels: Pixels<'_>, window: Window) -> NdTensor<i32, 4> {
    let step = window.side / SIDE as f64;
    let n = (step.ceil() as usize).clamp(1, MOST_SAMPLES);
    let mut input = NdTensor::<i32, 4>::zeros([1, SIDE, SIDE, 3]);
    for row in 0..SIDE {
        for column in 0..SIDE {
            let mut sum = [0.0; 3];
            for sy in 0..n {
                for sx in 0..n {
                    let x = window.left + (column as f64 + (sx as f64 + 0.5) / n as f64) * step;
                    let y = window.top + (row as f64 + (sy as f64 + 0.5) / n as f64) * step;
                    let c = sample(pixels, x, y);
                    for k in 0..3 {
                        sum[k] += c[k];
                    }
                }
            }
            for (k, total) in sum.iter().enumerate() {
                input[[0, row, column, k]] =
                    (total / (n * n) as f64).round().clamp(0.0, 255.0) as i32;
            }
        }
    }
    input
}

/// Looks at `window` of the picture: where each of the seventeen points is, in its pixels.
pub(crate) fn run(model: &Model, pixels: Pixels<'_>, window: Window) -> Result<Points, PoseError> {
    let input = square(pixels, window);
    let output = model
        .run_one(input.view().into(), None)
        .map_err(|e| PoseError::Model(e.to_string()))?;
    let output: NdTensor<f32, 4> = output
        .try_into()
        .map_err(|_| PoseError::Model("the network answered with something else".into()))?;
    if output.shape() != [1, 1, POINTS, 3] {
        return Err(PoseError::Model(format!(
            "the network answered with {:?} numbers",
            output.shape()
        )));
    }
    let mut points = [Point {
        at: [0.0; 2],
        score: 0.0,
    }; POINTS];
    for (k, point) in points.iter_mut().enumerate() {
        // The network gives each point as y, x and a score, the square's sides from 0 to 1.
        let (y, x, score) = (
            f64::from(output[[0, 0, k, 0]]),
            f64::from(output[[0, 0, k, 1]]),
            f64::from(output[[0, 0, k, 2]]),
        );
        if !(x.is_finite() && y.is_finite() && score.is_finite()) {
            continue;
        }
        *point = Point {
            at: [window.left + x * window.side, window.top + y * window.side],
            score: score.clamp(0.0, 1.0),
        };
    }
    Ok(points)
}
