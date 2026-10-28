//! Motion tweens: a tween span moving one target, a symbol instance or text,
//! by property keyframes inside the span. Each property is keyed on its own:
//! where the target's transformation point is (the motion path), its rotation,
//! skew, scale and color effect. As in Animate's motion XML, which this reads
//! and writes, values are relative to the target as its keyframe holds it.

use std::borrow::Cow;
use std::collections::BTreeSet;
use std::f32::consts::{PI, TAU};

use roxmltree::Node;

use crate::Result;
use crate::doc::*;
use crate::text;
use crate::xfl_write::Xml;

/// Indices into `Motion::props`.
pub const ROTATION: usize = 0;
pub const SKEW_X: usize = 1;
pub const SKEW_Y: usize = 2;
pub const SCALE_X: usize = 3;
pub const SCALE_Y: usize = 4;
/// Each of `props` where it has no keyframes: no change.
const NEUTRAL: [f32; 5] = [0.0, 0.0, 0.0, 100.0, 100.0];
/// Motion XML time units in a frame: Animate's TimeScale is the frame rate × 1000.
const UNIT: u32 = 1000;

/// A motion tween: property keyframes by frame into its span, and its settings.
#[derive(Clone, Debug, PartialEq)]
pub struct Motion {
    /// The motion path: where the target's transformation point has moved,
    /// with the handles of the curve into and out of each point (on the point:
    /// a straight run). Played at an even speed along each curve.
    pub path: Vec<PathKey>,
    /// Rotation, skew x and skew y in degrees, scale x and y in percent; see `ROTATION`...
    pub props: [Vec<(u32, f32)>; 5],
    /// The color effect. Unlike the rest it is the effect itself, as Animate
    /// keys it; with no keyframes the target's own shows.
    pub color: Vec<(u32, ColorTransform)>,
    /// -1 (in) ..= 1 (out), as a classic tween's, from each keyframe to the next.
    // ponytail: Animate eases the whole span's time (the XML's TimeMap), which keyframes in
    // between then don't keep; easing each stretch is the same with two keyframes, and keeps
    // auto-keyframing exact. Remap the span's time if eased tweens with more drift.
    pub ease: f32,
    /// Whole turns over the span, on top of the rotation keyframes.
    pub rotate: Rotate,
    /// The target turns with the path.
    pub orient: bool,
    /// The motion XML's TimeScale: frames per second × 1000.
    pub time_scale: u32,
    /// Motion XML the model doesn't read (depth, filters, 3D...): (container id, XML).
    pub kept: Vec<(String, String)>,
    /// `fingerprint` of the tween as read from a file, whose motion XML the frame's
    /// `extra` still holds: saving writes that back, untouched, until the tween changes.
    pub read: u64,
}

/// A keyframe of the motion path, relative to where the target's transformation point starts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PathKey {
    pub frame: u32,
    pub at: Point,
    /// The handles of the curve arriving here and leaving.
    pub prev: Point,
    pub next: Point,
}

impl PathKey {
    /// A point the path runs straight into and out of.
    pub fn still(frame: u32, at: Point) -> PathKey {
        PathKey { frame, at, prev: at, next: at }
    }
}

/// What a property keyframe keys, as the timeline's Insert and View Keyframes menus have it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Position,
    Scale,
    Skew,
    Rotation,
    Color,
}

impl Kind {
    pub const ALL: [Kind; 5] = [Kind::Position, Kind::Scale, Kind::Skew, Kind::Rotation, Kind::Color];

    fn props(self) -> &'static [usize] {
        match self {
            Kind::Rotation => &[ROTATION],
            Kind::Skew => &[SKEW_X, SKEW_Y],
            Kind::Scale => &[SCALE_X, SCALE_Y],
            Kind::Position | Kind::Color => &[],
        }
    }
}

impl Motion {
    /// A tween with no keyframes yet, so the target stays as it is, in a document
    /// playing `fps` frames a second.
    pub fn new(fps: f32) -> Motion {
        Motion {
            path: Vec::new(),
            props: Default::default(),
            color: Vec::new(),
            ease: 0.0,
            rotate: Rotate::Auto,
            orient: false,
            time_scale: (fps * UNIT as f32).round() as u32,
            kept: Vec::new(),
            read: 0,
        }
    }

    /// The frames with a keyframe of `kind`.
    pub fn frames(&self, kind: Kind) -> Vec<u32> {
        let frames: BTreeSet<u32> = match kind {
            Kind::Position => self.path.iter().map(|k| k.frame).collect(),
            Kind::Color => self.color.iter().map(|k| k.0).collect(),
            _ => kind.props().iter().flat_map(|&i| self.props[i].iter().map(|k| k.0)).collect(),
        };
        frames.into_iter().collect()
    }

    /// The target's matrix `offset` frames into a `duration`-frame span, its own
    /// being `base` and its transformation point `pivot` (in its own coordinates).
    pub fn matrix(&self, base: &Matrix, pivot: Point, offset: u32, duration: u32) -> Matrix {
        let at = offset as f32;
        let mut turn = self.value(ROTATION, at) + self.spin(offset, duration);
        if self.orient && self.path.len() > 1 {
            turn += (self.along(at).1 - self.along(0.0).1).to_degrees();
        }
        let (rot, kx, ky) = (turn.to_radians(), self.value(SKEW_X, at).to_radians(), self.value(SKEW_Y, at).to_radians());
        let (sx, sy) = (self.value(SCALE_X, at) / 100.0, self.value(SCALE_Y, at) / 100.0);
        let (d, _) = self.along(at);
        if (rot, kx, ky, sx, sy, d) == (0.0, 0.0, 0.0, 1.0, 1.0, (0.0, 0.0)) {
            return *base;
        }
        let (bx, by, bkx, bky) = base.parts();
        let mut m = Matrix::from_parts((bx * sx, by * sy, bkx + rot + kx, bky + rot + ky));
        // The transformation point lands where the path has it.
        let (from, now) = (base.apply(pivot), m.apply(pivot));
        (m.tx, m.ty) = (from.0 + d.0 - now.0, from.1 + d.1 - now.1);
        m
    }

    /// The color effect `offset` frames in, if keyed.
    pub fn color_at(&self, offset: u32) -> Option<ColorTransform> {
        sample(&self.color, offset as f32, self.ease, |a, b, t| ColorTransform {
            mult: std::array::from_fn(|i| a.mult[i] + (b.mult[i] - a.mult[i]) * t),
            add: std::array::from_fn(|i| a.add[i] + (b.add[i] - a.add[i]) * t),
        })
    }

    /// Where the path has the transformation point at frame `at` (relative to where it
    /// starts), and its heading there in radians. It holds before the first keyframe and past the last.
    pub fn along(&self, at: f32) -> (Point, f32) {
        let n = self.path.len();
        match n {
            0 => return ((0.0, 0.0), 0.0),
            1 => return (self.path[0].at, 0.0),
            _ => {}
        }
        let i = self.path.partition_point(|k| k.frame as f32 <= at).clamp(1, n - 1);
        let (a, b) = (&self.path[i - 1], &self.path[i]);
        let s = ((at - a.frame as f32) / (b.frame - a.frame) as f32).clamp(0.0, 1.0);
        segment(a, b, ease(self.ease, s))
    }

    fn value(&self, i: usize, at: f32) -> f32 {
        sample(&self.props[i], at, self.ease, |a, b, t| a + (b - a) * t).unwrap_or(NEUTRAL[i])
    }

