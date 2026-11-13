//! Filling triangles into samples: a depth buffer for the body, a mask for its shadow, and the
//! samples brought down into a picture with an outline drawn where the body ends or a nearer
//! part stands in front of a further one.

use crate::render::Picture;

/// What each corner carries across its triangle: depth (larger is nearer), the lit colour, and
/// where it is on the guide lines (round, along, and the middle lines).
pub(crate) const VALUES: usize = 7;

pub(crate) struct Corner {
    pub x: f32,
    pub y: f32,
    pub v: [f32; VALUES],
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Lines {
    pub shown: bool,
}

/// The orange of the lines down the middle of the front and back.
const MIDDLE_LINE: [f32; 3] = [236.0, 128.0, 52.0];
/// The outline round the figure.
const OUTLINE: [f32; 3] = [70.0, 72.0, 80.0];
/// How dark the shadow on the floor is, from 0 to 1.
const SHADOW: f32 = 0.26;
/// A nearer part this far in front of the one beside it (in figure units) is outlined.
const DEPTH_STEP: f32 = 0.03;

pub(crate) struct Canvas {
    width: usize,
    height: usize,
    depth: Vec<f32>,
    color: Vec<[u8; 3]>,
    joint: Vec<u8>,
    shadow: Vec<bool>,
}

/// How much of a sample a line at the whole numbers of a value covers, given how much the value
/// changes from one sample to the next; lines fade where they would crowd together.
fn line(value: f32, step: f32) -> f32 {
    if step > 0.6 || step <= 0.0 {
        return 0.0;
    }
    let off = (value - value.round()).abs() / step;
    let fade = (1.0 - (step - 0.3) / 0.3).clamp(0.0, 1.0);
    (1.2 - off).clamp(0.0, 1.0) * fade
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [0, 1, 2].map(|k| a[k] + (b[k] - a[k]) * t)
}

/// The rows a triangle covers and, for a row through `py`, the samples of it inside.
fn rows(points: [[f32; 2]; 3], height: usize) -> std::ops::Range<usize> {
    let (lo, hi) = points.iter().fold((f32::MAX, f32::MIN), |(lo, hi), p| {
        (lo.min(p[1]), hi.max(p[1]))
    });
    let top = (lo - 0.5).ceil().max(0.0) as usize;
    let bottom = ((hi - 0.5).floor() + 1.0).clamp(0.0, height as f32) as usize;
    top..bottom
}

/// The first and last sample of the row through `py` inside the triangle wound `points`
/// (clockwise in the picture, whose y runs down), or none.
fn span(points: [[f32; 2]; 3], py: f32, width: usize) -> Option<(usize, usize)> {
    let (mut from, mut to) = (0.0f32, width as f32 - 1.0);
    for k in 0..3 {
        let (p, q) = (points[k], points[(k + 1) % 3]);
        // Inside where dx * (x - p.x) + dy * (y - p.y) is not negative.
        let (dx, dy) = (p[1] - q[1], q[0] - p[0]);
        let rest = dy * (py - p[1]);
        if dx > 1e-12 {
            from = from.max((p[0] - rest / dx - 0.5).ceil());
        } else if dx < -1e-12 {
            to = to.min((p[0] - rest / dx - 0.5).floor());
        } else if rest < 0.0 {
            return None;
        }
    }
    (from <= to).then_some((from as usize, to as usize))
}

impl Canvas {
    pub fn new(width: usize, height: usize) -> Self {
        let size = width * height;
        Self {
            width,
            height,
            depth: vec![f32::NEG_INFINITY; size],
            color: vec![[0; 3]; size],
            joint: vec![0; size],
            shadow: vec![false; size],
        }
    }

    /// Marks the shadow of a triangle, either way round.
    pub fn shade(&mut self, a: [f32; 2], b: [f32; 2], c: [f32; 2]) {
        let area = (b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1]);
        if !area.is_finite() || area.abs() < 1e-9 {
            return;
        }
        let points = if area > 0.0 { [a, b, c] } else { [a, c, b] };
        for y in rows(points, self.height) {
            if let Some((from, to)) = span(points, y as f32 + 0.5, self.width) {
                let row = y * self.width;
                self.shadow[row + from..=row + to].fill(true);
            }
        }
    }

