//! Eases and motion tweens.
//!
//! * [`Ease`]: named presets (Quad, Cubic, …, Back, Bounce, Elastic) and
//!   custom Bezier curves edited in the ease graph. Classic and shape tweens
//!   take one per span; motion tweens take one per property.
//! * [`Motion`]: Animate-style motion tween: one object on a tween span with
//!   *property keyframes* (position on an editable motion path, rotation,
//!   skew, scale, alpha, brightness). Evaluated per frame, never baked.

use crate::geom::{pt, Decomposed, Matrix, Pt};
use serde::{Deserialize, Serialize};
use std::f32::consts::PI;

// ------------------------------------------------------------------ eases

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EaseFamily {
    Quad,
    Cubic,
    Quart,
    Quint,
    Sine,
    Circ,
    Back,
    Bounce,
    Elastic,
    /// Fast in the middle, slow at both ends and back again ("Stop and Start").
    StopStart,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EaseDir {
    In,
    Out,
    InOut,
}

impl EaseFamily {
    pub const ALL: [EaseFamily; 10] = [
        EaseFamily::Quad,
        EaseFamily::Cubic,
        EaseFamily::Quart,
        EaseFamily::Quint,
        EaseFamily::Sine,
        EaseFamily::Circ,
        EaseFamily::Back,
        EaseFamily::Bounce,
        EaseFamily::Elastic,
        EaseFamily::StopStart,
    ];
    pub fn name(self) -> &'static str {
        match self {
            EaseFamily::Quad => "Quad",
            EaseFamily::Cubic => "Cubic",
            EaseFamily::Quart => "Quart",
            EaseFamily::Quint => "Quint",
            EaseFamily::Sine => "Sine",
            EaseFamily::Circ => "Circ",
            EaseFamily::Back => "Back",
            EaseFamily::Bounce => "Bounce",
            EaseFamily::Elastic => "Elastic",
            EaseFamily::StopStart => "Stop and Start",
        }
    }
    /// Ease-in shape (t = 0..1).
    fn ease_in(self, t: f32) -> f32 {
        match self {
            EaseFamily::Quad => t * t,
            EaseFamily::Cubic => t * t * t,
            EaseFamily::Quart => t.powi(4),
            EaseFamily::Quint => t.powi(5),
            EaseFamily::Sine => 1.0 - (t * PI / 2.0).cos(),
            EaseFamily::Circ => 1.0 - (1.0 - t * t).max(0.0).sqrt(),
            EaseFamily::Back => {
                let s = 1.70158;
                t * t * ((s + 1.0) * t - s)
            }
            EaseFamily::Bounce => 1.0 - bounce_out(1.0 - t),
            EaseFamily::Elastic => {
                if t <= 0.0 || t >= 1.0 {
                    return t;
                }
                let p = 0.3;
                -(2f32.powf(10.0 * (t - 1.0))) * ((t - 1.0 - p / 4.0) * 2.0 * PI / p).sin()
            }
            EaseFamily::StopStart => {
                // slow-fast-slow twice: a smoothstep on each half
                let h = |u: f32| u * u * (3.0 - 2.0 * u);
                if t < 0.5 {
                    h(t * 2.0) / 2.0
                } else {
                    0.5 + h(t * 2.0 - 1.0) / 2.0
                }
            }
        }
    }
}

fn bounce_out(t: f32) -> f32 {
    let (n, d) = (7.5625, 2.75);
    if t < 1.0 / d {
        n * t * t
    } else if t < 2.0 / d {
        let t = t - 1.5 / d;
        n * t * t + 0.75
    } else if t < 2.5 / d {
        let t = t - 2.25 / d;
        n * t * t + 0.9375
    } else {
        let t = t - 2.625 / d;
        n * t * t + 0.984375
    }
}

impl EaseDir {
    pub const ALL: [EaseDir; 3] = [EaseDir::In, EaseDir::Out, EaseDir::InOut];
    pub fn name(self) -> &'static str {
        match self {
            EaseDir::In => "In",
            EaseDir::Out => "Out",
            EaseDir::InOut => "In Out",
        }
    }
}

/// An anchor of a custom ease curve. `x` = time 0..1, `y` = progress (may
/// overshoot). Handles are relative to the anchor.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CurvePt {
    pub x: f32,
    pub y: f32,
    #[serde(default)]
    pub h_in: Pt,
    #[serde(default)]
    pub h_out: Pt,
}