    /// The whole turns' share, in degrees, `offset` frames in.
    fn spin(&self, offset: u32, duration: u32) -> f32 {
        let turns = match self.rotate {
            Rotate::Auto => return 0.0,
            Rotate::Cw(n) => n as f32,
            Rotate::Ccw(n) => -(n as f32),
        };
        let t = (offset as f32 / duration.saturating_sub(1).max(1) as f32).min(1.0);
        360.0 * turns * ease(self.ease, t)
    }

    /// Auto-keyframing: keys frame `at` of a `duration`-frame span so the target, its own
    /// matrix being `base` and its transformation point `pivot`, shows with matrix `want`
    /// there. What moved gets a position keyframe, what turned a rotation one, what slanted
    /// skew ones, what grew scale ones. A property's first keyframe past frame 0 brings
    /// one there along, keeping what showed before.
    pub fn key_matrix(&mut self, base: &Matrix, pivot: Point, at: u32, duration: u32, want: &Matrix) {
        let now = self.matrix(base, pivot, at, duration);
        let (p, q) = (now.apply(pivot), want.apply(pivot));
        if (q.0 - p.0).hypot(q.1 - p.1) > 1e-3 {
            let from = base.apply(pivot);
            self.set_point(at, (q.0 - from.0, q.1 - from.1));
        }
        let ((sx0, sy0, kx0, ky0), (sx1, sy1, kx1, ky1)) = (now.parts(), want.parts());
        let turned = |a: f32, b: f32| ((b - a + PI).rem_euclid(TAU) - PI).to_degrees();
        let (dx, dy) = (turned(kx0, kx1), turned(ky0, ky1));
        // Both axes turned alike: a rotation; else a skew.
        let changes: &[(usize, f32)] = if (dx - dy).abs() < 1e-3 { &[(ROTATION, dx)] } else { &[(SKEW_X, dx), (SKEW_Y, dy)] };
        for &(i, d) in changes {
            if d.abs() > 1e-3 {
                self.set(i, at, self.value(i, at as f32) + d);
            }
        }
        for (i, a, b) in [(SCALE_X, sx0, sx1), (SCALE_Y, sy0, sy1)] {
            if a > 1e-6 && (b / a - 1.0).abs() > 1e-5 {
                self.set(i, at, self.value(i, at as f32) * b / a);
            }
        }
    }

    /// Auto-keyframing the color effect: frame `at` shows `c`, the target's own being `own`.
    pub fn key_color(&mut self, at: u32, own: ColorTransform, c: ColorTransform) {
        if self.color.is_empty() && at > 0 {
            self.color.push((0, own));
        }
        put(&mut self.color, at, c);
    }

    /// Keys the path's point at frame `at` to `to`; a keyframe there takes its handles along.
    pub fn set_point(&mut self, at: u32, to: Point) {
        if self.path.is_empty() && at > 0 {
            self.path.push(PathKey::still(0, (0.0, 0.0)));
        }
        self.cut_path(at);
        if let Ok(i) = self.path.binary_search_by_key(&at, |k| k.frame) {
            let k = &mut self.path[i];
            let d = (to.0 - k.at.0, to.1 - k.at.1);
            *k = PathKey { frame: at, at: to, prev: shift(k.prev, d), next: shift(k.next, d) };
        }
    }

    /// Bends the path's stretch around frame `at` (no keyframe of its own) so it passes `d`
    /// further on there; `from` is the motion as the bend started. The handles either side
    /// move, as a curve dragged with the Selection tool.
    pub fn bend(&mut self, from: &Motion, at: u32, d: Point) {
        let i = from.path.partition_point(|k| k.frame < at);
        if i == 0 || i >= from.path.len() || from.path[i].frame == at {
            return;
        }
        let (a, b) = (&from.path[i - 1], &from.path[i]);
        let s = ease(from.ease, (at - a.frame) as f32 / (b.frame - a.frame) as f32);
        // A point of the curve moves by 3u(1 - u) times what both handles move.
        let u = if straight(a, b) { s } else { by_length([a.at, a.next, b.prev, b.at], s) }.clamp(0.1, 0.9);
        let w = 3.0 * u * (1.0 - u);
        let pull = (d.0 / w, d.1 / w);
        // A straight run starts as a curve with its handles a third of the way along.
        let (next, prev) = if straight(a, b) { (mix(a.at, b.at, 1.0 / 3.0), mix(a.at, b.at, 2.0 / 3.0)) } else { (a.next, b.prev) };
        (self.path[i - 1].next, self.path[i].prev) = (shift(next, pull), shift(prev, pull));
    }

    fn set(&mut self, i: usize, at: u32, value: f32) {
        let keys = &mut self.props[i];
        if keys.is_empty() && at > 0 {
            keys.push((0, NEUTRAL[i]));
        }
        put(keys, at, value);
    }

    /// Insert Keyframe: keys `kind` at frame `at` as it shows there; `own` is the target's color.
    pub fn key(&mut self, kind: Kind, at: u32, own: ColorTransform) {
        match kind {
            Kind::Position => self.set_point(at, self.along(at as f32).0),
            Kind::Color => self.key_color(at, own, self.color_at(at).unwrap_or(own)),
            _ => kind.props().iter().for_each(|&i| self.set(i, at, self.value(i, at as f32))),
        }
    }

    /// Clear Keyframe: `kind`'s keyframe at frame `at` goes.
    pub fn clear(&mut self, kind: Kind, at: u32) {
        match kind {
            Kind::Position => self.path.retain(|k| k.frame != at),
            Kind::Color => self.color.retain(|k| k.0 != at),
            _ => kind.props().iter().for_each(|&i| self.props[i].retain(|k| k.0 != at)),
        }
    }

    /// `n` frames inserted after frame `at` of the span: later keyframes move along.
    pub fn insert_frames(&mut self, at: u32, n: u32) {
        self.renumber(|f| Some(if f > at { f + n } else { f }));
    }

    /// Frames `at..at + n` of a `duration`-frame span removed: their keyframes go, later ones
    /// move back. When keyframes go, the frames either side are keyed first, so the rest of
    /// the span shows what it did rather than losing its start or end.
    pub fn remove_frames(&mut self, at: u32, n: u32, duration: u32) {
        let end = at.saturating_add(n);
        if Kind::ALL.iter().any(|&k| self.frames(k).iter().any(|f| (at..end).contains(f))) {
            if at > 0 {
                self.cut(at - 1);
            }
            if end < duration {
                self.cut(end);
            }
        }
        self.renumber(|f| match f {
            f if f < at => Some(f),
            f if f < end => None,
            f => Some(f - n),
        });
    }

    /// Moves every keyframe to the frame `to` gives, dropping those it gives none.
    fn renumber(&mut self, to: impl Fn(u32) -> Option<u32>) {
        self.path.retain_mut(|k| to(k.frame).map(|f| k.frame = f).is_some());
        for keys in &mut self.props {
            keys.retain_mut(|k| to(k.0).map(|f| k.0 = f).is_some());
        }
        self.color.retain_mut(|k| to(k.0).map(|f| k.0 = f).is_some());
    }

    /// Keys every keyed property at frame `at` as it is there, the motion staying as it was.
    fn cut(&mut self, at: u32) {
        if !self.path.is_empty() {
            self.cut_path(at);
        }
        for i in 0..self.props.len() {
            if !self.props[i].is_empty() && !self.props[i].iter().any(|k| k.0 == at) {
                let v = self.value(i, at as f32);
                put(&mut self.props[i], at, v);
            }
        }
        if let Some(c) = self.color_at(at).filter(|_| !self.color.iter().any(|k| k.0 == at)) {
            put(&mut self.color, at, c);
        }
    }

