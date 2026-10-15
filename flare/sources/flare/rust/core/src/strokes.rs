//! Artistic stroke styles (Properties ▸ Stroke ▸ Style). Ragged, Stipple and
//! Hatched are Animate's; Herringbone … Curl are ported from OpenToonz's
//! `colorfx/strokestyles.cpp` (BSD-3-Clause, © DWANGO and OpenToonz
//! contributors; see THIRD-PARTY-NOTICES.md). OpenToonz draws them with
//! OpenGL each frame; here the same constructions produce plain geometry at
//! evaluation time, so every renderer and exporter draws them the same way.
//! Randomness is seeded by the path, so a stroke looks the same on every frame.

use crate::geom::{pt, Path, Pt, Seg};
use crate::model::{Color, Dash, Stroke, StrokeStyle};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f32 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        ((self.0.wrapping_mul(0x2545F4914F6CDD1D) >> 40) as f32) / (1u64 << 24) as f32
    }
    /// −1 … 1
    fn sym(&mut self) -> f32 {
        self.next() * 2.0 - 1.0
    }
}

fn seed(path: &Path) -> u64 {
    let mut h: u64 = 0x9E3779B97F4A7C15;
    for s in &path.segs {
        if let Seg::M(p) | Seg::L(p) = s {
            h = (h ^ (p.x.to_bits() as u64) ^ ((p.y.to_bits() as u64) << 32)).wrapping_mul(0x100000001B3);
        }
    }
    h | 1
}

/// Points every `step` along a polyline, with the unit normal there.
fn samples(pts: &[Pt], closed: bool, step: f32) -> Vec<(Pt, Pt)> {
    let mut ring = pts.to_vec();
    if closed && pts.len() > 2 {
        ring.push(pts[0]);
    }
    let mut out = vec![];
    let mut carry = 0.0;
    for w in ring.windows(2) {
        let (a, b) = (w[0], w[1]);
        let len = a.dist(b);
        if len < 1e-6 {
            continue;
        }
        let (dx, dy) = ((b.x - a.x) / len, (b.y - a.y) / len);
        let normal = pt(-dy, dx);
        let mut d = carry;
        while d <= len {
            out.push((a.lerp(b, d / len), normal));
            d += step;
        }
        carry = d - len;
    }
    out
}

/// Geometry for an artistic stroke: (path, fill, stroke) parts.
pub fn artistic(path: &Path, stroke: Stroke, dash: Dash) -> Vec<(Path, Option<Color>, Option<Stroke>)> {
    let w = stroke.width.max(0.5);
    let mut rng = Rng(seed(path));
    let mut out = vec![];
    for (pts, closed) in path.flatten(12) {
        if pts.len() < 2 {
            continue;
        }
        match dash {
            Dash::Ragged => {
                // a wobbly line: points pushed sideways by up to 60% of the width
                let mut s = samples(&pts, closed, (w * 1.2).max(1.5));
                if !closed {
                    // finish at the line's end
                    let (a, b) = (pts[pts.len() - 2], pts[pts.len() - 1]);
                    let l = a.dist(b).max(1e-6);
                    s.push((b, pt(-(b.y - a.y) / l, (b.x - a.x) / l)));
                }
                let mut segs = vec![];
                for (i, (p, n)) in s.iter().enumerate() {
                    let k = rng.sym() * w * 0.6;
                    let q = pt(p.x + n.x * k, p.y + n.y * k);
                    segs.push(if i == 0 { Seg::M(q) } else { Seg::L(q) });
                }
                if closed {
                    segs.push(Seg::Z);
                }
                out.push((Path { segs }, None, Some(Stroke { width: w * 0.8, ..stroke })));
            }
            Dash::Stipple => {
                // dots scattered across the stroke's width
                let mut segs = vec![];
                // (medium density, small-to-medium dots; at least half a pixel)
                for (p, n) in samples(&pts, closed, (w * 0.3).max(0.6)) {
                    for _ in 0..2 {
                        let k = rng.sym() * w * 0.45;
                        let along = rng.sym() * w * 0.2;
                        let c = pt(p.x + n.x * k - n.y * along, p.y + n.y * k + n.x * along);
                        let r = (w * (0.1 + rng.next() * 0.12)).max(0.5);
                        segs.extend(Path::ellipse(c.x, c.y, r, r).segs);
                    }
                }
                out.push((Path { segs }, Some(stroke.color), None));
            }
            Dash::Hatched => {
                // short strokes across the line, slightly jiggled
                let mut segs = vec![];
                for (p, n) in samples(&pts, closed, (w * 0.9).max(1.5)) {
                    let tilt = rng.sym() * 0.25;
                    let (nx, ny) = (n.x - n.y * tilt, n.y + n.x * tilt);
                    let h = w * 0.5 * (0.85 + rng.next() * 0.3);
                    segs.push(Seg::M(pt(p.x - nx * h, p.y - ny * h)));
                    segs.push(Seg::L(pt(p.x + nx * h, p.y + ny * h)));
                }
                out.push((Path { segs }, None, Some(Stroke { width: (w * 0.22).max(0.4), ..stroke })));
            }
            _ => {}
        }
    }
    out
}