impl CurvePt {
    pub fn corner(x: f32, y: f32) -> Self {
        CurvePt {
            x,
            y,
            h_in: Pt::default(),
            h_out: Pt::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Ease {
    Preset {
        family: EaseFamily,
        dir: EaseDir,
    },
    /// Bezier curve from (0,0) to (1,1); first and last anchors keep their x.
    Custom {
        points: Vec<CurvePt>,
    },
}

impl Ease {
    pub fn preset(family: EaseFamily, dir: EaseDir) -> Self {
        Ease::Preset { family, dir }
    }
    /// A straight (linear) custom curve, ready to edit.
    pub fn linear_custom() -> Self {
        Ease::Custom {
            points: vec![
                CurvePt {
                    x: 0.0,
                    y: 0.0,
                    h_in: Pt::default(),
                    h_out: pt(0.33, 0.33),
                },
                CurvePt {
                    x: 1.0,
                    y: 1.0,
                    h_in: pt(-0.33, -0.33),
                    h_out: Pt::default(),
                },
            ],
        }
    }
    /// Turn any ease into an editable custom curve that approximates it.
    pub fn to_custom(&self) -> Ease {
        match self {
            Ease::Custom { .. } => self.clone(),
            Ease::Preset { .. } => Ease::fit(|t| self.apply(t)),
        }
    }
    /// An editable custom curve approximating any easing function.
    pub fn fit(f: impl Fn(f32) -> f32) -> Ease {
        let n = 8;
        let dx = 1.0 / n as f32;
        // derivative from a fine central difference
        let slope = |x: f32| {
            let h = 1e-3;
            let (a, b) = ((x - h).max(0.0), (x + h).min(1.0));
            (f(b) - f(a)) / (b - a)
        };
        let points = (0..=n)
            .map(|i| {
                let x = i as f32 * dx;
                let (s, hx) = (slope(x), dx / 3.0);
                CurvePt {
                    x,
                    y: f(x),
                    h_in: if i == 0 {
                        Pt::default()
                    } else {
                        pt(-hx, -hx * s)
                    },
                    h_out: if i == n {
                        Pt::default()
                    } else {
                        pt(hx, hx * s)
                    },
                }
            })
            .collect();
        Ease::Custom { points }
    }
    /// The same ease played backwards (Reverse Frames): In ↔ Out.
    pub fn reversed(&self) -> Ease {
        match self {
            Ease::Preset { family, dir } => Ease::Preset {
                family: *family,
                dir: match dir {
                    EaseDir::In => EaseDir::Out,
                    EaseDir::Out => EaseDir::In,
                    EaseDir::InOut => EaseDir::InOut,
                },
            },
            Ease::Custom { points } => Ease::Custom {
                points: points
                    .iter()
                    .rev()
                    .map(|p| CurvePt {
                        x: 1.0 - p.x,
                        y: 1.0 - p.y,
                        h_in: pt(-p.h_out.x, -p.h_out.y),
                        h_out: pt(-p.h_in.x, -p.h_in.y),
                    })
                    .collect(),
            },
        }
    }
    pub fn name(&self) -> String {
        match self {
            Ease::Preset { family, dir } => format!("{} {}", family.name(), dir.name()),
            Ease::Custom { .. } => "Custom".into(),
        }
    }
    /// Look a preset up by name ("Quad In", "bounce out", "Stop and Start InOut").
    pub fn from_name(s: &str) -> Option<Ease> {
        let low = s.to_ascii_lowercase().replace(['-', '_'], " ");
        let low = low.trim();
        if low == "custom" {
            return Some(Ease::linear_custom());
        }
        for f in EaseFamily::ALL {
            let fname = f.name().to_ascii_lowercase();
            if let Some(rest) = low.strip_prefix(&fname) {
                let r = rest.trim().replace(' ', "");
                let dir = match r.as_str() {
                    "in" => EaseDir::In,
                    "out" => EaseDir::Out,
                    "inout" | "" => EaseDir::InOut,
                    _ => continue,
                };
                return Some(Ease::preset(f, dir));
            }
        }
        None
    }

    /// Map time (0..1) to progress.
    pub fn apply(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Ease::Preset { family, dir } => match dir {
                EaseDir::In => family.ease_in(t),
                EaseDir::Out => 1.0 - family.ease_in(1.0 - t),
                EaseDir::InOut => {
                    if t < 0.5 {
                        family.ease_in(t * 2.0) / 2.0
                    } else {
                        1.0 - family.ease_in((1.0 - t) * 2.0) / 2.0
                    }
                }
            },
            Ease::Custom { points } => eval_curve(points, t),
        }
    }
}

/// Cubic Bezier through the anchors, solved for x = t (x is kept monotonic by
/// clamping handles inside their segment).
pub fn eval_curve(points: &[CurvePt], t: f32) -> f32 {
    match points.len() {
        0 => return t,
        1 => return points[0].y,
        _ => {}
    }
    if t <= points[0].x {
        return points[0].y;
    }
    let last = points[points.len() - 1];
    if t >= last.x {
        return last.y;
    }
    let i = points
        .windows(2)
        .position(|w| t >= w[0].x && t <= w[1].x)
        .unwrap_or(0);
    let (a, b) = (points[i], points[i + 1]);
    let (p0, p1, p2, p3) = segment_ctrl(a, b);
    // bisection on x(s)
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    for _ in 0..40 {
        let mid = (lo + hi) / 2.0;
        if cubic(p0.x, p1.x, p2.x, p3.x, mid) < t {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    cubic(p0.y, p1.y, p2.y, p3.y, (lo + hi) / 2.0)
}

/// Control points of the segment between two anchors (handles clamped in x).
pub fn segment_ctrl(a: CurvePt, b: CurvePt) -> (Pt, Pt, Pt, Pt) {
    let w = (b.x - a.x).max(1e-6);
    let p0 = pt(a.x, a.y);
    let p3 = pt(b.x, b.y);
    let p1 = pt(a.x + a.h_out.x.clamp(0.0, w), a.y + a.h_out.y);
    let p2 = pt(b.x + b.h_in.x.clamp(-w, 0.0), b.y + b.h_in.y);
    (p0, p1, p2, p3)
}

fn cubic(a: f32, b: f32, c: f32, d: f32, s: f32) -> f32 {
    let u = 1.0 - s;
    u * u * u * a + 3.0 * u * u * s * b + 3.0 * u * s * s * c + s * s * s * d
}

/// Eased progress for a classic/shape tween span: the custom ease when set,
/// otherwise Animate's −100…100 simple ease.
pub fn tween_progress(t: f32, simple: f32, ease: Option<&Ease>) -> f32 {
    match ease {
        Some(e) => e.apply(t),
        None => crate::model::apply_ease(t, simple),
    }
}

// ------------------------------------------------------------ motion tween

/// Properties a motion tween animates (besides position).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Prop {
    X,
    Y,
    Rotation,
    Skew,
    ScaleX,
    ScaleY,
    Alpha,
    Brightness,
}

impl Prop {
    pub const ALL: [Prop; 8] = [
        Prop::X,
        Prop::Y,
        Prop::Rotation,
        Prop::Skew,
        Prop::ScaleX,
        Prop::ScaleY,
        Prop::Alpha,
        Prop::Brightness,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Prop::X => "X",
            Prop::Y => "Y",
            Prop::Rotation => "Rotation",
            Prop::Skew => "Skew",
            Prop::ScaleX => "Scale X",
            Prop::ScaleY => "Scale Y",
            Prop::Alpha => "Alpha",
            Prop::Brightness => "Brightness",
        }
    }
    pub fn unit(self) -> &'static str {
        match self {
            Prop::X | Prop::Y => "px",
            Prop::Rotation | Prop::Skew => "°",
            _ => "%",
        }
    }
    pub fn from_name(s: &str) -> Option<Prop> {
        let s = s.to_ascii_lowercase().replace([' ', '_'], "");
        Prop::ALL
            .into_iter()
            .find(|p| p.name().to_ascii_lowercase().replace(' ', "") == s)
    }
    /// Shown in the UI as percent for scale/alpha/brightness.
    pub fn display_scale(self) -> f32 {
        match self {
            Prop::ScaleX | Prop::ScaleY | Prop::Alpha | Prop::Brightness => 100.0,
            _ => 1.0,
        }
    }
}

/// A property keyframe. `frame` is relative to the tween span's start.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PropKey {
    pub frame: u32,
    pub value: f32,
    /// Smooth point (curved through, Catmull-Rom) vs corner point (linear).
    #[serde(default)]
    pub smooth: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Track {
    pub keys: Vec<PropKey>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ease: Option<Ease>,
}

impl Track {
    pub fn value_at(&self, f: f32) -> Option<f32> {
        let k = &self.keys;
        let first = k.first()?;
        if f <= first.frame as f32 {
            return Some(first.value);
        }
        let last = k[k.len() - 1];
        if f >= last.frame as f32 {
            return Some(last.value);
        }
        let i = k
            .windows(2)
            .position(|w| f >= w[0].frame as f32 && f <= w[1].frame as f32)?;
        let (a, b) = (k[i], k[i + 1]);
        let span = (b.frame - a.frame).max(1) as f32;
        let u = (f - a.frame as f32) / span;
        if !a.smooth && !b.smooth {
            return Some(a.value + (b.value - a.value) * u);
        }
        // cubic Hermite with Catmull-Rom tangents at smooth keys (per frame)
        let tangent = |j: usize, smooth: bool| -> f32 {
            if !smooth {
                // corner: aim at the neighbour on this segment
                return (b.value - a.value) / span;
            }
            let p = if j > 0 { k[j - 1] } else { k[j] };
            let n = if j + 1 < k.len() { k[j + 1] } else { k[j] };
            let df = (n.frame as f32 - p.frame as f32).max(1.0);
            (n.value - p.value) / df
        };
        let (m0, m1) = (tangent(i, a.smooth) * span, tangent(i + 1, b.smooth) * span);
        let (u2, u3) = (u * u, u * u * u);
        Some(
            (2.0 * u3 - 3.0 * u2 + 1.0) * a.value
                + (u3 - 2.0 * u2 + u) * m0
                + (-2.0 * u3 + 3.0 * u2) * b.value
                + (u3 - u2) * m1,
        )
    }
    /// Insert or replace the key at `frame`.
    pub fn set(&mut self, frame: u32, value: f32) {
        match self.keys.binary_search_by_key(&frame, |k| k.frame) {
            Ok(i) => self.keys[i].value = value,
            Err(i) => {
                let smooth = self
                    .keys
                    .get(i.saturating_sub(1))
                    .map(|k| k.smooth)
                    .unwrap_or(false);
                self.keys.insert(
                    i,
                    PropKey {
                        frame,
                        value,
                        smooth,
                    },
                );
            }
        }
    }
    pub fn remove(&mut self, frame: u32) -> bool {
        let n = self.keys.len();
        self.keys.retain(|k| k.frame != frame);
        n != self.keys.len()
    }
    pub fn has_key(&self, frame: u32) -> bool {
        self.keys.iter().any(|k| k.frame == frame)
    }
}

/// A point of the motion path: where the object is at `frame` (relative),
/// with spatial Bezier handles relative to the point.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PathKey {
    pub frame: u32,
    pub at: Pt,
    #[serde(default)]
    pub h_in: Pt,
    #[serde(default)]
    pub h_out: Pt,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Motion {
    /// Transformation point, in the object's local coordinates.
    pub pivot: Pt,
    /// Position keys = anchors of the motion path.
    pub path: Vec<PathKey>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path_ease: Option<Ease>,
    pub rotation: Track,
    pub skew: Track,
    pub scale_x: Track,
    pub scale_y: Track,
    pub alpha: Track,
    pub brightness: Track,
    /// Extra full turns added across the span (Properties ▸ Rotate).
    #[serde(default)]
    pub rotate_turns: i32,
}

/// The values a motion tween animates, as seen on one frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub pos: Pt,
    /// degrees
    pub rotation: f32,
    /// degrees (skew_x − rotation)
    pub skew: f32,
    pub scale_x: f32,
    pub scale_y: f32,
    pub alpha: f32,
    /// −1 (black) … 0 … 1 (white)
    pub brightness: f32,
}