    /// A position keyframe at frame `at` where the path is, a curve it cuts keeping its shape.
    fn cut_path(&mut self, at: u32) {
        let i = self.path.partition_point(|k| k.frame < at);
        if self.path.get(i).is_some_and(|k| k.frame == at) {
            return;
        }
        let mut key = PathKey::still(at, self.along(at as f32).0);
        if i > 0 && i < self.path.len() && !straight(&self.path[i - 1], &self.path[i]) {
            let (a, b) = (self.path[i - 1], self.path[i]);
            let u = by_length([a.at, a.next, b.prev, b.at], ease(self.ease, (at - a.frame) as f32 / (b.frame - a.frame) as f32));
            // de Casteljau: the curve's two halves either side of `u`.
            let (p01, p12, p23) = (mix(a.at, a.next, u), mix(a.next, b.prev, u), mix(b.prev, b.at, u));
            let (p012, p123) = (mix(p01, p12, u), mix(p12, p23, u));
            (self.path[i - 1].next, self.path[i].prev) = (p01, p23);
            key = PathKey { frame: at, at: mix(p012, p123, u), prev: p012, next: p123 };
        }
        self.path.insert(i, key);
    }

    /// The whole turns as rotation keyframes, for a span `duration` frames long.
    fn bake_spin(&mut self, duration: u32) {
        if self.rotate == Rotate::Auto {
            return;
        }
        let last = duration.saturating_sub(1);
        for at in [0, last] {
            if !self.props[ROTATION].iter().any(|k| k.0 == at) {
                let v = self.value(ROTATION, at as f32);
                put(&mut self.props[ROTATION], at, v);
            }
        }
        let spun: Vec<f32> = self.props[ROTATION].iter().map(|k| self.spin(k.0, duration)).collect();
        self.props[ROTATION].iter_mut().zip(spun).for_each(|(k, s)| k.1 += s);
        self.rotate = Rotate::Auto;
    }
}

/// The value `keys` give at frame `at`, `lerp`ing from each keyframe to the next, eased;
/// held before the first and past the last. None without keys.
fn sample<T: Copy>(keys: &[(u32, T)], at: f32, eased: f32, lerp: impl Fn(T, T, f32) -> T) -> Option<T> {
    let n = keys.len();
    if n < 2 {
        return keys.first().map(|k| k.1);
    }
    let i = keys.partition_point(|k| k.0 as f32 <= at).clamp(1, n - 1);
    let ((a, va), (b, vb)) = (keys[i - 1], keys[i]);
    let s = ((at - a as f32) / (b - a) as f32).clamp(0.0, 1.0);
    Some(lerp(va, vb, ease(eased, s)))
}

/// Sets the keyframe at frame `at`, keeping `keys` in frame order.
fn put<T>(keys: &mut Vec<(u32, T)>, at: u32, value: T) {
    match keys.binary_search_by_key(&at, |k| k.0) {
        Ok(i) => keys[i].1 = value,
        Err(i) => keys.insert(i, (at, value)),
    }
}

/// `s` (0..=1) eased as a classic tween eases: -1 in ..= 1 out.
fn ease(ease: f32, s: f32) -> f32 {
    s + ease * s * (1.0 - s)
}

fn straight(a: &PathKey, b: &PathKey) -> bool {
    a.next == a.at && b.prev == b.at
}

/// The point `s` of the way along the stretch from `a` to `b`, and the heading there.
fn segment(a: &PathKey, b: &PathKey, s: f32) -> (Point, f32) {
    if straight(a, b) {
        return (mix(a.at, b.at, s), (b.at.1 - a.at.1).atan2(b.at.0 - a.at.0));
    }
    let p = [a.at, a.next, b.prev, b.at];
    let u = by_length(p, s);
    (cubic(p, u), heading(p, u))
}

fn cubic([p0, p1, p2, p3]: [Point; 4], u: f32) -> Point {
    let v = 1.0 - u;
    let (a, b, c, d) = (v * v * v, 3.0 * v * v * u, 3.0 * v * u * u, u * u * u);
    (a * p0.0 + b * p1.0 + c * p2.0 + d * p3.0, a * p0.1 + b * p1.1 + c * p2.1 + d * p3.1)
}

/// The curve's heading at `u`, in radians.
fn heading(p: [Point; 4], u: f32) -> f32 {
    let v = 1.0 - u;
    let (a, b, c) = (3.0 * v * v, 6.0 * v * u, 3.0 * u * u);
    let d = |i: usize| (p[i + 1].0 - p[i].0, p[i + 1].1 - p[i].1);
    let (d0, d1, d2) = (d(0), d(1), d(2));
    let (x, y) = (a * d0.0 + b * d1.0 + c * d2.0, a * d0.1 + b * d1.1 + c * d2.1);
    if x.hypot(y) > 1e-6 { y.atan2(x) } else { (p[3].1 - p[0].1).atan2(p[3].0 - p[0].0) }
}

/// The curve's parameter `s` of the way along it by length, measured on 32 chords.
fn by_length(p: [Point; 4], s: f32) -> f32 {
    const N: usize = 32;
    let mut lengths = [0.0f32; N + 1];
    let mut prev = p[0];
    for i in 1..=N {
        let q = cubic(p, i as f32 / N as f32);
        lengths[i] = lengths[i - 1] + (q.0 - prev.0).hypot(q.1 - prev.1);
        prev = q;
    }
    let want = s.clamp(0.0, 1.0) * lengths[N];
    let i = lengths.partition_point(|&l| l < want).clamp(1, N);
    let run = lengths[i] - lengths[i - 1];
    let frac = if run > 0.0 { (want - lengths[i - 1]) / run } else { 0.0 };
    (i as f32 - 1.0 + frac) / N as f32
}

