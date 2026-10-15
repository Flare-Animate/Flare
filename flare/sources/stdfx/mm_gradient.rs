//! Gradient fills and strokes, linear and radial, as XFL and SWF define them: a
//! ramp of colour stops across a square ±819.2 px each way (SWF's ±16384 twips),
//! which the gradient's matrix maps into its shape. A linear gradient runs along
//! the square's x axis; a radial one from its middle, or from a focal point on
//! the x axis, out to the circle touching the square's sides.

use tiny_skia as sk;

use crate::doc::{ColorTransform, Extra, Matrix, Paint, Point, Rgba, Shape};
use crate::hit::{self, Rect};

/// Half the gradient square's side, in pixels.
pub const HALF: f32 = 819.2;

#[derive(Clone, Debug, PartialEq)]
pub struct Gradient {
    pub kind: GradientKind,
    /// From the gradient square to the shape's coordinates.
    pub matrix: Matrix,
    /// (ratio 0..=1, colour), in order of ratio.
    pub stops: Vec<(f32, Rgba)>,
    pub spread: Spread,
    /// Interpolates in linear light rather than between sRGB values.
    pub linear_rgb: bool,
    /// What the model doesn't read of the XFL element.
    pub extra: Extra,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GradientKind {
    Linear,
    /// `focal` (-1..=1) puts the focal point on the x axis, from the middle to the edge.
    Radial { focal: f32 },
}

/// What lies past the ramp's ends: its end colours (Animate's Extend), the ramp mirrored, or repeated.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Spread {
    Pad,
    Reflect,
    Repeat,
}

impl Gradient {
    /// A gradient as Animate starts one: black to white, across the gradient square.
    pub fn new(kind: GradientKind) -> Gradient {
        let stops = vec![(0.0, [0.0, 0.0, 0.0, 1.0]), (1.0, [1.0; 4])];
        Gradient { kind, matrix: Matrix::IDENTITY, stops, spread: Spread::Pad, linear_rgb: false, extra: Extra::default() }
    }

    /// Spans the box `b`, in its shape's coordinates: a linear gradient runs across
    /// it, a radial one is the circle as wide as its longer side.
    // ponytail: how Animate fits a fresh fill is from recall; compare the matrices it saves.
    pub fn fit(&mut self, b: Rect) {
        let (w, h) = ((b.max.0 - b.min.0).max(1.0), (b.max.1 - b.min.1).max(1.0));
        let (sx, sy) = match self.kind {
            GradientKind::Linear => (w, h),
            GradientKind::Radial { .. } => (w.max(h), w.max(h)),
        };
        let (cx, cy) = ((b.min.0 + b.max.0) / 2.0, (b.min.1 + b.max.1) / 2.0);
        self.matrix = Matrix { a: sx / (2.0 * HALF), b: 0.0, c: 0.0, d: sy / (2.0 * HALF), tx: cx, ty: cy };
    }

    /// The ramp's colour at `t` (0..=1).
    pub fn color_at(&self, t: f32) -> Rgba {
        let (Some(first), Some(last)) = (self.stops.first(), self.stops.last()) else { return [0.0; 4] };
        if t <= first.0 {
            return first.1;
        }
        let Some(i) = self.stops.iter().position(|s| s.0 > t) else { return last.1 };
        let ((t0, a), (t1, b)) = (self.stops[i - 1], self.stops[i]);
        let k = if t1 > t0 { (t - t0) / (t1 - t0) } else { 1.0 };
        mix(a, b, k, self.linear_rgb)
    }