impl Pose {
    pub fn get(&self, p: Prop) -> f32 {
        match p {
            Prop::X => self.pos.x,
            Prop::Y => self.pos.y,
            Prop::Rotation => self.rotation,
            Prop::Skew => self.skew,
            Prop::ScaleX => self.scale_x,
            Prop::ScaleY => self.scale_y,
            Prop::Alpha => self.alpha,
            Prop::Brightness => self.brightness,
        }
    }
    pub fn set(&mut self, p: Prop, v: f32) {
        match p {
            Prop::X => self.pos.x = v,
            Prop::Y => self.pos.y = v,
            Prop::Rotation => self.rotation = v,
            Prop::Skew => self.skew = v,
            Prop::ScaleX => self.scale_x = v,
            Prop::ScaleY => self.scale_y = v,
            Prop::Alpha => self.alpha = v,
            Prop::Brightness => self.brightness = v,
        }
    }
    /// Read the pose of an element (matrix about `pivot`, alpha, tint).
    pub fn of(el: &crate::model::Element, pivot: Pt) -> Pose {
        let d = el.matrix.decompose();
        Pose {
            pos: el.matrix.apply(pivot),
            rotation: d.skew_y.to_degrees(),
            skew: wrap_deg((d.skew_x - d.skew_y).to_degrees()),
            scale_x: d.scale_x,
            scale_y: d.scale_y,
            alpha: el.alpha,
            brightness: brightness_of(el.tint),
        }
    }
    /// Write this pose onto an element.
    pub fn apply(&self, el: &mut crate::model::Element, pivot: Pt) {
        let r = self.rotation.to_radians();
        let lin = Matrix::compose(&Decomposed {
            tx: 0.0,
            ty: 0.0,
            scale_x: self.scale_x,
            scale_y: self.scale_y,
            skew_x: r + self.skew.to_radians(),
            skew_y: r,
        });
        let m = Matrix::translate(self.pos.x, self.pos.y)
            .concat(&lin)
            .concat(&Matrix::translate(-pivot.x, -pivot.y));
        el.matrix = m;
        el.alpha = self.alpha.clamp(0.0, 1.0);
        el.tint = brightness_tint(self.brightness, el.tint);
    }
}