/// Is this one of the generated styles?
pub fn is_artistic(d: Dash) -> bool {
    !matches!(d, Dash::Solid | Dash::Dashed | Dash::Dotted)
}

/// A style parameter: name, range, default.
#[derive(Clone, Copy, Debug)]
pub struct Param {
    pub name: &'static str,
    pub min: f32,
    pub max: f32,
    pub default: f32,
}
const fn prm(name: &'static str, min: f32, max: f32, default: f32) -> Param {
    Param { name, min, max, default }
}

/// The parameters of a style (OpenToonz's names, ranges and defaults).
pub fn params(d: Dash) -> &'static [Param] {
    const HERRING: [Param; 2] = [prm("Angle", 0.0, 180.0, 120.0), prm("Size", 0.0, 1.0, 1.0)];
    const CIRCLETS: [Param; 3] = [prm("Border fade", 0.0, 1.0, 0.5), prm("Density", 0.0, 100.0, 10.0), prm("Size", 0.0, 1.0, 0.3)];
    const SCRATCHY: [Param; 1] = [prm("Density", 0.0, 10.0, 10.0)];
    const ROPE: [Param; 1] = [prm("Tilt", -1.0, 1.0, 0.4)];
    const TULLE: [Param; 2] = [prm("Crease", 1.0, 100.0, 10.0), prm("Opacity", 0.0, 1.0, 0.5)];
    const PLAIT: [Param; 1] = [prm("Twirl", 1.0, 100.0, 80.0)];
    const FUZZ: [Param; 1] = [prm("Density", 0.0, 1.0, 0.4)];
    const JAGGED: [Param; 1] = [prm("Distance", 0.1, 100.0, 20.0)];
    const ZIGZAG: [Param; 5] = [prm("Min distance", 0.5, 50.0, 0.5), prm("Max distance", 0.5, 50.0, 6.0), prm("Min angle", -90.0, 90.0, 30.0), prm("Max angle", -90.0, 90.0, 60.0), prm("Thickness", 0.0, 3.0, 1.0)];
    const WAVE: [Param; 2] = [prm("Frequency", 1.0, 20.0, 10.0), prm("Thickness", 0.0, 1.0, 0.4)];
    const CURL: [Param; 2] = [prm("Twirl", -1.0, 1.0, 0.7), prm("Thickness", 0.0, 1.0, 0.3)];
    match d {
        Dash::Herringbone => &HERRING,
        Dash::Circlets => &CIRCLETS,
        Dash::Scratchy => &SCRATCHY,
        Dash::Rope => &ROPE,
        Dash::Tulle => &TULLE,
        Dash::Plait => &PLAIT,
        Dash::Fuzz => &FUZZ,
        Dash::Jagged => &JAGGED,
        Dash::Zigzag => &ZIGZAG,
        Dash::Wave => &WAVE,
        Dash::Curl => &CURL,
        _ => &[],
    }
}

/// Names of a style's extra colours (beyond the stroke colour).
pub fn color_params(d: Dash) -> &'static [&'static str] {
    match d {
        Dash::Rope => &["Outline"],
        Dash::Plait => &["Stripe 2", "Stripe 3"],
        Dash::Bubbles => &["Second colour"],
        _ => &[],
    }
}

