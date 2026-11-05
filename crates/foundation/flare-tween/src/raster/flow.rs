//! Coarse-to-fine Lucas-Kanade flow: for each pixel of the template, the offset into the moving
//! picture that matches it best, solved over a small window at every pyramid level.

use super::FlowParams;
use super::plane::{Plane, box_sum};

/// One offset per pixel, in pixels of the level it belongs to: `x` and `y`.
pub(super) type Flow = Vec<[f32; 2]>;

/// Keeps the 2x2 system well posed where the picture is flat (its gradients vanish there).
const REGULARISE: f32 = 1.0;
/// Largest correction one Gauss-Newton step may make, in pixels of its level.
const MAX_STEP: f32 = 2.0;

/// The flow that maps `template` onto `moving`: `template(p)` is close to `moving(p + flow(p))`.
/// Both pyramids come from the same-sized rasters, finest first.
pub(super) fn estimate(template: &[Plane], moving: &[Plane], params: FlowParams) -> Flow {
    let mut flow: Flow = Vec::new();
    let mut previous = (0, 0);
    let window = params.window as usize;
    for (level, (t, m)) in template.iter().zip(moving).enumerate().rev() {
        let (width, height) = (t.width, t.height);
        flow = if level + 1 == template.len() {
            vec![[0.0, 0.0]; width * height]
        } else {
            upsample(&flow, previous, (width, height))
        };
        for _ in 0..params.iterations {
            refine(t, m, &mut flow, window);
        }
        previous = (width, height);
    }
    flow
}

/// Doubles a coarse flow onto the next finer grid: the offsets are in pixels, so they double too.
fn upsample(coarse: &[[f32; 2]], (cw, ch): (usize, usize), (fw, fh): (usize, usize)) -> Flow {
    let scale_x = cw as f32 / fw as f32;
    let scale_y = ch as f32 / fh as f32;
    let gain_x = fw as f32 / cw as f32;
    let gain_y = fh as f32 / ch as f32;
    let mut out = Vec::with_capacity(fw * fh);
    for y in 0..fh {
        for x in 0..fw {
            let cx = ((x as f32 + 0.5) * scale_x - 0.5).clamp(0.0, (cw - 1) as f32);
            let cy = ((y as f32 + 0.5) * scale_y - 0.5).clamp(0.0, (ch - 1) as f32);
            let [dx, dy] = sample_flow(coarse, cw, ch, cx, cy);
            out.push([dx * gain_x, dy * gain_y]);
        }
    }
    out
}

/// Bilinear sample of a flow at a position in pixels, clamped to the grid.
pub(super) fn sample_flow(
    flow: &[[f32; 2]],
    width: usize,
    height: usize,
    x: f32,
    y: f32,
) -> [f32; 2] {
    let x = x.clamp(0.0, (width - 1) as f32);
    let y = y.clamp(0.0, (height - 1) as f32);
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (x - x0, y - y0);
    let (x0, y0) = (x0 as usize, y0 as usize);
    let x1 = (x0 + 1).min(width - 1);
    let y1 = (y0 + 1).min(height - 1);
    let at = |xx: usize, yy: usize| flow[yy * width + xx];
    let (p00, p10, p01, p11) = (at(x0, y0), at(x1, y0), at(x0, y1), at(x1, y1));
    let mut out = [0.0; 2];
    for k in 0..2 {
        let top = p00[k] + (p10[k] - p00[k]) * fx;
        let bottom = p01[k] + (p11[k] - p01[k]) * fx;
        out[k] = top + (bottom - top) * fy;
    }
    out
}

/// One Gauss-Newton step over every pixel: the warped moving picture's gradient and residual
/// against the template give the 2x2 normal equations of a window, solved for the correction.
fn refine(template: &Plane, moving: &Plane, flow: &mut Flow, window: usize) {
    let (width, height) = (template.width, template.height);
    let count = width * height;
    let mut warped = vec![0.0f32; count];
    for (i, w) in warped.iter_mut().enumerate() {
        let (x, y) = ((i % width) as f32, (i / width) as f32);
        let [dx, dy] = flow[i];
        *w = moving.sample(x + dx, y + dy);
    }
    let mut xx = vec![0.0f32; count];
    let mut xy = vec![0.0f32; count];
    let mut yy = vec![0.0f32; count];
    let mut xt = vec![0.0f32; count];
    let mut yt = vec![0.0f32; count];
    for y in 0..height {
        for x in 0..width {
            let (xi, yi) = (x as isize, y as isize);
            let gx = 0.5
                * (sample_warped(&warped, width, height, xi + 1, yi)
                    - sample_warped(&warped, width, height, xi - 1, yi));
            let gy = 0.5
                * (sample_warped(&warped, width, height, xi, yi + 1)
                    - sample_warped(&warped, width, height, xi, yi - 1));
            let residual = warped[y * width + x] - template.data[y * width + x];
            let i = y * width + x;
            xx[i] = gx * gx;
            xy[i] = gx * gy;
            yy[i] = gy * gy;
            xt[i] = gx * residual;
            yt[i] = gy * residual;
        }
    }
    let mut scratch = vec![0.0f32; count];
    for sums in [&mut xx, &mut xy, &mut yy, &mut xt, &mut yt] {
        box_sum(sums, width, height, window, &mut scratch);
    }
    for i in 0..count {
        let a = xx[i] + REGULARISE;
        let d = yy[i] + REGULARISE;
        let det = a * d - xy[i] * xy[i];
        if det <= f32::EPSILON {
            continue;
        }
        let step_x = -(d * xt[i] - xy[i] * yt[i]) / det;
        let step_y = -(a * yt[i] - xy[i] * xt[i]) / det;
        let bound = (width.max(height)) as f32;
        flow[i][0] = (flow[i][0] + step_x.clamp(-MAX_STEP, MAX_STEP)).clamp(-bound, bound);
        flow[i][1] = (flow[i][1] + step_y.clamp(-MAX_STEP, MAX_STEP)).clamp(-bound, bound);
    }
}

fn sample_warped(warped: &[f32], width: usize, height: usize, x: isize, y: isize) -> f32 {
    let x = x.clamp(0, width as isize - 1) as usize;
    let y = y.clamp(0, height as isize - 1) as usize;
    warped[y * width + x]
}