fn mix(a: Point, b: Point, t: f32) -> Point {
    (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
}

fn shift(p: Point, d: Point) -> Point {
    (p.0 + d.0, p.1 + d.1)
}

// The tween span in the timeline.

/// The target of `key`'s motion tween: its lone instance or text.
pub fn target(key: &Frame) -> Option<&Element> {
    match &key.elements[..] {
        [el @ (Element::Instance(_) | Element::Text(_))] => Some(el),
        _ => None,
    }
}

/// A target's own matrix and transformation point: an instance's own, text's box middle.
fn handle(el: &Element) -> Option<(Matrix, Point)> {
    match el {
        Element::Instance(i) => Some((i.matrix, i.pivot)),
        Element::Text(t) => {
            let l = text::layout(t);
            Some((t.matrix, (l.width / 2.0, l.height / 2.0)))
        }
        _ => None,
    }
}

/// The target of `key`'s motion tween as it shows `offset` frames into the span.
pub fn target_at(key: &Frame, offset: u32) -> Option<Element> {
    let Tween::Motion(m) = &key.tween else { return None };
    let mut el = target(key)?.clone();
    let (base, pivot) = handle(&el)?;
    let matrix = m.matrix(&base, pivot, offset, key.duration);
    match &mut el {
        Element::Instance(i) => {
            i.matrix = matrix;
            i.color = m.color_at(offset).unwrap_or(i.color);
        }
        Element::Text(t) => t.matrix = matrix,
        _ => {}
    }
    Some(el)
}

/// The matrix of `key`'s motion-tweened text `offset` frames in.
pub fn text_at(key: &Frame, offset: u32) -> Option<Matrix> {
    match target_at(key, offset)? {
        Element::Text(t) => Some(t.matrix),
        _ => None,
    }
}

/// The motion path of `key`'s tween where it shows: the target's transformation
/// point at each frame of the span, in the timeline's coordinates.
pub fn path_points(key: &Frame) -> Vec<Point> {
    let (Tween::Motion(m), Some((base, pivot))) = (&key.tween, target(key).and_then(handle)) else { return Vec::new() };
    let from = base.apply(pivot);
    (0..key.duration).map(|f| shift(from, m.along(f as f32).0)).collect()
}

/// Whether element `r` of `tl` is a motion tween's target, and its span holds `frame`.
pub fn tweening(tl: &Timeline, r: crate::hit::ElementRef, frame: u32) -> bool {
    let Some(key) = tl.layers.get(r.layer).and_then(|l| l.frames.get(r.keyframe)) else { return false };
    (key.index..key.index + key.duration).contains(&frame) && r.element == 0 && matches!(key.tween, Tween::Motion(_)) && target(key).is_some()
}

/// Auto-keyframing: runs `edit` on element `r` of `tl` as it shows at `frame`. On a motion
/// tween's target, inside its span, what it changes becomes property keyframes there and
/// the target itself stays; anything else just takes the edit.
pub fn edit_shown(tl: &mut Timeline, r: crate::hit::ElementRef, frame: u32, edit: impl FnOnce(&mut Element)) {
    if !tweening(tl, r, frame) {
        if let Some(el) = tl.element_mut(r) {
            edit(el);
        }
        return;
    }
    let key = &mut tl.layers[r.layer].frames[r.keyframe];
    let offset = frame - key.index;
    let Some(mut shown) = target_at(key, offset) else { return };
    let before = shown.clone();
    edit(&mut shown);
    let (Some((base, pivot)), Tween::Motion(m)) = (target(key).and_then(handle), &mut key.tween) else { return };
    match (&before, &shown) {
        (Element::Instance(a), Element::Instance(b)) => {
            if a.matrix != b.matrix {
                m.key_matrix(&base, pivot, offset, key.duration, &b.matrix);
            }
            if a.color != b.color
                && let Some(Element::Instance(own)) = key.elements.first()
            {
                m.key_color(offset, own.color, b.color);
            }
        }
        (Element::Text(a), Element::Text(b)) if a.matrix != b.matrix => m.key_matrix(&base, pivot, offset, key.duration, &b.matrix),
        _ => {}
    }
}

/// Splits `key`'s motion tween at frame `at` (0 < `at` < `key.duration`) of its span:
/// `key` keeps the keyframes before `at`, and the tween of the frames from `at` on comes
/// back, rebased on its own target, `key`'s as it shows at `at`. None if no motion tween.
pub fn split(key: &mut Frame, at: u32) -> Option<Motion> {
    let duration = key.duration;
    let Tween::Motion(m) = &mut key.tween else { return None };
    m.bake_spin(duration);
    m.cut(at - 1);
    m.cut(at);
    let mut tail = (**m).clone();
    m.renumber(|f| (f < at).then_some(f));
    tail.renumber(|f| f.checked_sub(at));
    // What the frames from `at` show is now the target itself.
    if let Some(p0) = tail.path.first().map(|k| k.at) {
        let back = (-p0.0, -p0.1);
        tail.path.iter_mut().for_each(|k| *k = PathKey { frame: k.frame, at: shift(k.at, back), prev: shift(k.prev, back), next: shift(k.next, back) });
    }
    for i in [ROTATION, SKEW_X, SKEW_Y] {
        if let Some(v0) = tail.props[i].first().map(|k| k.1) {
            tail.props[i].iter_mut().for_each(|k| k.1 -= v0);
        }
    }
    for i in [SCALE_X, SCALE_Y] {
        if let Some(v0) = tail.props[i].first().map(|k| k.1).filter(|v| v.abs() > 1e-6) {
            tail.props[i].iter_mut().for_each(|k| k.1 *= 100.0 / v0);
        }
    }
    Some(tail)
}

/// `key`'s tween as it runs over frames `from..to` of its span, for frames copied out of
/// it: from `from` on it is `split`'s second half, and short of the span's end its whole
/// turns are keyed, as they don't fit the shorter span.
pub fn copied(key: &Frame, from: u32, to: u32) -> Tween {
    let Tween::Motion(m) = &key.tween else { return key.tween.clone() };
    if from > 0 {
        return split(&mut key.clone(), from).map_or(Tween::None, |m| Tween::Motion(Box::new(m)));
    }
    let mut m = m.clone();
    if to < key.duration {
        m.bake_spin(key.duration);
    }
    Tween::Motion(m)
}

impl Layer {
    /// Create Motion Tween on the span holding frame `f`, whose keyframe holds one
    /// instance or text (the tween's target). A span one frame long at the layer's end
    /// grows to a second's worth of frames at `fps`, as in Animate. Returns whether it made one.
    pub fn create_motion_tween(&mut self, f: u32, fps: f32) -> bool {
        let Some(i) = self.keyframe_at(f) else { return false };
        let last = i + 1 == self.frames.len();
        let key = &mut self.frames[i];
        if matches!(key.tween, Tween::Motion(_)) || target(key).is_none() {
            return false;
        }
        if key.duration == 1 && last {
            key.duration = (fps.round() as u32).max(2);
        }
        key.tween = Tween::Motion(Box::new(Motion::new(fps)));
        true
    }

    /// Split Motion: the motion tween span holding frame `f` becomes two, the second from
    /// `f` on, each showing what it did. Returns whether it split one.
    pub fn split_motion(&mut self, f: u32) -> bool {
        let Some(i) = self.keyframe_at(f).filter(|&i| self.frames[i].index < f && matches!(self.frames[i].tween, Tween::Motion(_))) else { return false };
        self.split_at(f, true);
        self.frames.get(i + 1).is_some_and(|k| k.index == f)
    }

    /// Insert Keyframe inside a motion tween: keys every property at frame `at`. Just past
    /// the layer's end, a motion tween ending it first stretches to `at`. Returns whether it did.
    pub fn key_motion(&mut self, at: u32) -> bool {
        let Some(last) = self.frames.last_mut() else { return false };
        if matches!(last.tween, Tween::Motion(_)) && at >= last.index + last.duration {
            last.duration = at - last.index + 1;
        }
        let Some(i) = self.keyframe_at(at) else { return false };
        let key = &mut self.frames[i];
        let own = match key.elements.first() {
            Some(Element::Instance(i)) => i.color,
            _ => ColorTransform::IDENTITY,
        };
        match &mut key.tween {
            Tween::Motion(m) if at > key.index => {
                Kind::ALL.into_iter().for_each(|kind| m.key(kind, at - key.index, own));
                true
            }
            _ => false,
        }
    }
}

// Animate's motion XML: a frame's <motionObjectXML><AnimationCore>, its properties in
// containers, each with keyframes holding `anchor`, `previous` and `next` as "time,value".
// ponytail: only the values are read. `previous` and `next` are taken as the path's handles
// for Motion_X and Motion_Y, and are ignored elsewhere, which reads Motion Editor curves as
// straight lines. How Animate shapes a curved path is unconfirmed; ours round-trips.

/// The ids of `props` in the XML, with the container each sits in.
const PROPS: [(&str, &str); 5] =
    [("Basic_Motion", "Rotation_Z"), ("Transformation", "Skew_X"), ("Transformation", "Skew_Y"), ("Transformation", "Scale_X"), ("Transformation", "Scale_Y")];
/// Advanced color properties: per channel (red, green, blue, alpha) a percentage and an offset.
// ponytail: the color properties' ids (these, Alpha_Amount, Brightness_Amount, Tint_*) are
// from memory of Animate's files, unconfirmed; color keys are saved as Advanced_Color.
const ADVANCED: [(&str, &str); 4] = [("AdvClr_R_Pct", "AdvClr_R_Offset"), ("AdvClr_G_Pct", "AdvClr_G_Offset"), ("AdvClr_B_Pct", "AdvClr_B_Offset"), ("AdvClr_A_Pct", "AdvClr_A_Offset")];

/// A keyframe as the XML has it: frame, value, and the values of its incoming and outgoing handles.
type XmlKey = (u32, f32, f32, f32);

/// Reads a `tweenType="motion object"` frame `n`, `duration` frames long: its turns and motion XML.
pub(crate) fn read(n: Node, duration: u32) -> Result<Motion> {
    let mut m = Motion::new(24.0);
    let times = |n: Node| n.attribute("motionTweenRotateTimes").map_or(Ok(1), str::parse);
    m.rotate = match n.attribute("motionTweenRotate") {
        Some("clockwise") => Rotate::Cw(times(n)?),
        Some("counter-clockwise") => Rotate::Ccw(times(n)?),
        _ => Rotate::Auto,
    };
    let Some(core) = child(n, "motionObjectXML").and_then(|x| child(x, "AnimationCore")) else { return Ok(m) };
    m.time_scale = core.attribute("TimeScale").map_or(Ok(m.time_scale), str::parse)?;
    if let Some(strength) = child(core, "TimeMap").and_then(|t| t.attribute("strength")) {
        m.ease = strength.parse::<f32>()? / 100.0;
    }
    let settings = core.descendants().find(|d| d.tag_name().name() == "Settings");
    m.orient = settings.and_then(|s| s.attribute("orientToPath")) == Some("1");
    let mut tracks: Vec<(&str, Vec<XmlKey>)> = Vec::new();
    for c in child(core, "PropertyContainer").into_iter().flat_map(|head| head.children().filter(Node::is_element)) {
        let id = c.attribute("id").unwrap_or_default();
        match id {
            "Basic_Motion" | "Transformation" => read_props(c, id, &mut tracks, &mut m.kept)?,
            // Each color effect has a container of its own; one with properties not read stays as it is.
            "Colors" => {
                for effect in c.children().filter(Node::is_element) {
                    let known = |p: Node| matches!(p.attribute("id"), Some(id) if color_prop(id));
                    if effect.children().filter(Node::is_element).all(known) {
                        read_props(effect, "Colors", &mut tracks, &mut Vec::new())?;
                    } else {
                        m.kept.push(("Colors".into(), source(effect)));
                    }
                }
            }
            // Filters and the like; an empty one, which saving writes anyway, holds nothing to keep.
            _ if c.children().any(|n| n.is_element()) => m.kept.push(("headContainer".into(), source(c))),
            _ => {}
        }
    }
    let track = |id: &str| tracks.iter().find(|t| t.0 == id).map_or(&[][..], |t| &t.1[..]);
    let (xs, ys) = (track("Motion_X"), track("Motion_Y"));
    let frames: BTreeSet<u32> = xs.iter().chain(ys).map(|k| k.0).collect();
    m.path = frames
        .into_iter()
        .map(|f| {
            // A frame keyed on one axis only takes the other's value there.
            let pick = |keys: &[XmlKey]| match keys.iter().find(|k| k.0 == f) {
                Some(&(_, v, prev, next)) => (v, prev, next),
                None => {
                    let v = sample(&keys.iter().map(|k| (k.0, k.1)).collect::<Vec<_>>(), f as f32, m.ease, |a, b, t| a + (b - a) * t);
                    let v = v.unwrap_or(0.0);
                    (v, v, v)
                }
            };
            let ((x, px, nx), (y, py, ny)) = (pick(xs), pick(ys));
            PathKey { frame: f, at: (x, y), prev: (px, py), next: (nx, ny) }
        })
        .collect();
    for (i, (_, id)) in PROPS.iter().enumerate() {
        m.props[i] = track(id).iter().map(|k| (k.0, k.1)).collect();
    }
    m.color = read_color(&tracks);
    // A property keyed once, at frame 0, to no change, is as good as not keyed: Animate writes those.
    if let [k] = m.path[..]
        && k == PathKey::still(0, (0.0, 0.0))
    {
        m.path.clear();
    }
    for (keys, neutral) in m.props.iter_mut().zip(NEUTRAL) {
        if keys[..] == [(0, neutral)] {
            keys.clear();
        }
    }
    if m.color[..] == [(0, ColorTransform::IDENTITY)] {
        m.color.clear();
    }
    m.read = fingerprint(&m, duration);
    Ok(m)
}

/// The properties of container `c` into `tracks`, by id; those not read into `kept`.
fn read_props<'a>(c: Node<'a, '_>, container: &str, tracks: &mut Vec<(&'a str, Vec<XmlKey>)>, kept: &mut Vec<(String, String)>) -> Result<()> {
    for p in c.children().filter(Node::is_element) {
        let id = p.attribute("id").unwrap_or_default();
        let known = matches!(id, "Motion_X" | "Motion_Y") || PROPS.iter().any(|&(_, name)| name == id) || color_prop(id);
        if !known || p.tag_name().name() != "Property" {
            kept.push((container.into(), source(p)));
            continue;
        }
        let mut keys = Vec::new();
        for k in p.children().filter(|k| k.tag_name().name() == "Keyframe") {
            let value = |name: &str| -> Result<f32> {
                let v = k.attribute(name).unwrap_or("0,0");
                Ok(v.rsplit(',').next().unwrap_or_default().trim().parse::<f32>().map_err(|e| format!("Keyframe {name}={v:?}: {e}"))?)
            };
            let time: f32 = k.attribute("timevalue").map_or(Ok(0.0), str::parse)?;
            let v = value("anchor")?;
            let (prev, next) = (k.attribute("previous").map_or(Ok(v), |_| value("previous"))?, k.attribute("next").map_or(Ok(v), |_| value("next"))?);
            keys.push(((time / UNIT as f32).round().max(0.0) as u32, v, prev, next));
        }
        keys.sort_by_key(|k| k.0);
        keys.dedup_by_key(|k| k.0);
        tracks.push((id, keys));
    }
    Ok(())
}