    /// Fills a triangle of the body that faces the viewer; one facing away is behind the body.
    pub fn fill(
        &mut self,
        [a, b, c]: [&Corner; 3],
        joint: u8,
        tint: Option<[f32; 3]>,
        lines: Lines,
    ) {
        // The picture's y runs down, so a triangle wound counterclockwise in the figure's space
        // (its outside toward the viewer) has a negative area here.
        let area = (b.x - a.x) * (c.y - a.y) - (c.x - a.x) * (b.y - a.y);
        if area >= -1e-9 || !area.is_finite() {
            return;
        }
        let (b, c) = (c, b);
        let inv = -1.0 / area;
        // How each value changes for a step right and a step down.
        let slopes: [(f32, f32); VALUES] = std::array::from_fn(|k| {
            let (p, q) = (b.v[k] - a.v[k], c.v[k] - a.v[k]);
            (
                (p * (c.y - a.y) - q * (b.y - a.y)) * inv,
                (q * (b.x - a.x) - p * (c.x - a.x)) * inv,
            )
        });
        let steps: [f32; 3] =
            std::array::from_fn(|k| slopes[4 + k].0.abs() + slopes[4 + k].1.abs());
        let points = [[a.x, a.y], [b.x, b.y], [c.x, c.y]];
        for y in rows(points, self.height) {
            let py = y as f32 + 0.5;
            let Some((from, to)) = span(points, py, self.width) else {
                continue;
            };
            let (ox, oy) = (from as f32 + 0.5 - a.x, py - a.y);
            let mut v: [f32; VALUES] =
                std::array::from_fn(|k| a.v[k] + slopes[k].0 * ox + slopes[k].1 * oy);
            let row = y * self.width;
            for i in row + from..=row + to {
                if v[0] > self.depth[i] {
                    self.depth[i] = v[0];
                    let mut rgb = [v[1], v[2], v[3]];
                    if let Some(tint) = tint {
                        rgb = mix(rgb, tint, 0.4);
                    }
                    if lines.shown {
                        let middle = line(v[6], steps[2]);
                        let grid = line(v[4], steps[0]).max(line(v[5], steps[1]));
                        rgb = mix(rgb, rgb.map(|c| c * 0.55), grid * 0.75);
                        rgb = mix(rgb, MIDDLE_LINE, middle * 0.85);
                    }
                    self.color[i] = rgb.map(|c| c.clamp(0.0, 255.0) as u8);
                    self.joint[i] = joint;
                }
                for (value, slope) in v.iter_mut().zip(&slopes) {
                    *value += slope.0;
                }
            }
        }
    }

    /// Whether the body's sample at `i` is at its edge: beside nothing, or in front of the part
    /// beside it, `reach` samples away.
    fn edge(&self, i: usize, x: usize, y: usize, reach: usize) -> bool {
        let depth = self.depth[i] - DEPTH_STEP;
        let farther = |j: usize| self.joint[j] == 0 || self.depth[j] < depth;
        (1..=reach).any(|r| {
            (x < r || farther(i - r))
                || (x + r >= self.width || farther(i + r))
                || (y < r || farther(i - r * self.width))
                || (y + r >= self.height || farther(i + r * self.width))
        })
    }

    /// Lays the samples into a picture `width` by `height`, each pixel the average of `n` by
    /// `n` of them, the canvas's corner at pixel `at`.
    pub fn picture(&self, n: usize, at: [usize; 2], width: usize, height: usize) -> Picture {
        let mut rgba = vec![0u8; width * height * 4];
        let mut joints = vec![0u8; width * height];
        let whole = (n * n) as f32;
        for y in 0..self.height / n {
            for x in 0..self.width / n {
                let (mut sum, mut alpha) = ([0f32; 3], 0f32);
                let (mut nearest, mut joint) = (f32::NEG_INFINITY, 0);
                for dy in 0..n {
                    let sy = y * n + dy;
                    for sx in x * n..x * n + n {
                        let i = sy * self.width + sx;
                        if self.joint[i] != 0 {
                            let mut c = self.color[i].map(f32::from);
                            if self.edge(i, sx, sy, n.div_ceil(2)) {
                                c = mix(c, OUTLINE, 0.9);
                            }
                            for (s, c) in sum.iter_mut().zip(c) {
                                *s += c;
                            }
                            alpha += 1.0;
                            if self.depth[i] > nearest {
                                (nearest, joint) = (self.depth[i], self.joint[i]);
                            }
                        } else if self.shadow[i] {
                            alpha += SHADOW;
                        }
                    }
                }
                if alpha > 0.0 {
                    let pixel = (y + at[1]) * width + x + at[0];
                    let out = &mut rgba[pixel * 4..pixel * 4 + 4];
                    for (o, s) in out.iter_mut().zip(sum) {
                        *o = (s / alpha).clamp(0.0, 255.0) as u8;
                    }
                    out[3] = (alpha / whole * 255.0).round().clamp(0.0, 255.0) as u8;
                    joints[pixel] = joint;
                }
            }
        }
        Picture {
            width: width as u32,
            height: height as u32,
            rgba,
            joints,
        }
    }
}