    /// The colour drawn at `p`, in the shape's coordinates.
    pub fn color_at_point(&self, p: Point) -> Rgba {
        let Some(inverse) = self.matrix.inverse() else { return self.color_at(0.0) };
        let (x, y) = inverse.apply(p);
        let (x, y) = (x / HALF, y / HALF);
        let t = match self.kind {
            GradientKind::Linear => (x + 1.0) / 2.0,
            // From the focal point f along the ray through p, the share of the way to the circle.
            GradientKind::Radial { focal } => {
                let (dx, dy) = (x - focal, y);
                let (dd, fd) = (dx * dx + dy * dy, focal * dx);
                if dd == 0.0 { 0.0 } else { dd / (-fd + (fd * fd + dd * (1.0 - focal * focal)).max(0.0).sqrt()) }
            }
        };
        let t = match self.spread {
            Spread::Pad => t.clamp(0.0, 1.0),
            Spread::Repeat => t.rem_euclid(1.0),
            Spread::Reflect => 1.0 - (t.rem_euclid(2.0) - 1.0).abs(),
        };
        self.color_at(t)
    }
}

/// `a` to `b`, `k` of the way: in linear light if `linear`, the alpha always straight.
fn mix(a: Rgba, b: Rgba, k: f32, linear: bool) -> Rgba {
    let lerp = |x: f32, y: f32| x + (y - x) * k;
    std::array::from_fn(|c| if linear && c < 3 { to_srgb(lerp(to_linear(a[c]), to_linear(b[c]))) } else { lerp(a[c], b[c]) })
}

/// A linear-RGB ramp as stops tiny-skia can mix in sRGB, as it mixes every
/// gradient: each span halved until mixing its ends in sRGB lands within half a
/// level of mixing them in linear light. The blending stays sRGB's, as Flash's does.
fn linear_light(stops: &[(f32, Rgba)]) -> Vec<(f32, Rgba)> {
    fn span(a: (f32, Rgba), b: (f32, Rgba), depth: u32, out: &mut Vec<(f32, Rgba)>) {
        let mid = ((a.0 + b.0) / 2.0, mix(a.1, b.1, 0.5, true));
        let off = (0..3).any(|c| (mid.1[c] - (a.1[c] + b.1[c]) / 2.0).abs() > 0.5 / 255.0);
        if off && depth < 10 && b.0 > a.0 {
            span(a, mid, depth + 1, out);
            span(mid, b, depth + 1, out);
        } else {
            out.push(b);
        }
    }
    let mut out: Vec<_> = stops.first().copied().into_iter().collect();
    for w in stops.windows(2) {
        span(w[0], w[1], 0, &mut out);
    }
    out
}

fn to_linear(c: f32) -> f32 {
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

fn to_srgb(c: f32) -> f32 {
    if c <= 0.003_130_8 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 }
}

/// The box around `s`'s edges, in its own coordinates, strokes left out.
pub fn edge_box(s: &Shape) -> Option<Rect> {
    let points = s.edges.iter().flat_map(|e| &e.paths).flat_map(hit::polyline);
    points.map(|p| Rect { min: p, max: p }).reduce(Rect::union)
}

/// Fits each gradient of `s`, fill or stroke, to the box around its edges, as
/// Animate fills a new shape.
pub fn fit_all(s: &mut Shape) {
    let Some(b) = edge_box(s) else { return };
    for paint in s.fills.iter_mut().chain(s.strokes.iter_mut().map(|st| &mut st.paint)) {
        if let Paint::Gradient(g) = paint {
            g.fit(b);
        }
    }
}