fn color_prop(id: &str) -> bool {
    matches!(id, "Alpha_Amount" | "Brightness_Amount" | "Tint_Amount" | "Tint_Color") || ADVANCED.iter().any(|&(p, o)| id == p || id == o)
}

/// The color effect keyframes the color properties make together, at every frame one is keyed.
fn read_color(tracks: &[(&str, Vec<XmlKey>)]) -> Vec<(u32, ColorTransform)> {
    let colors: Vec<(&str, Vec<(u32, f32)>)> =
        tracks.iter().filter(|t| color_prop(t.0)).map(|(id, keys)| (*id, keys.iter().map(|k| (k.0, k.1)).collect())).collect();
    let frames: BTreeSet<u32> = colors.iter().flat_map(|t| t.1.iter().map(|k| k.0)).collect();
    let at = |f: u32, id: &str| colors.iter().find(|t| t.0 == id).and_then(|t| sample(&t.1, f as f32, 0.0, |a, b, s| a + (b - a) * s));
    frames
        .into_iter()
        .map(|f| {
            let mut c = ColorTransform::IDENTITY;
            for (ch, (pct, offset)) in ADVANCED.iter().enumerate() {
                c.mult[ch] = at(f, pct).map_or(1.0, |v| v / 100.0);
                c.add[ch] = at(f, offset).map_or(0.0, |v| v / 255.0);
            }
            if let Some(a) = at(f, "Alpha_Amount") {
                c.mult[3] = a / 100.0;
            }
            if let Some(b) = at(f, "Brightness_Amount").map(|b| b / 100.0) {
                (0..3).for_each(|ch| (c.mult[ch], c.add[ch]) = (1.0 - b.abs(), b.max(0.0)));
            }
            if let Some(t) = at(f, "Tint_Amount").map(|t| t / 100.0) {
                let rgb = at(f, "Tint_Color").map_or(0, |v| v as u32);
                for (ch, shift) in [16, 8, 0].into_iter().enumerate() {
                    (c.mult[ch], c.add[ch]) = (1.0 - t, ((rgb >> shift) & 0xFF) as f32 / 255.0 * t);
                }
            }
            (f, c)
        })
        .collect()
}