/// Parameter `i` of a style (its default when unset), clamped to its range.
pub fn param(st: &StrokeStyle, i: usize) -> f32 {
    let ps = params(st.dash);
    let Some(p) = ps.get(i) else { return 0.0 };
    st.art.p[i].unwrap_or(p.default).clamp(p.min, p.max)
}

/// Extra colour `i` of a style (derived from `base` when unset).
pub fn color_param(st: &StrokeStyle, base: Color, i: usize) -> Color {
    if let Some(c) = st.art.colors.get(i).copied().flatten() {
        return c;
    }
    match (st.dash, i) {
        (Dash::Rope, _) => Color { a: base.a, ..Color::rgb(0, 0, 0) },
        (Dash::Plait, 0) => base.lerp(Color { a: base.a, ..Color::rgb(255, 255, 255) }, 0.45),
        (Dash::Plait, _) => base.lerp(Color { a: base.a, ..Color::rgb(0, 0, 0) }, 0.4),
        _ => base.lerp(Color { a: base.a, ..Color::rgb(255, 255, 255) }, 0.6),
    }
}

/// Geometry for any generated style: (path, fill, stroke) parts, in drawing
/// order. `profile` is a Width-tool profile ((t, × width) pairs).
pub fn styled(path: &Path, stroke: Stroke, st: &StrokeStyle, profile: Option<&[(f32, f32)]>) -> Vec<(Path, Option<Color>, Option<Stroke>)> {
    match st.dash {
        Dash::Ragged | Dash::Stipple | Dash::Hatched => artistic(path, stroke, st.dash),
        Dash::Solid | Dash::Dashed | Dash::Dotted => vec![],
        _ => toonz(path, stroke, st, profile),
    }
}

/// Arc-length sampling of one flattened polyline.
struct Track {
    pts: Vec<Pt>,
    cum: Vec<f32>,
}

impl Track {
    fn new(mut pts: Vec<Pt>, closed: bool) -> Option<Track> {
        pts.dedup_by(|a, b| a.dist(*b) < 1e-4);
        if closed && pts.len() > 2 {
            pts.push(pts[0]);
        }
        if pts.len() < 2 {
            return None;
        }
        let mut cum = vec![0.0];
        for w in pts.windows(2) {
            cum.push(cum.last().unwrap() + w[0].dist(w[1]));
        }
        (*cum.last().unwrap() > 1e-3).then_some(Track { pts, cum })
    }
    fn len(&self) -> f32 {
        *self.cum.last().unwrap()
    }
    fn point(&self, s: f32) -> Pt {
        let s = s.clamp(0.0, self.len());
        let i = self.cum.partition_point(|c| *c <= s).clamp(1, self.pts.len() - 1);
        let (a, b) = (self.cum[i - 1], self.cum[i]);
        self.pts[i - 1].lerp(self.pts[i], if b > a { (s - a) / (b - a) } else { 0.0 })
    }
    /// Point and unit tangent at arc length `s` (tangent smoothed over ±1 unit).
    fn at(&self, s: f32) -> (Pt, Pt) {
        let p = self.point(s);
        let (a, b) = (self.point(s - 1.0), self.point(s + 1.0));
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let l = (dx * dx + dy * dy).sqrt();
        let u = if l > 1e-6 { pt(dx / l, dy / l) } else { pt(1.0, 0.0) };
        (p, u)
    }
}

fn add(a: Pt, b: Pt) -> Pt {
    pt(a.x + b.x, a.y + b.y)
}
fn mul(a: Pt, k: f32) -> Pt {
    pt(a.x * k, a.y * k)
}
fn rot90(u: Pt) -> Pt {
    pt(-u.y, u.x)
}
fn alpha(c: Color, k: f32) -> Color {
    Color { a: (c.a as f32 * k.clamp(0.0, 1.0)).round() as u8, ..c }
}