fn wrap_deg(d: f32) -> f32 {
    let mut d = d % 360.0;
    if d > 180.0 {
        d -= 360.0;
    }
    if d < -180.0 {
        d += 360.0;
    }
    d
}

/// Brightness as Animate shows it: a tint toward white (positive) or black.
pub fn brightness_of(tint: Option<(crate::model::Color, f32)>) -> f32 {
    match tint {
        Some((c, a)) if c.r == 255 && c.g == 255 && c.b == 255 => a,
        Some((c, a)) if c.r == 0 && c.g == 0 && c.b == 0 => -a,
        _ => 0.0,
    }
}
pub fn brightness_tint(
    b: f32,
    keep: Option<(crate::model::Color, f32)>,
) -> Option<(crate::model::Color, f32)> {
    use crate::model::Color;
    let is_bw = |t: Option<(Color, f32)>| matches!(t, Some((c, _)) if (c.r, c.g, c.b) == (255, 255, 255) || (c.r, c.g, c.b) == (0, 0, 0));
    if b.abs() < 1e-4 {
        return if is_bw(keep) { None } else { keep };
    }
    if !is_bw(keep) && keep.is_some() {
        return keep; // a real colour tint wins; brightness needs the tint slot
    }
    let b = b.clamp(-1.0, 1.0);
    Some((
        if b > 0.0 {
            Color::rgb(255, 255, 255)
        } else {
            Color::rgb(0, 0, 0)
        },
        b.abs(),
    ))
}