/// A hash of the tween as it plays over a span `duration` frames long, to tell it changed.
fn fingerprint(m: &Motion, duration: u32) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    format!("{:?} {duration}", Motion { read: 0, ..m.clone() }).hash(&mut h);
    // Never 0, which `read` keeps for a tween not read from a file.
    h.finish().max(1)
}

/// Whether saving `f` can write back the motion XML its `extra` holds as read.
fn as_read(f: &Frame) -> bool {
    matches!(&f.tween, Tween::Motion(m) if m.read != 0 && m.read == fingerprint(m, f.duration))
        && f.extra.before.iter().chain(&f.extra.after).any(|x| is_motion_xml(x))
}

fn is_motion_xml(xml: &str) -> bool {
    xml.trim_start().starts_with("<motionObjectXML")
}

/// What saving writes of `f.extra`: without the motion XML it was read with once
/// `f` is no longer that tween as read (changed, or removed), for `write` to put ours.
pub(crate) fn saved_extra(f: &Frame) -> Cow<'_, Extra> {
    let stale = match &f.tween {
        Tween::Motion(_) => !as_read(f),
        // A motion XML the model couldn't read stays with its tween, kept as it was.
        Tween::Unsupported(_) => false,
        _ => true,
    };
    if !stale || !f.extra.before.iter().chain(&f.extra.after).any(|x| is_motion_xml(x)) {
        return Cow::Borrowed(&f.extra);
    }
    let mut extra = f.extra.clone();
    extra.before.retain(|x| !is_motion_xml(x));
    extra.after.retain(|x| !is_motion_xml(x));
    Cow::Owned(extra)
}

/// Writes `f`'s motion XML, unless the one it was read with goes back (`saved_extra`).
pub(crate) fn write(x: &mut Xml, f: &Frame) {
    let Tween::Motion(m) = &f.tween else { return };
    if as_read(f) {
        return;
    }
    let num = |v: f32| (v + 0.0).to_string();
    let key = |x: &mut Xml, frame: u32, v: f32, prev: f32, next: f32| {
        let attrs = [("anchor", v), ("next", next), ("previous", prev)].map(|(k, v)| (k, format!("0,{}", num(v))));
        let attrs: Vec<_> = attrs.into_iter().chain([("roving", "0".to_string()), ("timevalue", (frame * UNIT).to_string())]).collect();
        x.el("Keyframe", &attrs, |_| {});
    };
    // Every property Animate lists, each keyed at frame 0 at least.
    let property = |x: &mut Xml, id: &str, keys: &[XmlKey], neutral: f32| {
        let attrs = [("enabled", "1"), ("id", id), ("ignoreTimeMap", "0"), ("readonly", "0"), ("visible", "1")].map(|(k, v)| (k, v.to_string()));
        x.el("Property", &attrs, |x| match keys {
            [] => key(x, 0, neutral, neutral, neutral),
            _ => keys.iter().for_each(|&(f, v, prev, next)| key(x, f, v, prev, next)),
        });
    };
    let flat = |keys: &[(u32, f32)]| keys.iter().map(|&(f, v)| (f, v, v, v)).collect::<Vec<XmlKey>>();
    let kept = |x: &mut Xml, container: &str| m.kept.iter().filter(|k| k.0 == container).for_each(|k| x.raw(&k.1));
    let id = |id: &str| [("id", id.to_string())];
    let core = [("TimeScale", m.time_scale.to_string()), ("Version", "1".into()), ("duration", (f.duration * UNIT).to_string())];
    x.el("motionObjectXML", &[], |x| {
        x.el("AnimationCore", &core, |x| {
            x.el("TimeMap", &[("strength", num(m.ease * 100.0)), ("type", "Quadratic".into())], |_| {});
            x.el("metadata", &[], |x| {
                x.el("names", &[], |x| x.el("name", &[("langID", "en_US".into()), ("value", String::new())], |_| {}));
                let settings = [("orientToPath", if m.orient { "1" } else { "0" }), ("xformPtXOffsetPct", "0.5"), ("xformPtYOffsetPct", "0.5"), ("xformPtZOffsetPixels", "0")];
                x.el("Settings", &settings.map(|(k, v)| (k, v.to_string())), |_| {});
            });
            x.el("PropertyContainer", &id("headContainer"), |x| {
                x.el("PropertyContainer", &id("Basic_Motion"), |x| {
                    let axis = |pick: fn(Point) -> f32| m.path.iter().map(|k| (k.frame, pick(k.at), pick(k.prev), pick(k.next))).collect::<Vec<XmlKey>>();
                    property(x, "Motion_X", &axis(|p| p.0), 0.0);
                    property(x, "Motion_Y", &axis(|p| p.1), 0.0);
                    property(x, "Rotation_Z", &flat(&m.props[ROTATION]), 0.0);
                    kept(x, "Basic_Motion");
                });
                x.el("PropertyContainer", &id("Transformation"), |x| {
                    for i in [SKEW_X, SKEW_Y, SCALE_X, SCALE_Y] {
                        property(x, PROPS[i].1, &flat(&m.props[i]), NEUTRAL[i]);
                    }
                    kept(x, "Transformation");
                });
                x.el("PropertyContainer", &id("Colors"), |x| {
                    if !m.color.is_empty() {
                        x.el("PropertyContainer", &id("Advanced_Color"), |x| {
                            for (ch, (pct, offset)) in ADVANCED.iter().enumerate() {
                                let keys = |v: fn(&ColorTransform, usize) -> f32| flat(&m.color.iter().map(|(f, c)| (*f, v(c, ch))).collect::<Vec<_>>());
                                property(x, pct, &keys(|c, ch| c.mult[ch] * 100.0), 100.0);
                                property(x, offset, &keys(|c, ch| c.add[ch] * 255.0), 0.0);
                            }
                        });
                    }
                    kept(x, "Colors");
                });
                if !m.kept.iter().any(|k| k.0 == "headContainer") {
                    x.el("PropertyContainer", &id("Filters"), |_| {});
                }
                kept(x, "headContainer");
            });
        });
    });
}

fn child<'a, 'i>(n: Node<'a, 'i>, name: &str) -> Option<Node<'a, 'i>> {
    n.children().find(|c| c.tag_name().name() == name)
}