/// A closed polygon, wound positively so overlapping pieces never cancel.
fn polygon(segs: &mut Vec<Seg>, pts: &[Pt]) {
    if pts.len() < 3 {
        return;
    }
    let area: f32 = (0..pts.len()).map(|i| {
        let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
        a.x * b.y - b.x * a.y
    }).sum();
    if area.abs() < 1e-6 {
        return;
    }
    let mut it: Box<dyn Iterator<Item = &Pt>> = if area > 0.0 { Box::new(pts.iter()) } else { Box::new(pts.iter().rev()) };
    segs.push(Seg::M(*it.next().unwrap()));
    segs.extend(it.map(|p| Seg::L(*p)));
    segs.push(Seg::Z);
}
fn polyline(segs: &mut Vec<Seg>, pts: &[Pt]) {
    if pts.len() < 2 {
        return;
    }
    segs.push(Seg::M(pts[0]));
    segs.extend(pts[1..].iter().map(|p| Seg::L(*p)));
}
fn circle(segs: &mut Vec<Seg>, c: Pt, r: f32) {
    segs.extend(Path::ellipse(c.x, c.y, r, r).segs);
}
/// A quad strip between two point rows (OpenToonz `GL_QUAD_STRIP`).
fn ribbon(segs: &mut Vec<Seg>, a: &[Pt], b: &[Pt]) {
    for i in 1..a.len().min(b.len()) {
        polygon(segs, &[a[i - 1], a[i], b[i], b[i - 1]]);
    }
}

/// Parts collected by colour/alpha bucket, emitted in first-use order.
#[derive(Default)]
struct Parts {
    order: Vec<(Option<Color>, Option<Stroke>)>,
    segs: Vec<Vec<Seg>>,
}
impl Parts {
    fn get(&mut self, fill: Option<Color>, stroke: Option<Stroke>) -> &mut Vec<Seg> {
        let i = match self.order.iter().position(|k| *k == (fill, stroke)) {
            Some(i) => i,
            None => {
                self.order.push((fill, stroke));
                self.segs.push(vec![]);
                self.order.len() - 1
            }
        };
        &mut self.segs[i]
    }
    fn finish(self, out: &mut Vec<(Path, Option<Color>, Option<Stroke>)>) {
        for ((fill, stroke), segs) in self.order.into_iter().zip(self.segs) {
            if !segs.is_empty() {
                out.push((Path { segs }, fill, stroke));
            }
        }
    }
}

/// Quantise an alpha factor to a few levels (keeps the part count small).
fn level(k: f32, n: f32) -> f32 {
    ((k.clamp(0.0, 1.0) * n).ceil() / n).max(1.0 / n)
}

/// Half thickness at `t` (0..1 along the line): OpenToonz's `thick`.
fn half(w: f32, profile: Option<&[(f32, f32)]>, t: f32) -> f32 {
    let k = match profile {
        Some(p) if !p.is_empty() => crate::deform::profile_at(p, t),
        _ => 1.0,
    };
    (w * 0.5 * k).max(0.0)
}