impl Motion {
    pub fn new(pivot: Pt) -> Self {
        Motion {
            pivot,
            ..Default::default()
        }
    }
    pub fn track(&self, p: Prop) -> Option<&Track> {
        Some(match p {
            Prop::X | Prop::Y => return None,
            Prop::Rotation => &self.rotation,
            Prop::Skew => &self.skew,
            Prop::ScaleX => &self.scale_x,
            Prop::ScaleY => &self.scale_y,
            Prop::Alpha => &self.alpha,
            Prop::Brightness => &self.brightness,
        })
    }
    pub fn track_mut(&mut self, p: Prop) -> Option<&mut Track> {
        Some(match p {
            Prop::X | Prop::Y => return None,
            Prop::Rotation => &mut self.rotation,
            Prop::Skew => &mut self.skew,
            Prop::ScaleX => &mut self.scale_x,
            Prop::ScaleY => &mut self.scale_y,
            Prop::Alpha => &mut self.alpha,
            Prop::Brightness => &mut self.brightness,
        })
    }
    /// The ease that applies to a property (its own, else the span's).
    pub fn ease_of<'a>(&'a self, p: Prop, span_ease: Option<&'a Ease>) -> Option<&'a Ease> {
        match p {
            Prop::X | Prop::Y => self.path_ease.as_ref().or(span_ease),
            _ => self.track(p).and_then(|t| t.ease.as_ref()).or(span_ease),
        }
    }
    /// Frames that hold any property keyframe (sorted, unique).
    pub fn key_frames(&self) -> Vec<u32> {
        let mut v: Vec<u32> = self.path.iter().map(|k| k.frame).collect();
        for p in Prop::ALL {
            if let Some(t) = self.track(p) {
                v.extend(t.keys.iter().map(|k| k.frame));
            }
        }
        v.sort();
        v.dedup();
        v
    }
    pub fn keys_of(&self, p: Prop) -> Vec<(u32, f32)> {
        match p {
            Prop::X => self.path.iter().map(|k| (k.frame, k.at.x)).collect(),
            Prop::Y => self.path.iter().map(|k| (k.frame, k.at.y)).collect(),
            _ => self
                .track(p)
                .map(|t| t.keys.iter().map(|k| (k.frame, k.value)).collect())
                .unwrap_or_default(),
        }
    }

    fn eased_frame(&self, local: f32, len: u32, ease: Option<&Ease>, simple: f32) -> f32 {
        let len = len.max(1) as f32 - 1.0;
        if len <= 0.0 {
            return local;
        }
        let t = (local / len).clamp(0.0, 1.0);
        tween_progress(t, simple, ease) * len
    }

    /// The pose at `local` frame of a span `len` frames long.
    pub fn pose_at(
        &self,
        base: &Pose,
        local: f32,
        len: u32,
        simple: f32,
        span_ease: Option<&Ease>,
    ) -> Pose {
        let mut p = *base;
        let fp = self.eased_frame(local, len, self.ease_of(Prop::X, span_ease), simple);
        if let Some(pos) = self.path_point(fp) {
            p.pos = pos;
        }
        for prop in [
            Prop::Rotation,
            Prop::Skew,
            Prop::ScaleX,
            Prop::ScaleY,
            Prop::Alpha,
            Prop::Brightness,
        ] {
            let t = self.track(prop).unwrap();
            let f = self.eased_frame(local, len, self.ease_of(prop, span_ease), simple);
            if let Some(v) = t.value_at(f) {
                p.set(prop, v);
            }
        }
        if self.rotate_turns != 0 && len > 1 {
            let f = self.eased_frame(local, len, self.ease_of(Prop::Rotation, span_ease), simple);
            p.rotation += 360.0 * self.rotate_turns as f32 * (f / (len - 1) as f32).clamp(0.0, 1.0);
        }
        p
    }

    /// Point on the motion path at (eased) frame `f`; None without position keys.
    pub fn path_point(&self, f: f32) -> Option<Pt> {
        let k = &self.path;
        let first = k.first()?;
        if f <= first.frame as f32 || k.len() == 1 {
            return Some(first.at);
        }
        let last = k[k.len() - 1];
        if f >= last.frame as f32 {
            return Some(last.at);
        }
        let i = k
            .windows(2)
            .position(|w| f >= w[0].frame as f32 && f <= w[1].frame as f32)?;
        let (a, b) = (k[i], k[i + 1]);
        let u = (f - a.frame as f32) / (b.frame - a.frame).max(1) as f32;
        Some(seg_point_uniform(a, b, u))
    }

    /// Path direction (degrees) at frame `f`, for Orient to Path.
    pub fn path_angle(&self, f: f32) -> Option<f32> {
        let a = self.path_point((f - 0.5).max(0.0))?;
        let b = self.path_point(f + 0.5)?;
        if a.dist(b) < 1e-4 {
            // at the ends: use the nearest segment's direction
            let k = &self.path;
            if k.len() < 2 {
                return None;
            }
            let (p, q) = if f <= k[0].frame as f32 {
                (k[0], k[1])
            } else {
                (k[k.len() - 2], k[k.len() - 1])
            };
            let d = pt(q.at.x - p.at.x, q.at.y - p.at.y);
            return Some(d.y.atan2(d.x).to_degrees());
        }
        Some((b.y - a.y).atan2(b.x - a.x).to_degrees())
    }

    /// The motion path as polyline segments for drawing (stage coordinates).
    pub fn path_polyline(&self, steps: usize) -> Vec<Pt> {
        let mut out = vec![];
        for w in self.path.windows(2) {
            for i in 0..=steps {
                out.push(seg_point(w[0], w[1], i as f32 / steps as f32));
            }
        }
        out
    }

    /// Set (or add) the position key at `frame`.
    pub fn set_pos(&mut self, frame: u32, at: Pt) {
        match self.path.binary_search_by_key(&frame, |k| k.frame) {
            Ok(i) => self.path[i].at = at,
            Err(i) => self.path.insert(
                i,
                PathKey {
                    frame,
                    at,
                    h_in: Pt::default(),
                    h_out: Pt::default(),
                },
            ),
        }
    }