fn source(n: Node) -> String {
    n.document().input_text()[n.range()].into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hit::ElementRef;

    fn close(a: Point, b: Point) -> bool {
        (a.0 - b.0).abs() < 1e-3 && (a.1 - b.1).abs() < 1e-3
    }

    fn instance(matrix: Matrix) -> Instance {
        Instance {
            symbol: "Ball".into(),
            kind: SymbolKind::Graphic,
            matrix,
            pivot: (10.0, 10.0),
            color: ColorTransform::IDENTITY,
            looping: Loop::Loop,
            first_frame: 0,
            extra: Extra::default(),
        }
    }

    /// A layer with one 11-frame motion tween span moving a 20 px instance from (100, 100).
    fn tweened() -> Layer {
        let el = Element::Instance(instance(Matrix::translate(100.0, 100.0)));
        let key = Frame { index: 0, duration: 1, tween: Tween::None, elements: vec![el], extra: Extra::default() };
        let mut l = Layer { name: "Layer_1".into(), kind: LayerKind::Normal, frames: vec![key], extra: Extra::default() };
        assert!(l.create_motion_tween(0, 11.0), "a lone instance tweens");
        assert_eq!(l.frames[0].duration, 11, "a one-frame span grows to a second");
        l
    }

    fn shown(l: &Layer, frame: u32) -> Instance {
        let key = &l.frames[l.keyframe_at(frame).unwrap()];
        match target_at(key, frame - key.index) {
            Some(Element::Instance(i)) => i,
            other => panic!("{other:?}"),
        }
    }

    fn tl(l: Layer) -> Timeline {
        Timeline { name: "Scene 1".into(), layers: vec![l], extra: Extra::default() }
    }

    const TARGET: ElementRef = ElementRef { layer: 0, keyframe: 0, element: 0 };

    #[test]
    fn auto_keyframes_move_turn_and_scale() {
        let mut t = tl(tweened());
        // At frame 10 the instance is dragged 100 px right: keys at 0 and 10, halfway at 5.
        edit_shown(&mut t, TARGET, 10, |el| crate::hit::translate(el, 100.0, 0.0));
        let l = &t.layers[0];
        let Tween::Motion(m) = &l.frames[0].tween else { panic!("motion") };
        assert_eq!(m.frames(Kind::Position), [0, 10]);
        assert_eq!(l.frames[0].elements[0], Element::Instance(instance(Matrix::translate(100.0, 100.0))), "the target stays");
        assert!(close((shown(l, 5).matrix.tx, shown(l, 5).matrix.ty), (150.0, 100.0)));
        assert!(close((shown(l, 10).matrix.tx, 0.0), (200.0, 0.0)));

        // At frame 4, a quarter turn about the transformation point, there at (150, 110).
        let turn = Matrix::rotate(PI / 2.0).about((150.0, 110.0));
        edit_shown(&mut t, TARGET, 4, |el| crate::transform::transform(el, &turn));
        let l = &t.layers[0];
        let Tween::Motion(m) = &l.frames[0].tween else { panic!("motion") };
        assert_eq!((m.frames(Kind::Rotation), m.frames(Kind::Position)), (vec![0, 4], vec![0, 10]), "turning about the point doesn't move it");
        assert!((m.props[ROTATION][1].1 - 90.0).abs() < 1e-3);
        let at2 = shown(l, 2).matrix;
        assert!((at2.b - (PI / 4.0).sin()).abs() < 1e-4, "an eighth turn halfway: {at2:?}");
        // At frame 10, twice the size about the point, there at (210, 110).
        let grow = Matrix::scale(2.0, 2.0).about((210.0, 110.0));
        edit_shown(&mut t, TARGET, 10, |el| crate::transform::transform(el, &grow));
        let Tween::Motion(m) = &t.layers[0].frames[0].tween else { panic!("motion") };
        assert_eq!(m.props[SCALE_X].iter().map(|k| (k.0, k.1.round())).collect::<Vec<_>>(), [(0, 100.0), (10, 200.0)]);
        assert_eq!(m.frames(Kind::Position), [0, 10]);
        assert!(close(shown(&t.layers[0], 10).matrix.apply((10.0, 10.0)), (210.0, 110.0)), "the point stays put");

        // A color effect keys color; outside the span, or on another element, edits go straight in.
        let half = ColorTransform { mult: [1.0, 1.0, 1.0, 0.5], add: [0.0; 4] };
        edit_shown(&mut t, TARGET, 6, |el| if let Element::Instance(i) = el { i.color = half });
        assert_eq!(shown(&t.layers[0], 3).color.mult[3], 0.75);
        edit_shown(&mut t, TARGET, 20, |el| crate::hit::translate(el, 0.0, 5.0));
        assert_eq!(t.layers[0].frames[0].elements[0], Element::Instance(instance(Matrix::translate(100.0, 105.0))));
    }

    #[test]
    fn path_curves_ease_and_orient() {
        let mut m = Motion::new(24.0);
        m.set_point(10, (100.0, 0.0));
        // Bent at frame 5 by 40 px down: the path passes there, curving, at an even speed.
        let from = m.clone();
        m.bend(&from, 5, (0.0, 40.0));
        let (mid, heading) = m.along(5.0);
        assert!(close(mid, (50.0, 40.0)), "{mid:?}");
        assert!(heading.abs() < 1e-3, "level at the top of the bend: {heading}");
        let (p2, p8) = (m.along(2.0).0, m.along(8.0).0);
        assert!((p2.1 - p8.1).abs() < 1e-2 && (p2.0 + p8.0 - 100.0).abs() < 1e-2, "symmetric: {p2:?} {p8:?}");
        // Cutting the curve keeps it, to the 32 chords its length is measured on.
        let mut cut = m.clone();
        cut.cut(3);
        for f in [1.0, 3.0, 5.0, 7.0, 9.0] {
            let (a, b) = (cut.along(f).0, m.along(f).0);
            assert!((a.0 - b.0).hypot(a.1 - b.1) < 0.05, "frame {f}: {a:?} vs {b:?}");
        }

        // Orient to path: the target turns with the path's heading.
        m.orient = true;
        let at = |f: u32| m.matrix(&Matrix::IDENTITY, (0.0, 0.0), f, 11);
        let (start, top) = (at(0), at(5));
        assert!(start.b.abs() < 1e-6 && (top.b + m.along(0.0).1.sin()).abs() < 1e-3, "turned back level at the top: {top:?}");

        // Ease out: ahead of the plain tween early on.
        let mut eased = Motion::new(24.0);
        eased.set_point(10, (100.0, 0.0));
        eased.ease = 1.0;
        assert!((eased.along(5.0).0.0 - 75.0).abs() < 1e-3);
        // A clockwise turn spread over the span: halfway, half a turn.
        eased.ease = 0.0;
        eased.rotate = Rotate::Cw(1);
        assert!((eased.matrix(&Matrix::IDENTITY, (0.0, 0.0), 5, 11).a + 1.0).abs() < 1e-4);
    }

    #[test]
    fn splitting_keeps_what_shows() {
        // A curved path, a turn keyed at frame 10, and a whole turn over the span.
        let mut t = tl(tweened());
        edit_shown(&mut t, TARGET, 10, |el| crate::hit::translate(el, 100.0, 50.0));
        edit_shown(&mut t, TARGET, 10, |el| crate::transform::transform(el, &Matrix::rotate(1.0).about((210.0, 160.0))));
        if let Tween::Motion(m) = &mut t.layers[0].frames[0].tween {
            let from = (**m).clone();
            m.bend(&from, 5, (30.0, -20.0));
            m.rotate = Rotate::Ccw(1);
        }
        let mut l = t.layers[0].clone();
        let before: Vec<Matrix> = (0..11).map(|f| shown(&l, f).matrix).collect();
        // F6 inside the span keys its properties; pasting frames over its middle cuts it in three.
        l.convert_to_keyframes(4, 1, false);
        assert_eq!(l.frames.len(), 1, "inside a motion tween, F6 keys properties");
        let clip = l.copy_frames(3, 5);
        assert!(matches!(clip[0].tween, Tween::Motion(_)));
        l.paste_frames(3, 5, &clip, false);
        assert_eq!(l.frames.iter().map(|k| (k.index, k.duration)).collect::<Vec<_>>(), [(0, 3), (3, 5), (8, 3)]);
        for (f, want) in before.iter().enumerate() {
            let got = shown(&l, f as u32).matrix;
            let same = [got.a - want.a, got.b - want.b, got.c - want.c, got.d - want.d].iter().all(|d| d.abs() < 1e-3)
                && (got.tx - want.tx).hypot(got.ty - want.ty) < 0.05;
            assert!(same, "frame {f}: {got:?} was {want:?}");
        }
    }

    #[test]
    fn frames_in_and_out_of_a_span() {
        let mut t = tl(tweened());
        edit_shown(&mut t, TARGET, 6, |el| crate::hit::translate(el, 60.0, 0.0));
        let l = &mut t.layers[0];
        l.insert_frames(2, 3);
        let Tween::Motion(m) = &l.frames[0].tween else { panic!("motion") };
        assert_eq!((l.frames[0].duration, m.frames(Kind::Position)), (14, vec![0, 9]));
        // The keyframe at 0 goes with the frames: the new first frame is keyed where frame 2 was.
        l.remove_frames(0, 2);
        let Tween::Motion(m) = &l.frames[0].tween else { panic!("motion") };
        assert_eq!((l.frames[0].duration, m.frames(Kind::Position)), (12, vec![0, 7]));
        // F6 past the end stretches the span to a keyframe there.
        assert!(l.key_motion(15));
        let Tween::Motion(m) = &l.frames[0].tween else { panic!("motion") };
        assert_eq!((l.frames[0].duration, m.frames(Kind::Rotation)), (16, vec![0, 15]));
    }

    #[test]
    fn removing_the_tween_keeps_its_first_frame() {
        // Dragged and faded on the span's first frame, which keys it there, then the tween removed.
        let mut t = tl(tweened());
        edit_shown(&mut t, TARGET, 0, |el| crate::hit::translate(el, 30.0, 0.0));
        edit_shown(&mut t, TARGET, 0, |el| if let Element::Instance(i) = el { i.color.mult[3] = 0.5 });
        let first = shown(&t.layers[0], 0);
        let mut shape = t.layers[0].clone();
        t.layers[0].remove_tween(0);
        assert_eq!(t.layers[0].frames[0].elements, [Element::Instance(first.clone())], "it stays where it was put");
        // So it does when a shape tween takes the motion tween's place.
        shape.create_shape_tween(0);
        assert_eq!(shape.frames[0].elements, [Element::Instance(first)]);
    }

    #[test]
    fn copied_frames_of_a_spinning_span_turn_as_they_did() {
        // A whole clockwise turn over frames 0..=10; its first five frames copied and pasted.
        let mut l = tweened();
        if let Tween::Motion(m) = &mut l.frames[0].tween {
            m.rotate = Rotate::Cw(1);
        }
        let clip = l.copy_frames(0, 5);
        let mut pasted = Layer { frames: Vec::new(), ..l.clone() };
        pasted.paste_frames(0, 5, &clip, true);
        for f in 0..5 {
            let (a, b) = (shown(&l, f).matrix, shown(&pasted, f).matrix);
            assert!((a.a - b.a).abs() < 1e-4 && (a.b - b.b).abs() < 1e-4, "frame {f}: {b:?} was {a:?}");
        }
    }

    #[test]
    fn removing_frames_keeps_the_rest_moving() {
        let mut t = tl(tweened());
        edit_shown(&mut t, TARGET, 10, |el| crate::hit::translate(el, 100.0, 0.0));
        let xs = |l: &Layer, frames: std::ops::Range<u32>| frames.map(|f| shown(l, f).matrix.tx).collect::<Vec<_>>();
        let same = |a: &[f32], b: &[f32]| a.len() == b.len() && a.iter().zip(b).all(|(a, b)| (a - b).abs() < 1e-3);
        let before = xs(&t.layers[0], 0..11);
        // The first three frames go, keyframe and all: the rest carry on from where they were.
        let mut head = t.layers[0].clone();
        head.remove_frames(0, 3);
        assert!(same(&xs(&head, 0..8), &before[3..]), "{:?}", xs(&head, 0..8));
        // The last three go, with the keyframe at frame 10: the rest stop where they were.
        let mut tail = t.layers[0].clone();
        tail.remove_frames(8, 3);
        assert!(same(&xs(&tail, 0..8), &before[..8]), "{:?}", xs(&tail, 0..8));
    }

    #[test]
    fn animates_motion_xml() {
        // As Animate saves a 15-frame tween at 30 fps: a turn and a move to (40, -10) at frame 14,
        // half alpha at frame 7, and a depth it keeps.
        let xml = r#"<DOMFrame index="0" duration="15" tweenType="motion object" motionTweenRotate="none" isMotionObject="true">
  <motionObjectXML>
    <AnimationCore TimeScale="30000" Version="1" duration="15000">
      <TimeMap strength="-50" type="Quadratic"/>
      <metadata><names><name langID="en_US" value=""/></names><Settings orientToPath="1" xformPtXOffsetPct="0.5" xformPtYOffsetPct="0.5" xformPtZOffsetPixels="0"/></metadata>
      <PropertyContainer id="headContainer">
        <PropertyContainer id="Basic_Motion">
          <Property enabled="1" id="Motion_X" ignoreTimeMap="0" readonly="0" visible="1">
            <Keyframe anchor="0,0" next="0,0" previous="0,0" roving="0" timevalue="0"/>
            <Keyframe anchor="0,40" next="0,40" previous="0,40" roving="0" timevalue="14000"/>
          </Property>
          <Property enabled="1" id="Motion_Y" ignoreTimeMap="0" readonly="0" visible="1">
            <Keyframe anchor="0,0" next="0,0" previous="0,0" roving="0" timevalue="0"/>
            <Keyframe anchor="0,-10" next="0,-10" previous="0,-10" roving="0" timevalue="14000"/>
          </Property>
          <Property enabled="1" id="Rotation_Z" ignoreTimeMap="0" readonly="0" visible="1">
            <Keyframe anchor="0,0" next="0,0" previous="0,0" roving="0" timevalue="0"/>
            <Keyframe anchor="0,360" next="0,360" previous="0,360" roving="0" timevalue="14000"/>
          </Property>
          <Property enabled="1" id="Depth" ignoreTimeMap="0" readonly="0" visible="1">
            <Keyframe anchor="0,0" next="0,0" previous="0,0" roving="0" timevalue="0"/>
          </Property>
        </PropertyContainer>
        <PropertyContainer id="Transformation">
          <Property enabled="1" id="Scale_X" ignoreTimeMap="0" readonly="0" visible="1">
            <Keyframe anchor="0,100" next="0,100" previous="0,100" roving="0" timevalue="0"/>
          </Property>
        </PropertyContainer>
        <PropertyContainer id="Colors">
          <PropertyContainer id="Alpha_Color">
            <Property enabled="1" id="Alpha_Amount" ignoreTimeMap="0" readonly="0" visible="1">
              <Keyframe anchor="0,100" next="0,100" previous="0,100" roving="0" timevalue="0"/>
              <Keyframe anchor="0,50" next="0,50" previous="0,50" roving="0" timevalue="7000"/>
            </Property>
          </PropertyContainer>
        </PropertyContainer>
        <PropertyContainer id="Filters"/>
      </PropertyContainer>
    </AnimationCore>
  </motionObjectXML>
  <elements/>
</DOMFrame>"#;
        let doc = roxmltree::Document::parse(xml).unwrap();
        let m = read(doc.root_element(), 15).unwrap();
        assert_eq!((m.time_scale, m.ease, m.orient, m.rotate), (30000, -0.5, true, Rotate::Auto));
        assert_eq!(m.path, [PathKey::still(0, (0.0, 0.0)), PathKey::still(14, (40.0, -10.0))]);
        assert_eq!(m.props[ROTATION], [(0, 0.0), (14, 360.0)]);
        assert!(m.props[SCALE_X].is_empty(), "keyed at 100% only: no change");
        assert_eq!(m.color.iter().map(|(f, c)| (*f, c.mult[3])).collect::<Vec<_>>(), [(0, 1.0), (7, 0.5)]);
        assert_eq!(m.kept.iter().map(|k| k.0.as_str()).collect::<Vec<_>>(), ["Basic_Motion"], "Depth stays; empty Filters holds nothing");
        assert_ne!(m.read, 0);
    }
}