fn toonz(path: &Path, stroke: Stroke, st: &StrokeStyle, profile: Option<&[(f32, f32)]>) -> Vec<(Path, Option<Color>, Option<Stroke>)> {
    let w = stroke.width.max(0.5);
    let c = stroke.color;
    let hair = |k: f32| Stroke { color: c, width: (w * k).clamp(0.5, 1.5) };
    let mut rng = Rng(seed(path));
    let mut parts = Parts::default();
    let mut out = vec![];
    let p = |i| param(st, i);
    for (pts, closed) in path.flatten(12) {
        let Some(tr) = Track::new(pts, closed) else { continue };
        let len = tr.len();
        let th = |s: f32| half(w, profile, s / len);
        match st.dash {
            Dash::Herringbone => {
                let (cs, sn) = (p(0).to_radians().cos(), p(0).to_radians().sin());
                let mut vs = 1.0;
                let mut s = 0.0;
                while s <= len {
                    let (pos, u) = tr.at(s);
                    let v = rot90(u);
                    let l = if th(s) > 0.0 { p(1) * th(s) } else { 1.0 };
                    vs = -vs;
                    let q = 0.01 * rng.sym();
                    let end = add(pos, mul(add(mul(u, cs + q), mul(v, vs * sn)), l));
                    // OpenToonz fades each hair to transparent: three steps here
                    for (k, a) in [(0.0, 1.0), (0.4, 0.6), (0.7, 0.3)] {
                        let k2 = if k == 0.7 { 1.0 } else { k + 0.4 };
                        let seg = parts.get(None, Some(Stroke { color: alpha(c, a), ..hair(0.08) }));
                        polyline(seg, &[pos.lerp(end, k), pos.lerp(end, k2.min(1.0))]);
                    }
                    s += 4.0;
                }
            }
            Dash::Chain => {
                let thick = (0.25 * (th(0.0) + th(len / 3.0) + th(len * 2.0 / 3.0) + th(len))).max(0.1);
                let (ring_h, ring_w) = (thick, 1.5 * thick);
                let dist = 2.0 * 1.2 * ring_w;
                let join = 0.45 * ring_w;
                let (a, b) = (0.6, 0.6);
                let shape = [(1.0, b), (a, 1.0), (-a, 1.0), (-1.0, b), (-1.0, -b), (-a, -1.0), (a, -1.0), (1.0, -b), (1.0, b)];
                let line = Stroke { color: c, width: (thick * 0.3).max(0.5) };
                let mut old: Option<Pt> = None;
                let mut s = 0.0;
                while s <= len {
                    let (pos, u) = tr.at(s);
                    let v = rot90(u);
                    let ring: Vec<Pt> = shape.iter().map(|(x, y)| add(pos, add(mul(u, x * ring_w), mul(v, y * ring_h)))).collect();
                    let seg = parts.get(None, Some(line));
                    polyline(seg, &ring);
                    if let Some(o) = old {
                        polyline(seg, &[o, add(pos, mul(u, -join))]);
                    }
                    old = Some(add(pos, mul(u, join)));
                    s += dist;
                }
            }
            Dash::Circlets => {
                let (blend, intensity, radius) = (p(0), p(1).round() as usize, p(2));
                let decay = 1.0 - blend;
                let step = 4.0;
                let mut s = 0.0;
                while s <= len {
                    let (pos, u) = tr.at(s);
                    let v = rot90(u);
                    let t = th(s);
                    let thickness = t.max(1.0);
                    for _ in 0..intensity {
                        let vr = (0.5 - rng.next()) * 2.0;
                        let shift = add(mul(u, (0.5 - rng.next()) * step), mul(v, vr * t));
                        let m = vr.abs();
                        let a = if m < decay { rng.next() } else { rng.next() * (1.0 - m) };
                        let r = radius * thickness * rng.next();
                        if a < 0.05 || r < 0.05 {
                            continue;
                        }
                        let seg = parts.get(None, Some(Stroke { color: alpha(c, level(a, 4.0)), ..hair(0.05) }));
                        circle(seg, add(pos, shift), r);
                    }
                    s += step;
                }
            }
            Dash::Scratchy => {
                let n = p(0).round() as usize;
                let step = 10.0;
                let mut s = 0.0;
                while s <= len {
                    let (pos, u) = tr.at(s);
                    let v = rot90(u);
                    let t = th(s);
                    let seg = parts.get(None, Some(hair(0.06)));
                    for _ in 0..n {
                        let shift = add(mul(u, (0.5 - rng.next()) * step), mul(v, (0.5 - rng.next()) * t));
                        let q = add(pos, shift);
                        polyline(seg, &[add(q, mul(v, t)), add(q, mul(v, -t))]);
                    }
                    s += step;
                }
            }
            Dash::Rope => {
                let tilt = p(0);
                let step = 10.0;
                let outline = Stroke { color: color_param(st, c, 0), width: (w * 0.06).clamp(0.5, 1.2) };
                let mut prev: Option<(Pt, Pt)> = None;
                let mut s = 0.0;
                while s <= len {
                    let (pos, u) = tr.at(s);
                    let t = th(s);
                    let bend = t * tilt;
                    let bump = (t * 0.3).min(step / 4.0);
                    let v = mul(rot90(u), t);
                    let v1 = mul(v, 0.2);
                    let p1 = add(add(add(pos, mul(u, bend + bump)), v), mul(v1, -1.0));
                    let p2 = add(add(add(pos, mul(u, bump - bend)), mul(v, -1.0)), v1);
                    if let Some((o1, o2)) = prev {
                        let poly = [p1, add(add(pos, mul(u, bend)), v), add(add(o1, mul(u, bump)), v1), o1, o2, add(add(o2, mul(u, bump)), mul(v1, -1.0)), add(add(pos, mul(u, -bend)), mul(v, -1.0)), p2];
                        polygon(parts.get(Some(c), None), &poly);
                        polyline(parts.get(None, Some(outline)), &poly);
                    }
                    prev = Some((p1, p2));
                    s += step;
                }
                if let Some((o1, o2)) = prev {
                    polyline(parts.get(None, Some(outline)), &[o1, o2]);
                }
            }
            Dash::Tulle => {
                let step = 10.0;
                let period = (101.0 - p(0)) * step;
                let opacity = p(1);
                let (mut a, mut b) = (vec![], vec![]);
                let mut s = 0.0;
                while s <= len {
                    let (pos, u) = tr.at(s);
                    let v = mul(rot90(u), th(s) / 2.0);
                    a.push(add(add(pos, mul(v, 1.0 + rng.next())), mul(u, 2.0 * rng.next())));
                    b.push(add(add(pos, mul(v, -(1.0 + rng.next()))), mul(u, -2.0 * rng.next())));
                    s += step;
                }
                let mut counter = 0.0;
                for i in 1..a.len() {
                    if counter > period {
                        counter = 0.0;
                    }
                    let k = opacity + (counter / period) * rng.next();
                    polygon(parts.get(Some(alpha(c, level(k, 5.0))), None), &[a[i - 1], a[i], b[i], b[i - 1]]);
                    counter += step;
                }
                let seg = parts.get(None, Some(hair(0.05)));
                polyline(seg, &a);
                polyline(seg, &b);
            }
            Dash::Plait => {
                const NTICK: usize = 81;
                let period = ((101.0 - p(0)).floor() * 20.0).max(20.0);
                let step = period / NTICK as f32;
                let sin: Vec<f32> = (0..NTICK).map(|z| (z as f32 * std::f32::consts::TAU / NTICK as f32).sin()).collect();
                let colors = [c, color_param(st, c, 0), color_param(st, c, 1)];
                let black = Stroke { color: Color { a: c.a, ..Color::rgb(0, 0, 0) }, width: (w * 0.04).clamp(0.4, 1.0) };
                // (phase, colour index) in drawing order; swapped as OpenToonz does
                let mut braid = [(0usize, 0usize), (NTICK / 3, 1), (2 * NTICK / 3, 2)];
                let mut old: [Option<(Pt, Pt)>; 3] = [None; 3];
                let (mut swap, mut swapcount, mut count) = (0.0, 0usize, 0usize);
                // one bucket set per run between swaps keeps the over/under order
                let mut run = Parts::default();
                let mut s = 0.0;
                while s <= len {
                    count += 1;
                    let (pos, u) = tr.at(s);
                    let v = mul(rot90(u), th(s));
                    let v1 = mul(v, 0.3);
                    let v = mul(v, 0.5);
                    for (phase, ci) in braid.iter() {
                        let k = sin[(count + phase) % NTICK];
                        let p1 = add(pos, mul(v, k));
                        let p2 = add(p1, v1);
                        if let Some((o1, o2)) = old[*ci] {
                            polygon(run.get(Some(colors[*ci]), None), &[o1, p1, p2, o2]);
                            let seg = run.get(None, Some(black));
                            polyline(seg, &[o1, p1]);
                            polyline(seg, &[p2, o2]);
                        }
                        old[*ci] = Some((p1, p2));
                    }
                    s += step;
                    swap += step;
                    if swap > period / 3.0 {
                        swapcount += 1;
                        braid.swap(0, 1 + (swapcount & 1));
                        swap -= period / 3.0;
                        std::mem::take(&mut run).finish(&mut out);
                    }
                }
                run.finish(&mut out);
            }
            Dash::Fuzz => {
                let count = (len * p(0)) as usize;
                let max_dw = (20.0 / len).min(1.0);
                let min_dw = 1.0 / len;
                let line = Stroke { color: alpha(c, 0.5), ..hair(0.05) };
                for _ in 0..count {
                    let r = rng.next();
                    let dw = (1.0 - r) * min_dw + r * max_dw;
                    let (lo, hi) = (dw, 1.0 - dw);
                    if lo >= hi {
                        continue;
                    }
                    let r = rng.next();
                    let wm = (1.0 - r) * lo + r * hi;
                    let (w0, w1) = ((wm - dw) * len, (wm + dw) * len);
                    let (p0, p1) = (tr.point(w0), tr.point(w1));
                    let d01 = p0.dist(p1);
                    if d01 == 0.0 {
                        continue;
                    }
                    let n = (d01 as usize).max(1);
                    let delta = 0.5 * (rng.next() - 0.5) * (th(w0) + th(w1)) * 2.0;
                    let d = 0.1 * d01;
                    let (d0, d1) = (delta - d, delta + d);
                    let pts: Vec<Pt> = (0..=n).map(|j| {
                        let t = j as f32 / n as f32;
                        let (q, u) = tr.at(w0 + (w1 - w0) * t);
                        add(q, mul(rot90(u), (1.0 - t) * d0 + t * d1))
                    }).collect();
                    polyline(parts.get(None, Some(line)), &pts);
                }
            }
            Dash::Bubbles => {
                let c1 = color_param(st, c, 0);
                let mut s = 0.0;
                while s < len {
                    let (pos, _) = tr.at(s);
                    let q = add(pos, pt(2.0 * (rng.next() - 0.5), 2.0 * (rng.next() - 0.5)));
                    let r = rng.next() * th(s).max(1.0);
                    let col = c.lerp(c1, level(rng.next(), 4.0));
                    if r > 0.2 {
                        circle(parts.get(None, Some(Stroke { color: col, ..hair(0.06) })), q, r);
                    }
                    s += 5.0;
                }
            }
            Dash::Jagged => {
                let step = p(0).max(0.5);
                let mut old: Option<Pt> = None;
                let mut s = 0.0;
                while s <= len + 1e-3 {
                    let (pos, u) = tr.at(s.min(len));
                    let v = mul(rot90(u), th(s));
                    let (l, r) = (add(pos, v), add(pos, mul(v, -1.0)));
                    if let Some(o) = old {
                        polygon(parts.get(Some(c), None), &[o, l, r]);
                    }
                    old = Some(l);
                    s += step;
                }
            }
            Dash::Zigzag => {
                let (d0, d1) = (p(0).min(p(1)), p(0).max(p(1)));
                let (a0, a1) = (p(2).min(p(3)), p(2).max(p(3)));
                let thick = p(4);
                let mut first = 1.0;
                let mut pts = vec![];
                let zig = |s: f32, first: f32, rng: &mut Rng| {
                    let (pos, u) = tr.at(s);
                    let angle = a0 + (a1 - a0) * (rng.next() * 101.0).floor() * 0.01;
                    let (sa, ca) = angle.to_radians().sin_cos();
                    let ur = pt(u.x * ca - u.y * sa, u.x * sa + u.y * ca);
                    let mut max_t = if angle == 90.0 { 1.0 } else { 2.0 };
                    if angle > 30.0 && angle < 90.0 {
                        let rta = 1.0 / angle.to_radians().tan();
                        max_t = (rta * rta + 1.0).sqrt();
                    }
                    let r = (0.7 + (max_t - 0.7) * rng.next()) * th(s) * first;
                    add(pos, mul(ur, r))
                };
                let mut s = 0.0;
                while s <= len {
                    pts.push(zig(s, first, &mut rng));
                    first = -first;
                    s += d0 + (d1 - d0) * (rng.next() * 101.0).floor() * 0.01;
                }
                pts.push(zig((len - 1e-3).max(0.0), first, &mut rng));
                polyline(parts.get(None, Some(Stroke { color: c, width: thick.max(0.3) })), &pts);
            }
            Dash::Wave | Dash::Curl => {
                let (mut a, mut b) = (vec![], vec![]);
                if st.dash == Dash::Wave {
                    let freq = p(0) / 100.0;
                    let k = 1.0 - p(1);
                    let mut s = 0.0;
                    while s <= len {
                        let (pos, u) = tr.at(s);
                        let v = rot90(u);
                        let sv = (freq * s).sin() * th(s);
                        a.push(add(pos, mul(v, sv)));
                        b.push(add(pos, mul(v, k * sv)));
                        s += 5.0;
                    }
                } else {
                    let (twirl, k) = (p(0), 1.0 - p(1));
                    let (mut s, mut last_s, mut phi) = (0.01, 0.0, 0.0f32);
                    while s <= len {
                        let (pos, u) = tr.at(s);
                        let v = rot90(u);
                        let t = th(s);
                        if t > 0.0 {
                            let omega = std::f32::consts::PI / t;
                            let q = 0.5 * (1.0 - phi.cos());
                            let theta = std::f32::consts::FRAC_PI_2 - std::f32::consts::PI * twirl * q;
                            let r = t * phi.sin();
                            let (st_, ct) = theta.sin_cos();
                            a.push(add(pos, add(mul(u, r * ct), mul(v, r * st_))));
                            b.push(add(pos, add(mul(u, r * k * ct), mul(v, r * k * st_))));
                            phi += (s - last_s) * omega;
                            last_s = s;
                        } else {
                            a.push(pos);
                            b.push(pos);
                        }
                        s += 0.5;
                    }
                }
                ribbon(parts.get(Some(c), None), &a, &b);
                let seg = parts.get(None, Some(hair(0.04)));
                polyline(seg, &a);
                polyline(seg, &b);
            }
            _ => {}
        }
    }
    parts.finish(&mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artistic_strokes_are_stable_and_stay_near_the_line() {
        let line = Path { segs: vec![Seg::M(pt(0.0, 0.0)), Seg::L(pt(100.0, 0.0))] };
        let st = Stroke { color: Color::rgb(0, 0, 0), width: 6.0 };
        for d in [Dash::Ragged, Dash::Stipple, Dash::Hatched] {
            let a = artistic(&line, st, d);
            assert_eq!(a, artistic(&line, st, d), "{d:?} is deterministic");
            let (lo, hi) = a[0].0.bounds().unwrap();
            assert!(lo.y >= -4.0 && hi.y <= 4.0 && hi.x <= 103.0, "{d:?}: {lo:?} {hi:?}");
            assert!(a[0].0.segs.len() > 10, "{d:?} has detail");
        }
    }

    #[test]
    fn opentoonz_styles_make_geometry_near_the_line() {
        let line = Path { segs: vec![Seg::M(pt(0.0, 0.0)), Seg::L(pt(200.0, 0.0))] };
        let st = Stroke { color: Color::rgb(200, 40, 0), width: 10.0 };
        for d in Dash::ALL.into_iter().filter(|d| is_artistic(*d) && !matches!(d, Dash::Ragged | Dash::Stipple | Dash::Hatched)) {
            let style = StrokeStyle { dash: d, ..StrokeStyle::default() };
            let a = styled(&line, st, &style, None);
            assert!(!a.is_empty(), "{d:?} draws something");
            assert_eq!(a, styled(&line, st, &style, None), "{d:?} is deterministic");
            let mut lo = pt(f32::MAX, f32::MAX);
            let mut hi = pt(f32::MIN, f32::MIN);
            for (p, _, _) in &a {
                let (l, h) = p.bounds().unwrap();
                lo = pt(lo.x.min(l.x), lo.y.min(l.y));
                hi = pt(hi.x.max(h.x), hi.y.max(h.y));
            }
            assert!(lo.y >= -16.0 && hi.y <= 16.0 && lo.x >= -16.0 && hi.x <= 216.0, "{d:?}: {lo:?} {hi:?}");
            assert!(hi.x > 150.0, "{d:?} runs along the line: {hi:?}");
        }
        // Rope and Plait use the extra colours; a width profile narrows the result
        let rope = StrokeStyle { dash: Dash::Rope, ..StrokeStyle::default() };
        assert!(styled(&line, st, &rope, None).iter().any(|(_, _, s)| s.map(|s| s.color) == Some(Color::rgb(0, 0, 0))));
        let wave = StrokeStyle { dash: Dash::Wave, ..StrokeStyle::default() };
        let thin = styled(&line, st, &wave, Some(&[(0.0, 0.1), (1.0, 0.1)]));
        let (l, h) = thin[0].0.bounds().unwrap();
        assert!(h.y - l.y < 2.0, "profile scales the wave: {l:?} {h:?}");
        // parameters: defaults, overrides and clamping
        let mut z = StrokeStyle { dash: Dash::Zigzag, ..StrokeStyle::default() };
        assert_eq!(param(&z, 1), 6.0);
        z.art.p[1] = Some(500.0);
        assert_eq!(param(&z, 1), 50.0);
    }
}