    /// Record `pose` at `frame`: properties that differ from what the tween
    /// shows there get a key (the first key also pins the start value).
    pub fn record(&mut self, frame: u32, base: &Pose, before: &Pose, after: &Pose) {
        const EPS: f32 = 1e-3;
        if before.pos.dist(after.pos) > EPS {
            if self.path.is_empty() && frame != 0 {
                self.set_pos(0, base.pos);
            }
            if self.path.is_empty() && frame == 0 {
                // moving at the first frame with no path keys just moves the base
            } else {
                self.set_pos(frame, after.pos);
            }
        }
        for prop in [
            Prop::Rotation,
            Prop::Skew,
            Prop::ScaleX,
            Prop::ScaleY,
            Prop::Alpha,
            Prop::Brightness,
        ] {
            let (b, a) = (before.get(prop), after.get(prop));
            if (b - a).abs() <= EPS {
                continue;
            }
            let t = self.track_mut(prop).unwrap();
            if t.keys.is_empty() {
                if frame == 0 {
                    continue; // changes the base
                }
                t.set(0, base.get(prop));
            }
            let v = if prop == Prop::Rotation {
                b + wrap_deg(a - b)
            } else {
                a
            };
            t.set(frame, v);
        }
    }

    /// Set one property's key at `frame` (Motion Editor, scripts). `now` is
    /// what the tween shows at that frame, `base` the object's own pose.
    pub fn set_value(&mut self, prop: Prop, frame: u32, value: f32, base: &Pose, now: &Pose) {
        match prop {
            Prop::X | Prop::Y => {
                if self.path.is_empty() && frame != 0 {
                    self.set_pos(0, base.pos);
                }
                let mut at = self
                    .path
                    .iter()
                    .find(|k| k.frame == frame)
                    .map(|k| k.at)
                    .unwrap_or(now.pos);
                if prop == Prop::X {
                    at.x = value;
                } else {
                    at.y = value;
                }
                self.set_pos(frame, at);
            }
            _ => {
                let t = self.track_mut(prop).unwrap();
                if t.keys.is_empty() && frame != 0 {
                    t.set(0, base.get(prop));
                }
                t.set(frame, value);
            }
        }
    }
    /// Remove one property's key (position keys remove both X and Y).
    pub fn remove_value(&mut self, prop: Prop, frame: u32) -> bool {
        match prop {
            Prop::X | Prop::Y => {
                let n = self.path.len();
                self.path.retain(|k| k.frame != frame);
                n != self.path.len()
            }
            _ => self.track_mut(prop).unwrap().remove(frame),
        }
    }
    /// Move a key in time; refused (returns `from`) when `to` already has one.
    pub fn move_key(&mut self, prop: Prop, from: u32, to: u32) -> u32 {
        if from == to {
            return from;
        }
        match prop {
            Prop::X | Prop::Y => {
                if self.path.iter().any(|k| k.frame == to) {
                    return from;
                }
                if let Some(k) = self.path.iter_mut().find(|k| k.frame == from) {
                    k.frame = to;
                }
                self.path.sort_by_key(|k| k.frame);
            }
            _ => {
                let t = self.track_mut(prop).unwrap();
                if t.has_key(to) {
                    return from;
                }
                if let Some(k) = t.keys.iter_mut().find(|k| k.frame == from) {
                    k.frame = to;
                }
                t.keys.sort_by_key(|k| k.frame);
            }
        }
        to
    }
    pub fn set_smooth(&mut self, prop: Prop, frame: u32, smooth: bool) {
        if let Some(k) = self
            .track_mut(prop)
            .and_then(|t| t.keys.iter_mut().find(|k| k.frame == frame))
        {
            k.smooth = smooth;
        }
    }

    pub fn set_ease(&mut self, prop: Prop, ease: Option<Ease>) {
        match prop {
            Prop::X | Prop::Y => self.path_ease = ease,
            _ => self.track_mut(prop).unwrap().ease = ease,
        }
    }

    /// Keys for every property at `frame` (F6 inside a motion tween).
    pub fn key_all(&mut self, frame: u32, base: &Pose, now: &Pose) {
        if self.path.is_empty() {
            self.set_pos(0, base.pos);
        }
        self.set_pos(frame, now.pos);
        for prop in [
            Prop::Rotation,
            Prop::Skew,
            Prop::ScaleX,
            Prop::ScaleY,
            Prop::Alpha,
            Prop::Brightness,
        ] {
            let t = self.track_mut(prop).unwrap();
            if t.keys.is_empty() {
                t.set(0, base.get(prop));
            }
            t.set(frame, now.get(prop));
        }
    }

    /// Split at `at` (relative): `self` keeps the keys before it (plus `pose`
    /// as the closing value); the returned motion holds the rest, rebased.
    pub fn split(&mut self, at: u32, pose: &Pose) -> Motion {
        let mut tail = Motion {
            pivot: self.pivot,
            path_ease: self.path_ease.clone(),
            rotate_turns: 0,
            ..Default::default()
        };
        if !self.path.is_empty() {
            tail.path = self
                .path
                .iter()
                .filter(|k| k.frame > at)
                .map(|k| PathKey {
                    frame: k.frame - at,
                    ..*k
                })
                .collect();
            tail.path.insert(
                0,
                PathKey {
                    frame: 0,
                    at: pose.pos,
                    h_in: Pt::default(),
                    h_out: Pt::default(),
                },
            );
            self.path.retain(|k| k.frame < at);
            self.set_pos(at, pose.pos);
        }
        for prop in [
            Prop::Rotation,
            Prop::Skew,
            Prop::ScaleX,
            Prop::ScaleY,
            Prop::Alpha,
            Prop::Brightness,
        ] {
            let t = self.track_mut(prop).unwrap();
            let mut nt = Track {
                keys: vec![],
                ease: t.ease.clone(),
            };
            if !t.keys.is_empty() {
                nt.keys = t
                    .keys
                    .iter()
                    .filter(|k| k.frame > at)
                    .map(|k| PropKey {
                        frame: k.frame - at,
                        ..*k
                    })
                    .collect();
                nt.keys.insert(
                    0,
                    PropKey {
                        frame: 0,
                        value: pose.get(prop),
                        smooth: false,
                    },
                );
                t.keys.retain(|k| k.frame < at);
                t.set(at, pose.get(prop));
            }
            *tail.track_mut(prop).unwrap() = nt;
        }
        tail
    }