/// The tiny-skia paint drawing `g`, its colours through `cx`.
pub(crate) fn sk_paint(g: &Gradient, cx: ColorTransform) -> Option<sk::Paint<'static>> {
    let color = |c: Rgba| {
        let [r, g, b, a] = cx.apply(c);
        sk::Color::from_rgba(r, g, b, a).unwrap_or(sk::Color::BLACK)
    };
    let stops = if g.linear_rgb { linear_light(&g.stops) } else { g.stops.clone() };
    let stops: Vec<_> = stops.iter().map(|&(t, c)| sk::GradientStop::new(t, color(c))).collect();
    let mode = match g.spread {
        Spread::Pad => sk::SpreadMode::Pad,
        Spread::Reflect => sk::SpreadMode::Reflect,
        Spread::Repeat => sk::SpreadMode::Repeat,
    };
    let m = g.matrix;
    let ts = sk::Transform::from_row(m.a, m.b, m.c, m.d, m.tx, m.ty);
    let shader = match g.kind {
        GradientKind::Linear => sk::LinearGradient::new(sk::Point::from_xy(-HALF, 0.0), sk::Point::from_xy(HALF, 0.0), stops, mode, ts),
        // A focal gradient is a two-point conical one: from the focal point, radius 0, to the circle.
        GradientKind::Radial { focal } => {
            sk::RadialGradient::new(sk::Point::from_xy(focal * HALF, 0.0), 0.0, sk::Point::zero(), HALF, stops, mode, ts)
        }
    }?;
    Some(sk::Paint { shader, ..Default::default() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_along_the_ramp_and_across_the_shape() {
        let mut g = Gradient::new(GradientKind::Linear);
        g.stops = vec![(0.0, [1.0, 0.0, 0.0, 1.0]), (0.5, [0.0, 0.0, 1.0, 1.0]), (1.0, [0.0, 0.0, 1.0, 0.0])];
        assert_eq!(g.color_at(-1.0), [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(g.color_at(0.25), [0.5, 0.0, 0.5, 1.0]);
        assert_eq!(g.color_at(0.75), [0.0, 0.0, 1.0, 0.5]);
        assert_eq!(g.color_at(2.0), [0.0, 0.0, 1.0, 0.0]);
        // Linear RGB: black to white is 50% light in the middle, which sRGB writes as about 188/255.
        let mut grey = Gradient::new(GradientKind::Linear);
        grey.linear_rgb = true;
        assert!((grey.color_at(0.5)[0] * 255.0 - 187.5).abs() < 1.0, "{:?}", grey.color_at(0.5));
        // Drawn as stops mixed in sRGB: a few dozen, within a level of the ramp.
        let baked = Gradient { stops: linear_light(&grey.stops), linear_rgb: false, ..grey.clone() };
        assert!(baked.stops.len() < 64, "{}", baked.stops.len());
        for t in (0..=200).map(|i| i as f32 / 200.0) {
            assert!((baked.color_at(t)[0] - grey.color_at(t)[0]).abs() * 255.0 < 1.0, "at {t}");
        }

        // Fitted to a 100 × 50 box at (10, 20): across it, and past its ends as the spread says.
        let b = Rect { min: (10.0, 20.0), max: (110.0, 70.0) };
        grey = Gradient::new(GradientKind::Linear);
        grey.fit(b);
        let at = |g: &Gradient, x: f32, y: f32| (g.color_at_point((x, y))[0] * 100.0).round() / 100.0;
        assert_eq!((at(&grey, 10.0, 0.0), at(&grey, 60.0, 45.0), at(&grey, 110.0, 99.0), at(&grey, 300.0, 45.0)), (0.0, 0.5, 1.0, 1.0));
        grey.spread = Spread::Reflect;
        assert_eq!((at(&grey, 135.0, 45.0), at(&grey, 210.0, 45.0)), (0.75, 0.0));
        grey.spread = Spread::Repeat;
        assert_eq!(at(&grey, 135.0, 45.0), 0.25);

        // A radial one is the circle as wide as the box's longer side, its focal point on the x axis.
        let mut radial = Gradient::new(GradientKind::Radial { focal: 0.0 });
        radial.fit(b);
        assert_eq!((at(&radial, 60.0, 45.0), at(&radial, 85.0, 45.0), at(&radial, 60.0, 95.0), at(&radial, 0.0, 0.0)), (0.0, 0.5, 1.0, 1.0));
        radial.kind = GradientKind::Radial { focal: 0.5 };
        assert_eq!((at(&radial, 85.0, 45.0), at(&radial, 110.0, 45.0), at(&radial, 97.5, 45.0), at(&radial, 10.0, 45.0)), (0.0, 1.0, 0.5, 1.0));
    }
}