    /// Remove every property key at `frame` (not the first frame).
    pub fn clear_keys_at(&mut self, frame: u32) -> bool {
        let mut any = false;
        let n = self.path.len();
        self.path.retain(|k| k.frame != frame);
        any |= n != self.path.len();
        for prop in [
            Prop::Rotation,
            Prop::Skew,
            Prop::ScaleX,
            Prop::ScaleY,
            Prop::Alpha,
            Prop::Brightness,
        ] {
            any |= self.track_mut(prop).unwrap().remove(frame);
        }
        any
    }

    /// Keep keys inside a span of `len` frames, scaling them if `stretch`.
    pub fn fit(&mut self, old_len: u32, new_len: u32, stretch: bool) {
        let map = |f: u32| -> u32 {
            if stretch && old_len > 1 {
                ((f as f32) * (new_len.max(1) - 1) as f32 / (old_len - 1) as f32).round() as u32
            } else {
                f.min(new_len.saturating_sub(1))
            }
        };
        for k in &mut self.path {
            k.frame = map(k.frame);
        }
        self.path.dedup_by_key(|k| k.frame);
        for prop in [
            Prop::Rotation,
            Prop::Skew,
            Prop::ScaleX,
            Prop::ScaleY,
            Prop::Alpha,
            Prop::Brightness,
        ] {
            let t = self.track_mut(prop).unwrap();
            for k in &mut t.keys {
                k.frame = map(k.frame);
            }
            t.keys.dedup_by_key(|k| k.frame);
        }
    }

    /// Reverse the keys in time (Reverse Frames on a motion span).
    pub fn reverse(&mut self, len: u32) {
        let last = len.saturating_sub(1);
        for k in &mut self.path {
            k.frame = last.saturating_sub(k.frame);
            std::mem::swap(&mut k.h_in, &mut k.h_out);
        }
        self.path.reverse();
        for prop in [
            Prop::Rotation,
            Prop::Skew,
            Prop::ScaleX,
            Prop::ScaleY,
            Prop::Alpha,
            Prop::Brightness,
        ] {
            let t = self.track_mut(prop).unwrap();
            for k in &mut t.keys {
                k.frame = last.saturating_sub(k.frame);
            }
            t.keys.reverse();
        }
    }

    /// Move the whole path (dragging the object at frame 0 with a path).
    pub fn translate_path(&mut self, d: Pt) {
        for k in &mut self.path {
            k.at = pt(k.at.x + d.x, k.at.y + d.y);
        }
    }

    /// Bend the path segment nearest `grab` so it passes through `to`
    /// (Selection-tool drag on the motion path).
    pub fn bend(&mut self, seg: usize, u: f32, to: Pt) {
        if seg + 1 >= self.path.len() {
            return;
        }
        let (a, b) = (self.path[seg], self.path[seg + 1]);
        let cur = seg_point(a, b, u);
        let d = pt(to.x - cur.x, to.y - cur.y);
        // distribute the offset to the two inner control points (least squares
        // on the Bernstein weights at u)
        let w1 = 3.0 * (1.0 - u) * (1.0 - u) * u;
        let w2 = 3.0 * (1.0 - u) * u * u;
        let den = (w1 * w1 + w2 * w2).max(1e-4);
        let (s1, s2) = (w1 / den, w2 / den);
        self.path[seg].h_out = pt(a.h_out.x + d.x * s1, a.h_out.y + d.y * s1);
        self.path[seg + 1].h_in = pt(b.h_in.x + d.x * s2, b.h_in.y + d.y * s2);
    }

    /// Nearest point on the path: (segment, u, distance).
    pub fn nearest(&self, p: Pt) -> Option<(usize, f32, f32)> {
        let mut best: Option<(usize, f32, f32)> = None;
        for (i, w) in self.path.windows(2).enumerate() {
            for s in 0..=48 {
                let u = s as f32 / 48.0;
                let d = seg_point(w[0], w[1], u).dist(p);
                if best.map(|b| d < b.2).unwrap_or(true) {
                    best = Some((i, u, d));
                }
            }
        }
        best
    }
}

/// Raw Bezier point on the segment a→b at parameter s.
pub fn seg_point(a: PathKey, b: PathKey, s: f32) -> Pt {
    let p1 = pt(a.at.x + a.h_out.x, a.at.y + a.h_out.y);
    let p2 = pt(b.at.x + b.h_in.x, b.at.y + b.h_in.y);
    pt(
        cubic(a.at.x, p1.x, p2.x, b.at.x, s),
        cubic(a.at.y, p1.y, p2.y, b.at.y, s),
    )
}

/// Point at fraction `u` of the segment's arc length (constant speed).
fn seg_point_uniform(a: PathKey, b: PathKey, u: f32) -> Pt {
    if a.h_out == Pt::default() && b.h_in == Pt::default() {
        return a.at.lerp(b.at, u);
    }
    const N: usize = 32;
    let mut lens = [0.0f32; N + 1];
    let mut prev = a.at;
    for (i, l) in lens.iter_mut().enumerate().skip(1) {
        let q = seg_point(a, b, i as f32 / N as f32);
        *l = prev.dist(q);
        prev = q;
    }
    for i in 1..=N {
        lens[i] += lens[i - 1];
    }
    let total = lens[N];
    if total < 1e-6 {
        return a.at;
    }
    let target = u.clamp(0.0, 1.0) * total;
    let i = lens.iter().position(|&l| l >= target).unwrap_or(N).max(1);
    let f = (target - lens[i - 1]) / (lens[i] - lens[i - 1]).max(1e-6);
    seg_point(a, b, (i as f32 - 1.0 + f) / N as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_hit_ends_and_shape() {
        for f in EaseFamily::ALL {
            for d in EaseDir::ALL {
                let e = Ease::preset(f, d);
                assert!(e.apply(0.0).abs() < 1e-3, "{f:?} {d:?} at 0");
                assert!((e.apply(1.0) - 1.0).abs() < 1e-3, "{f:?} {d:?} at 1");
            }
        }
        assert!(Ease::preset(EaseFamily::Quad, EaseDir::In).apply(0.5) < 0.5);
        assert!(Ease::preset(EaseFamily::Quad, EaseDir::Out).apply(0.5) > 0.5);
        assert!(
            Ease::preset(EaseFamily::Back, EaseDir::In).apply(0.2) < 0.0,
            "back overshoots"
        );
        assert_eq!(
            Ease::from_name("bounce out"),
            Some(Ease::preset(EaseFamily::Bounce, EaseDir::Out))
        );
        assert_eq!(
            Ease::from_name("Stop and Start In Out"),
            Some(Ease::preset(EaseFamily::StopStart, EaseDir::InOut))
        );
    }

    #[test]
    fn custom_curve_and_conversion() {
        let lin = Ease::linear_custom();
        for i in 0..=10 {
            let t = i as f32 / 10.0;
            assert!((lin.apply(t) - t).abs() < 1e-3);
        }
        let q = Ease::preset(EaseFamily::Cubic, EaseDir::InOut);
        let c = q.to_custom();
        for i in 0..=20 {
            let t = i as f32 / 20.0;
            assert!(
                (c.apply(t) - q.apply(t)).abs() < 0.02,
                "t={t}: {} vs {}",
                c.apply(t),
                q.apply(t)
            );
        }
    }

    #[test]
    fn motion_tracks_and_path() {
        let base = Pose {
            pos: pt(0.0, 0.0),
            rotation: 0.0,
            skew: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            alpha: 1.0,
            brightness: 0.0,
        };
        let mut m = Motion::new(Pt::default());
        let mut after = base;
        after.pos = pt(100.0, 0.0);
        after.rotation = 90.0;
        m.record(10, &base, &base, &after);
        assert_eq!(m.path.len(), 2);
        assert_eq!(m.rotation.keys.len(), 2);
        assert!(
            m.scale_x.keys.is_empty(),
            "unchanged properties get no keys"
        );
        let p = m.pose_at(&base, 5.0, 11, 0.0, None);
        assert!((p.pos.x - 50.0).abs() < 1e-3 && (p.rotation - 45.0).abs() < 1e-3);
        // bend the path: the midpoint follows
        m.bend(0, 0.5, pt(50.0, 40.0));
        assert!(seg_point(m.path[0], m.path[1], 0.5).dist(pt(50.0, 40.0)) < 0.5);
        let mid = m.pose_at(&base, 5.0, 11, 0.0, None).pos;
        assert!(mid.y > 20.0, "object rides the bent path: {mid:?}");
        // ease the position: early frames lag behind
        m.path_ease = Some(Ease::preset(EaseFamily::Quad, EaseDir::In));
        let e = m.pose_at(&base, 2.0, 11, 0.0, None).pos;
        let lin_m = Motion {
            path_ease: None,
            ..m.clone()
        };
        assert!(
            e.dist(m.path[0].at)
                < lin_m
                    .pose_at(&base, 2.0, 11, 0.0, None)
                    .pos
                    .dist(m.path[0].at)
        );
        // reverse
        let mut r = m.clone();
        r.reverse(11);
        assert_eq!(r.path[0].at, pt(100.0, 0.0));
        assert_eq!(r.rotation.keys[0].value, 90.0);
    }

    #[test]
    fn pose_roundtrip() {
        let mut el = crate::model::Element::shape(
            "s",
            crate::model::Shape::new(crate::geom::Path::rect(0.0, 0.0, 10.0, 10.0), None, None),
        );
        let pivot = pt(5.0, 5.0);
        let p = Pose {
            pos: pt(40.0, 30.0),
            rotation: 30.0,
            skew: 10.0,
            scale_x: 2.0,
            scale_y: 0.5,
            alpha: 0.5,
            brightness: -0.4,
        };
        p.apply(&mut el, pivot);
        let q = Pose::of(&el, pivot);
        assert!(q.pos.dist(p.pos) < 1e-3);
        for prop in Prop::ALL {
            assert!(
                (q.get(prop) - p.get(prop)).abs() < 1e-3,
                "{prop:?}: {} vs {}",
                q.get(prop),
                p.get(prop)
            );
        }
    }
}
