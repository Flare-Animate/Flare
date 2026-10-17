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
        CurvePt { x, y, h_in: Pt::default(), h_out: Pt::default() }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Ease {
    Preset { family: EaseFamily, dir: EaseDir },
    /// Bezier curve from (0,0) to (1,1); first and last anchors keep their x.
    Custom { points: Vec<CurvePt> },
}

impl Ease {
    pub fn preset(family: EaseFamily, dir: EaseDir) -> Self {
        Ease::Preset { family, dir }
    }
    /// A straight (linear) custom curve, ready to edit.
    pub fn linear_custom() -> Self {
        Ease::Custom { points: vec![CurvePt { x: 0.0, y: 0.0, h_in: Pt::default(), h_out: pt(0.33, 0.33) }, CurvePt { x: 1.0, y: 1.0, h_in: pt(-0.33, -0.33), h_out: Pt::default() }] }
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
                CurvePt { x, y: f(x), h_in: if i == 0 { Pt::default() } else { pt(-hx, -hx * s) }, h_out: if i == n { Pt::default() } else { pt(hx, hx * s) } }
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
                points: points.iter().rev().map(|p| CurvePt { x: 1.0 - p.x, y: 1.0 - p.y, h_in: pt(-p.h_out.x, -p.h_out.y), h_out: pt(-p.h_in.x, -p.h_in.y) }).collect(),
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
    let i = points.windows(2).position(|w| t >= w[0].x && t <= w[1].x).unwrap_or(0);
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
/// Classic tween "Edit each property separately": an ease per property group
/// (None = the span's ease).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PropEases {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<Ease>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rotation: Option<Ease>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<Ease>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Ease>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filters: Option<Ease>,
}

/// The property groups of `PropEases`, in UI order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropGroup {
    Position,
    Rotation,
    Scale,
    Color,
    Filters,
}

impl PropGroup {
    pub const ALL: [PropGroup; 5] = [PropGroup::Position, PropGroup::Rotation, PropGroup::Scale, PropGroup::Color, PropGroup::Filters];
    pub fn name(self) -> &'static str {
        match self {
            PropGroup::Position => "Position",
            PropGroup::Rotation => "Rotation",
            PropGroup::Scale => "Scale",
            PropGroup::Color => "Color",
            PropGroup::Filters => "Filters",
        }
    }
    pub fn id(self) -> &'static str {
        match self {
            PropGroup::Position => "position",
            PropGroup::Rotation => "rotation",
            PropGroup::Scale => "scale",
            PropGroup::Color => "color",
            PropGroup::Filters => "filters",
        }
    }
    pub fn from_id(s: &str) -> Option<PropGroup> {
        PropGroup::ALL.into_iter().find(|g| g.id() == s.to_ascii_lowercase())
    }
}

impl PropEases {
    pub fn get(&self, g: PropGroup) -> Option<&Ease> {
        match g {
            PropGroup::Position => self.position.as_ref(),
            PropGroup::Rotation => self.rotation.as_ref(),
            PropGroup::Scale => self.scale.as_ref(),
            PropGroup::Color => self.color.as_ref(),
            PropGroup::Filters => self.filters.as_ref(),
        }
    }
    pub fn get_mut(&mut self, g: PropGroup) -> &mut Option<Ease> {
        match g {
            PropGroup::Position => &mut self.position,
            PropGroup::Rotation => &mut self.rotation,
            PropGroup::Scale => &mut self.scale,
            PropGroup::Color => &mut self.color,
            PropGroup::Filters => &mut self.filters,
        }
    }
}

/// Tween progress per property group.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PropT {
    pub pos: f32,
    pub rot: f32,
    pub scale: f32,
    pub color: f32,
    pub filters: f32,
}

impl PropT {
    pub fn all(t: f32) -> PropT {
        PropT { pos: t, rot: t, scale: t, color: t, filters: t }
    }
    /// Progress at raw time `t` (0..1) for a span with simple ease `simple`,
    /// curve `curve` and optional per-property eases.
    pub fn at(t: f32, simple: f32, curve: Option<&Ease>, props: Option<&PropEases>) -> PropT {
        let base = tween_progress(t, simple, curve);
        let Some(p) = props else { return PropT::all(base) };
        let g = |g: PropGroup| p.get(g).map(|e| e.apply(t)).unwrap_or(base);
        PropT { pos: g(PropGroup::Position), rot: g(PropGroup::Rotation), scale: g(PropGroup::Scale), color: g(PropGroup::Color), filters: g(PropGroup::Filters) }
    }
}

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
    /// 3D rotation around X / Y (degrees) and depth (Z, px).
    RotationX,
    RotationY,
    Depth,
    /// Tint amount (0…1) of a colour tint.
    Tint,
    /// A numeric parameter of the element's filter at index n.
    Filter(u8, FilterParam),
}

/// Numeric filter parameters a motion tween can animate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum FilterParam {
    BlurX,
    BlurY,
    Strength,
    Angle,
    Distance,
    Brightness,
    Contrast,
    Saturation,
    Hue,
}

impl FilterParam {
    pub const ALL: [FilterParam; 9] = [FilterParam::BlurX, FilterParam::BlurY, FilterParam::Strength, FilterParam::Angle, FilterParam::Distance, FilterParam::Brightness, FilterParam::Contrast, FilterParam::Saturation, FilterParam::Hue];
    pub fn name(self) -> &'static str {
        match self {
            FilterParam::BlurX => "Blur X",
            FilterParam::BlurY => "Blur Y",
            FilterParam::Strength => "Strength",
            FilterParam::Angle => "Angle",
            FilterParam::Distance => "Distance",
            FilterParam::Brightness => "Brightness",
            FilterParam::Contrast => "Contrast",
            FilterParam::Saturation => "Saturation",
            FilterParam::Hue => "Hue",
        }
    }
    pub fn id(self) -> &'static str {
        match self {
            FilterParam::BlurX => "blur_x",
            FilterParam::BlurY => "blur_y",
            FilterParam::Strength => "strength",
            FilterParam::Angle => "angle",
            FilterParam::Distance => "distance",
            FilterParam::Brightness => "brightness",
            FilterParam::Contrast => "contrast",
            FilterParam::Saturation => "saturation",
            FilterParam::Hue => "hue",
        }
    }
    /// The parameter's slot in a filter, if it has one.
    fn slot(self, f: &mut crate::fx::Filter) -> Option<&mut f32> {
        use crate::fx::Filter as F;
        use FilterParam as P;
        Some(match (f, self) {
            (F::DropShadow { blur_x, .. } | F::Blur { blur_x, .. } | F::Glow { blur_x, .. } | F::Bevel { blur_x, .. } | F::GradientGlow { blur_x, .. } | F::GradientBevel { blur_x, .. }, P::BlurX) => blur_x,
            (F::DropShadow { blur_y, .. } | F::Blur { blur_y, .. } | F::Glow { blur_y, .. } | F::Bevel { blur_y, .. } | F::GradientGlow { blur_y, .. } | F::GradientBevel { blur_y, .. }, P::BlurY) => blur_y,
            (F::DropShadow { strength, .. } | F::Glow { strength, .. } | F::Bevel { strength, .. } | F::GradientGlow { strength, .. } | F::GradientBevel { strength, .. }, P::Strength) => strength,
            (F::DropShadow { angle, .. } | F::Bevel { angle, .. } | F::GradientGlow { angle, .. } | F::GradientBevel { angle, .. }, P::Angle) => angle,
            (F::DropShadow { distance, .. } | F::Bevel { distance, .. } | F::GradientGlow { distance, .. } | F::GradientBevel { distance, .. }, P::Distance) => distance,
            (F::AdjustColor { brightness, .. }, P::Brightness) => brightness,
            (F::AdjustColor { contrast, .. }, P::Contrast) => contrast,
            (F::AdjustColor { saturation, .. }, P::Saturation) => saturation,
            (F::AdjustColor { hue, .. }, P::Hue) => hue,
            _ => return None,
        })
    }
    /// The parameters a filter has.
    pub fn of(f: &crate::fx::Filter) -> Vec<FilterParam> {
        let mut c = f.clone();
        FilterParam::ALL.into_iter().filter(|p| p.slot(&mut c).is_some()).collect()
    }
}

impl Prop {
    /// The basic properties every motion tween has (position, transform, alpha, brightness).
    pub const ALL: [Prop; 8] = [Prop::X, Prop::Y, Prop::Rotation, Prop::Skew, Prop::ScaleX, Prop::ScaleY, Prop::Alpha, Prop::Brightness];
    /// 3D and colour properties (filters come from the object).
    pub const EXTRA: [Prop; 4] = [Prop::RotationX, Prop::RotationY, Prop::Depth, Prop::Tint];
    /// Every property the object `el` can animate: the basics, 3D, tint
    /// (with a colour tint) and its filters' parameters.
    pub fn all_for(el: &crate::model::Element) -> Vec<Prop> {
        let mut v: Vec<Prop> = Prop::ALL.to_vec();
        v.extend(Prop::EXTRA);
        for (i, f) in el.filters.iter().enumerate().take(255) {
            v.extend(FilterParam::of(f).into_iter().map(|p| Prop::Filter(i as u8, p)));
        }
        v
    }
    /// Kept in `Motion::extra` (not one of the built-in tracks).
    pub fn is_extra(self) -> bool {
        matches!(self, Prop::RotationX | Prop::RotationY | Prop::Depth | Prop::Tint | Prop::Filter(..))
    }
    /// Display label ("Rotation X", "Filter 1 · Blur X").
    pub fn label(self) -> String {
        match self {
            Prop::Filter(i, p) => format!("Filter {} · {}", i + 1, p.name()),
            p => p.name().to_string(),
        }
    }
    /// Script id: "x", "scale_x", "rotation_x", "filter1.blur_x".
    pub fn id(self) -> String {
        match self {
            Prop::Filter(i, p) => format!("filter{}.{}", i + 1, p.id()),
            p => p.name().to_ascii_lowercase().replace(' ', "_"),
        }
    }
    pub fn from_id(s: &str) -> Option<Prop> {
        let k = s.to_ascii_lowercase().replace(' ', "_");
        if let Some(rest) = k.strip_prefix("filter") {
            let (n, p) = rest.split_once('.')?;
            let i: u8 = n.parse::<u8>().ok()?.checked_sub(1)?;
            return FilterParam::ALL.into_iter().find(|x| x.id() == p).map(|p| Prop::Filter(i, p));
        }
        Prop::ALL.into_iter().chain(Prop::EXTRA).find(|p| p.id() == k || p.name().eq_ignore_ascii_case(s))
    }
    /// Read an extra property from an element (None if it doesn't have it).
    pub fn read(self, el: &crate::model::Element) -> Option<f32> {
        let t3 = el.three_d.unwrap_or_default();
        match self {
            Prop::RotationX => Some(t3.rx),
            Prop::RotationY => Some(t3.ry),
            Prop::Depth => Some(t3.z),
            Prop::Tint => Some(match el.tint {
                Some((c, a)) if !is_bw(c) => a,
                _ => 0.0,
            }),
            Prop::Filter(i, p) => {
                let mut f = el.filters.get(i as usize)?.clone();
                p.slot(&mut f).map(|v| *v)
            }
            _ => None,
        }
    }
    /// Write an extra property onto an element.
    pub fn write(self, el: &mut crate::model::Element, v: f32) {
        match self {
            Prop::RotationX | Prop::RotationY | Prop::Depth => {
                let t = el.three_d.get_or_insert_with(Default::default);
                match self {
                    Prop::RotationX => t.rx = v,
                    Prop::RotationY => t.ry = v,
                    _ => t.z = v,
                }
                if t.is_identity() {
                    el.three_d = None;
                }
            }
            Prop::Tint => {
                if let Some((c, a)) = &mut el.tint {
                    if !is_bw(*c) {
                        *a = v.clamp(0.0, 1.0);
                    }
                }
            }
            Prop::Filter(i, p) => {
                if let Some(slot) = el.filters.get_mut(i as usize).and_then(|f| p.slot(f)) {
                    *slot = v;
                }
            }
            _ => {}
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Prop::RotationX => "Rotation X",
            Prop::RotationY => "Rotation Y",
            Prop::Depth => "Depth",
            Prop::Tint => "Tint",
            Prop::Filter(_, p) => p.name(),
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
            Prop::X | Prop::Y | Prop::Depth => "px",
            Prop::Rotation | Prop::Skew | Prop::RotationX | Prop::RotationY => "°",
            Prop::Filter(_, FilterParam::Angle | FilterParam::Hue) => "°",
            Prop::Filter(_, FilterParam::Strength) => "%",
            Prop::Filter(..) => "",
            _ => "%",
        }
    }
    pub fn from_name(s: &str) -> Option<Prop> {
        let k = s.to_ascii_lowercase().replace([' ', '_'], "");
        Prop::ALL.into_iter().chain(Prop::EXTRA).find(|p| p.name().to_ascii_lowercase().replace(' ', "") == k).or_else(|| Prop::from_id(s))
    }
    /// Shown in the UI as percent for scale/alpha/brightness/tint.
    pub fn display_scale(self) -> f32 {
        match self {
            Prop::ScaleX | Prop::ScaleY | Prop::Alpha | Prop::Brightness | Prop::Tint => 100.0,
            _ => 1.0,
        }
    }
}

/// How a property curve gets from a key to the next one: OpenToonz's
/// keyframe types (`common/tparam/tdoubleparam.cpp`; BSD-3-Clause, © DWANGO
/// and OpenToonz contributors). Stored on the key that starts the segment.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Interp {
    /// Smooth / corner point or Bezier handles (Animate's Motion Editor).
    #[default]
    Auto,
    /// Hold the key's value until the next key (stepped).
    Constant,
    Linear,
    /// Accelerate over `ease_in` frames, decelerate over the last `ease_out`.
    EaseInOut { ease_in: f32, ease_out: f32 },
    /// Same with percentages of the segment.
    EaseInOutPct { ease_in: f32, ease_out: f32 },
    /// Geometric progression (both values must be positive; else linear).
    Exponential,
    /// Any expression (see `expr.rs`), in the property's displayed units.
    Expression { text: String },
    /// The shape of an expression, scaled to run from this key's value to the next.
    SimilarShape {
        text: String,
        #[serde(default)]
        offset: f32,
    },
}

impl Interp {
    pub fn is_auto(&self) -> bool {
        *self == Interp::Auto
    }
    pub const NAMES: [&'static str; 8] = ["Auto (smooth / corner / Bezier)", "Constant", "Linear", "Ease In/Out (frames)", "Ease In/Out (%)", "Exponential", "Expression", "Similar shape"];
    pub fn index(&self) -> usize {
        match self {
            Interp::Auto => 0,
            Interp::Constant => 1,
            Interp::Linear => 2,
            Interp::EaseInOut { .. } => 3,
            Interp::EaseInOutPct { .. } => 4,
            Interp::Exponential => 5,
            Interp::Expression { .. } => 6,
            Interp::SimilarShape { .. } => 7,
        }
    }
    /// A fresh interpolation of type `i` (NAMES order); `seg` = segment length in frames.
    pub fn of_index(i: usize, seg: f32) -> Interp {
        match i {
            1 => Interp::Constant,
            2 => Interp::Linear,
            3 => Interp::EaseInOut { ease_in: (seg / 3.0).round(), ease_out: (seg / 3.0).round() },
            4 => Interp::EaseInOutPct { ease_in: 33.0, ease_out: 33.0 },
            5 => Interp::Exponential,
            6 => Interp::Expression { text: "v0 + (v1 - v0) * t".into() },
            7 => Interp::SimilarShape { text: "sin(frame * 30)".into(), offset: 0.0 },
            _ => Interp::Auto,
        }
    }
    /// Script id.
    pub fn id(&self) -> &'static str {
        ["auto", "constant", "linear", "ease_in_out", "ease_in_out_pct", "exponential", "expression", "similar_shape"][self.index()]
    }
}

/// Property names an expression may use (ids of the animatable properties).
pub fn expr_props() -> Vec<String> {
    Prop::ALL.into_iter().chain(Prop::EXTRA).map(|p| p.id()).collect()
}

/// A property keyframe. `frame` is relative to the tween span's start.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PropKey {
    pub frame: u32,
    pub value: f32,
    /// Smooth point (curved through, Catmull-Rom) vs corner point (linear).
    #[serde(default)]
    pub smooth: bool,
    /// Bezier handles set in the Motion Editor, in (frames, value) relative to
    /// the key: `[in_df, in_dv, out_df, out_dv]`. Overrides `smooth`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handles: Option<[f32; 4]>,
    /// How the curve continues to the next key (OpenToonz keyframe types).
    #[serde(default, skip_serializing_if = "Interp::is_auto")]
    pub interp: Interp,
}

impl PropKey {
    pub fn new(frame: u32, value: f32, smooth: bool) -> PropKey {
        PropKey { frame, value, smooth, handles: None, interp: Interp::Auto }
    }
}

/// OpenToonz's ease in/out: constant acceleration over `e0` frames, constant
/// speed, constant deceleration over the last `e1` (x3 = segment length).
fn ease_in_out(x: f32, x3: f32, e0: f32, e1: f32) -> f32 {
    if x3 <= 0.0 || x >= x3 {
        return 1.0;
    }
    if x <= 0.0 {
        return 0.0;
    }
    let (mut e0, mut e1) = (e0.max(0.0), e1.max(0.0));
    if e0 + e1 >= x3 {
        let m = ((e0 + x3 - e1) / 2.0).clamp(0.0, x3);
        e0 = m;
        e1 = x3 - m;
    }
    let (x1, x2) = (e0, x3 - e1);
    let v = 2.0 / (x3 + x2 - x1);
    if x < x1 {
        0.5 * (v / e0) * x * x
    } else if x > x2 {
        1.0 - 0.5 * (v / e1) * (x3 - x) * (x3 - x)
    } else {
        x * v - 0.5 * v * e0
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Track {
    pub keys: Vec<PropKey>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ease: Option<Ease>,
}

impl Track {
    pub fn value_at(&self, f: f32) -> Option<f32> {
        self.value_at_ctx(f, 1.0, &|_, _| None, 0)
    }
    /// The value at `f`; `scale` converts to the displayed units expressions
    /// work in; `refs` gives other properties (displayed units, 1-based frame).
    pub fn value_at_ctx(&self, f: f32, scale: f32, refs: &dyn Fn(&str, f64) -> Option<f64>, depth: u32) -> Option<f32> {
        let k = &self.keys;
        let first = k.first()?;
        // (at the first key itself, an expression segment already applies)
        if f < first.frame as f32 || k.len() == 1 || (f == first.frame as f32 && matches!(first.interp, Interp::Auto)) {
            return Some(first.value);
        }
        let last = &k[k.len() - 1];
        if f >= last.frame as f32 {
            return Some(last.value);
        }
        let i = k.windows(2).position(|w| f >= w[0].frame as f32 && f <= w[1].frame as f32)?;
        let (a, b) = (&k[i], &k[i + 1]);
        let span = (b.frame - a.frame).max(1) as f32;
        let u = (f - a.frame as f32) / span;
        let lerp = |w: f32| a.value + (b.value - a.value) * w;
        let expr_at = |text: &str, fr: f32| -> Option<f32> {
            let props = expr_props();
            let names: Vec<&str> = props.iter().map(|s| s.as_str()).collect();
            let e = crate::expr::Expr::parse(text, &names).ok()?;
            let own = |fr1: f64| if depth < 8 { self.value_at_ctx(fr1 as f32 - 1.0, scale, refs, depth + 1).map(|v| (v * scale) as f64) } else { None };
            let c = crate::expr::Ctx { frame: fr as f64 + 1.0, rframe: (fr - a.frame as f32) as f64 + 1.0, t: ((fr - a.frame as f32) / span) as f64, v0: (a.value * scale) as f64, v1: (b.value * scale) as f64, refs, own: &own };
            Some(e.eval(&c) as f32 / scale.max(1e-9))
        };
        match &a.interp {
            Interp::Auto => {}
            Interp::Constant => return Some(a.value),
            Interp::Linear => return Some(lerp(u)),
            Interp::EaseInOut { ease_in, ease_out } => return Some(lerp(ease_in_out(f - a.frame as f32, span, *ease_in, *ease_out))),
            Interp::EaseInOutPct { ease_in, ease_out } => return Some(lerp(ease_in_out(f - a.frame as f32, span, ease_in * span * 0.01, ease_out * span * 0.01))),
            Interp::Exponential => {
                let (mut va, mut vb, mut t) = (a.value, b.value, u);
                if va <= 0.0 || vb <= 0.0 {
                    return Some(lerp(u));
                }
                if vb < va {
                    t = 1.0 - t;
                    std::mem::swap(&mut va, &mut vb);
                }
                return Some(va * (t * (vb / va).ln()).exp());
            }
            Interp::Expression { text } => return Some(expr_at(text, f).unwrap_or_else(|| lerp(u))),
            Interp::SimilarShape { text, offset } => {
                let r = (expr_at(text, a.frame as f32 + offset), expr_at(text, b.frame as f32 + offset), expr_at(text, f + offset));
                return Some(match r {
                    (Some(r0), Some(r1), Some(rv)) if (r1 - r0).abs() > 1e-9 => a.value + (b.value - a.value) * (rv - r0) / (r1 - r0),
                    _ => lerp(u),
                });
            }
        }
        if a.handles.is_some() || b.handles.is_some() {
            // a cubic Bezier in (frame, value); solve frame → curve parameter
            let (o, n) = (self.handle(i, true), self.handle(i + 1, false));
            let (x0, x3) = (a.frame as f32, b.frame as f32);
            let x1 = (x0 + o.0).clamp(x0, x3);
            let x2 = (x3 + n.0).clamp(x0, x3);
            let (y0, y1, y2, y3) = (a.value, a.value + o.1, b.value + n.1, b.value);
            let bez = |p0: f32, p1: f32, p2: f32, p3: f32, s: f32| {
                let r = 1.0 - s;
                r * r * r * p0 + 3.0 * r * r * s * p1 + 3.0 * r * s * s * p2 + s * s * s * p3
            };
            let (mut lo, mut hi) = (0.0f32, 1.0f32);
            for _ in 0..40 {
                let mid = (lo + hi) / 2.0;
                if bez(x0, x1, x2, x3, mid) < f {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            return Some(bez(y0, y1, y2, y3, (lo + hi) / 2.0));
        }
        if !a.smooth && !b.smooth {
            return Some(a.value + (b.value - a.value) * u);
        }
        // cubic Hermite with Catmull-Rom tangents at smooth keys (per frame)
        let tangent = |j: usize, smooth: bool| -> f32 {
            if !smooth {
                // corner: aim at the neighbour on this segment
                return (b.value - a.value) / span;
            }
            let p = if j > 0 { &k[j - 1] } else { &k[j] };
            let n = if j + 1 < k.len() { &k[j + 1] } else { &k[j] };
            let df = (n.frame as f32 - p.frame as f32).max(1.0);
            (n.value - p.value) / df
        };
        let (m0, m1) = (tangent(i, a.smooth) * span, tangent(i + 1, b.smooth) * span);
        let (u2, u3) = (u * u, u * u * u);
        Some((2.0 * u3 - 3.0 * u2 + 1.0) * a.value + (u3 - 2.0 * u2 + u) * m0 + (-2.0 * u3 + 3.0 * u2) * b.value + (u3 - u2) * m1)
    }
    /// The handle of key `j` (out = towards the next key) in (frames, value):
    /// its own, or the one a smooth/corner key implies.
    pub fn handle(&self, j: usize, out: bool) -> (f32, f32) {
        let k = &self.keys;
        let key = &k[j];
        if let Some(h) = key.handles {
            return if out { (h[2], h[3]) } else { (h[0], h[1]) };
        }
        let nb = if out { k.get(j + 1) } else { j.checked_sub(1).and_then(|p| k.get(p)) };
        let Some(nb) = nb else { return (0.0, 0.0) };
        let df = (nb.frame as f32 - key.frame as f32) / 3.0;
        let slope = if key.smooth {
            let p = if j > 0 { &k[j - 1] } else { key };
            let n = if j + 1 < k.len() { &k[j + 1] } else { key };
            (n.value - p.value) / (n.frame as f32 - p.frame as f32).max(1.0)
        } else {
            (nb.value - key.value) / (nb.frame as f32 - key.frame as f32)
        };
        (df, slope * df)
    }
    /// Set Bezier handles on the key at `frame` (None = back to smooth/corner).
    pub fn set_handles(&mut self, frame: u32, h: Option<[f32; 4]>) {
        if let Some(k) = self.keys.iter_mut().find(|k| k.frame == frame) {
            k.handles = h;
            if h.is_some() {
                k.smooth = true;
            }
        }
    }

    /// Insert or replace the key at `frame`.
    pub fn set(&mut self, frame: u32, value: f32) {
        match self.keys.binary_search_by_key(&frame, |k| k.frame) {
            Ok(i) => self.keys[i].value = value,
            Err(i) => {
                let smooth = self.keys.get(i.saturating_sub(1)).map(|k| k.smooth).unwrap_or(false);
                self.keys.insert(i, PropKey::new(frame, value, smooth));
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
    /// 3D, tint and filter tracks (see `Prop::is_extra`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extra: Vec<(Prop, Track)>,
}

fn is_bw(c: crate::model::Color) -> bool {
    (c.r, c.g, c.b) == (255, 255, 255) || (c.r, c.g, c.b) == (0, 0, 0)
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
            _ => 0.0,
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
            _ => {}
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
        let lin = Matrix::compose(&Decomposed { tx: 0.0, ty: 0.0, scale_x: self.scale_x, scale_y: self.scale_y, skew_x: r + self.skew.to_radians(), skew_y: r });
        let m = Matrix::translate(self.pos.x, self.pos.y).concat(&lin).concat(&Matrix::translate(-pivot.x, -pivot.y));
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
pub fn brightness_tint(b: f32, keep: Option<(crate::model::Color, f32)>) -> Option<(crate::model::Color, f32)> {
    use crate::model::Color;
    let is_bw = |t: Option<(Color, f32)>| matches!(t, Some((c, _)) if (c.r, c.g, c.b) == (255, 255, 255) || (c.r, c.g, c.b) == (0, 0, 0));
    if b.abs() < 1e-4 {
        return if is_bw(keep) { None } else { keep };
    }
    if !is_bw(keep) && keep.is_some() {
        return keep; // a real colour tint wins; brightness needs the tint slot
    }
    let b = b.clamp(-1.0, 1.0);
    Some((if b > 0.0 { Color::rgb(255, 255, 255) } else { Color::rgb(0, 0, 0) }, b.abs()))
}

impl Motion {
    pub fn new(pivot: Pt) -> Self {
        Motion { pivot, ..Default::default() }
    }
    pub fn track(&self, p: Prop) -> Option<&Track> {
        Some(match p {
            Prop::X | Prop::Y => return None,
            p if p.is_extra() => return self.extra.iter().find(|(q, _)| *q == p).map(|(_, t)| t),
            Prop::Rotation => &self.rotation,
            Prop::Skew => &self.skew,
            Prop::ScaleX => &self.scale_x,
            Prop::ScaleY => &self.scale_y,
            Prop::Alpha => &self.alpha,
            Prop::Brightness => &self.brightness,
            _ => return None,
        })
    }
    pub fn track_mut(&mut self, p: Prop) -> Option<&mut Track> {
        Some(match p {
            Prop::X | Prop::Y => return None,
            p if p.is_extra() => {
                let i = match self.extra.iter().position(|(q, _)| *q == p) {
                    Some(i) => i,
                    None => {
                        self.extra.push((p, Track::default()));
                        self.extra.len() - 1
                    }
                };
                &mut self.extra[i].1
            }
            Prop::Rotation => &mut self.rotation,
            Prop::Skew => &mut self.skew,
            Prop::ScaleX => &mut self.scale_x,
            Prop::ScaleY => &mut self.scale_y,
            Prop::Alpha => &mut self.alpha,
            Prop::Brightness => &mut self.brightness,
            _ => return None,
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
        for (_, t) in &self.extra {
            v.extend(t.keys.iter().map(|k| k.frame));
        }
        v.sort();
        v.dedup();
        v
    }
    pub fn keys_of(&self, p: Prop) -> Vec<(u32, f32)> {
        match p {
            Prop::X => self.path.iter().map(|k| (k.frame, k.at.x)).collect(),
            Prop::Y => self.path.iter().map(|k| (k.frame, k.at.y)).collect(),
            _ => self.track(p).map(|t| t.keys.iter().map(|k| (k.frame, k.value)).collect()).unwrap_or_default(),
        }
    }

    /// A property's value at span frame `f` (no eases), with expressions able
    /// to read the object's other properties.
    pub fn prop_value(&self, base: &Pose, p: Prop, f: f32, depth: u32) -> Option<f32> {
        match p {
            Prop::X => return self.path_point(f).map(|q| q.x),
            Prop::Y => return self.path_point(f).map(|q| q.y),
            _ => {}
        }
        let t = self.track(p)?;
        if t.keys.is_empty() {
            return None;
        }
        let refs = |name: &str, frame1: f64| -> Option<f64> {
            if depth >= 6 {
                return None;
            }
            let q = Prop::from_id(name)?;
            let v = self.prop_value(base, q, frame1 as f32 - 1.0, depth + 1).or_else(|| (!q.is_extra()).then(|| base.get(q)))?;
            Some((v * q.display_scale()) as f64)
        };
        t.value_at_ctx(f, p.display_scale(), &refs, depth)
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
    pub fn pose_at(&self, base: &Pose, local: f32, len: u32, simple: f32, span_ease: Option<&Ease>) -> Pose {
        let mut p = *base;
        let fp = self.eased_frame(local, len, self.ease_of(Prop::X, span_ease), simple);
        if let Some(pos) = self.path_point(fp) {
            p.pos = pos;
        }
        for prop in [Prop::Rotation, Prop::Skew, Prop::ScaleX, Prop::ScaleY, Prop::Alpha, Prop::Brightness] {
            let f = self.eased_frame(local, len, self.ease_of(prop, span_ease), simple);
            if let Some(v) = self.prop_value(base, prop, f, 0) {
                p.set(prop, v);
            }
        }
        if self.rotate_turns != 0 && len > 1 {
            let f = self.eased_frame(local, len, self.ease_of(Prop::Rotation, span_ease), simple);
            p.rotation += 360.0 * self.rotate_turns as f32 * (f / (len - 1) as f32).clamp(0.0, 1.0);
        }
        p
    }

    /// Write the 3D / tint / filter tracks onto the posed element.
    pub fn apply_extra(&self, el: &mut crate::model::Element, local: f32, len: u32, simple: f32, span_ease: Option<&Ease>) {
        let base = Pose::of(el, self.pivot);
        for (p, t) in &self.extra {
            let f = self.eased_frame(local, len, t.ease.as_ref().or(span_ease), simple);
            if let Some(v) = self.prop_value(&base, *p, f, 0) {
                p.write(el, v);
            }
        }
    }

    /// Record changes to 3D / tint / filter values made at `frame` (the
    /// object at that frame went from `before` to `after`; `base` is the span's object).
    pub fn record_extra(&mut self, frame: u32, base: &crate::model::Element, before: &crate::model::Element, after: &crate::model::Element) {
        for p in Prop::all_for(after).into_iter().filter(|p| p.is_extra()) {
            let (Some(b), Some(a)) = (p.read(before), p.read(after)) else { continue };
            if (b - a).abs() <= 1e-4 {
                continue;
            }
            let base_v = p.read(base).unwrap_or(b);
            let t = self.track_mut(p).unwrap();
            if t.keys.is_empty() {
                if frame == 0 {
                    continue; // changes the object itself
                }
                t.set(0, base_v);
            }
            t.set(frame, a);
        }
    }

    /// Every track including the extra ones (for retiming).
    fn all_tracks_mut(&mut self) -> Vec<&mut Track> {
        let Motion { rotation, skew, scale_x, scale_y, alpha, brightness, extra, .. } = self;
        let mut v = vec![rotation, skew, scale_x, scale_y, alpha, brightness];
        v.extend(extra.iter_mut().map(|(_, t)| t));
        v
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
        let i = k.windows(2).position(|w| f >= w[0].frame as f32 && f <= w[1].frame as f32)?;
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
            let (p, q) = if f <= k[0].frame as f32 { (k[0], k[1]) } else { (k[k.len() - 2], k[k.len() - 1]) };
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
            Err(i) => self.path.insert(i, PathKey { frame, at, h_in: Pt::default(), h_out: Pt::default() }),
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
        for prop in [Prop::Rotation, Prop::Skew, Prop::ScaleX, Prop::ScaleY, Prop::Alpha, Prop::Brightness] {
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
            let v = if prop == Prop::Rotation { b + wrap_deg(a - b) } else { a };
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
                let mut at = self.path.iter().find(|k| k.frame == frame).map(|k| k.at).unwrap_or(now.pos);
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
            _ => {
                let r = self.track_mut(prop).unwrap().remove(frame);
                self.extra.retain(|(_, t)| !t.keys.is_empty() || t.ease.is_some());
                r
            }
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
        if let Some(k) = self.track_mut(prop).and_then(|t| t.keys.iter_mut().find(|k| k.frame == frame)) {
            k.smooth = smooth;
            k.handles = None;
        }
    }

    /// A property's curve as a track (Motion Editor ▸ Copy curve). Position
    /// curves come out as linear keys with the path ease.
    pub fn curve(&self, prop: Prop) -> Track {
        match prop {
            Prop::X | Prop::Y => Track { keys: self.keys_of(prop).into_iter().map(|(f, v)| PropKey::new(f, v, false)).collect(), ease: self.path_ease.clone() },
            p => self.track(p).cloned().unwrap_or_default(),
        }
    }
    /// Replace a property's curve (Paste curve); keys beyond the span are
    /// dropped. Position (X/Y) can't take a pasted curve: false.
    pub fn paste_curve(&mut self, prop: Prop, mut t: Track, len: u32) -> bool {
        if matches!(prop, Prop::X | Prop::Y) {
            return false;
        }
        t.keys.retain(|k| k.frame < len.max(1));
        *self.track_mut(prop).unwrap() = t;
        self.extra.retain(|(_, t)| !t.keys.is_empty() || t.ease.is_some());
        true
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
        for prop in [Prop::Rotation, Prop::Skew, Prop::ScaleX, Prop::ScaleY, Prop::Alpha, Prop::Brightness] {
            let t = self.track_mut(prop).unwrap();
            if t.keys.is_empty() {
                t.set(0, base.get(prop));
            }
            t.set(frame, now.get(prop));
        }
    }

    /// F6 for the extra (3D / tint / filter) tracks the object already has.
    pub fn key_all_extra(&mut self, frame: u32, now: &crate::model::Element) {
        for (p, t) in &mut self.extra {
            if let Some(v) = p.read(now) {
                t.set(frame, v);
            }
        }
    }

    /// Split at `at` (relative): `self` keeps the keys before it (plus `pose`
    /// as the closing value); the returned motion holds the rest, rebased.
    pub fn split(&mut self, at: u32, pose: &Pose) -> Motion {
        let mut tail = Motion { pivot: self.pivot, path_ease: self.path_ease.clone(), rotate_turns: 0, ..Default::default() };
        if !self.path.is_empty() {
            tail.path = self.path.iter().filter(|k| k.frame > at).map(|k| PathKey { frame: k.frame - at, ..*k }).collect();
            tail.path.insert(0, PathKey { frame: 0, at: pose.pos, h_in: Pt::default(), h_out: Pt::default() });
            self.path.retain(|k| k.frame < at);
            self.set_pos(at, pose.pos);
        }
        for prop in [Prop::Rotation, Prop::Skew, Prop::ScaleX, Prop::ScaleY, Prop::Alpha, Prop::Brightness] {
            let t = self.track_mut(prop).unwrap();
            let mut nt = Track { keys: vec![], ease: t.ease.clone() };
            if !t.keys.is_empty() {
                nt.keys = t.keys.iter().filter(|k| k.frame > at).map(|k| PropKey { frame: k.frame - at, ..k.clone() }).collect();
                nt.keys.insert(0, PropKey::new(0, pose.get(prop), false));
                t.keys.retain(|k| k.frame < at);
                t.set(at, pose.get(prop));
            }
            *tail.track_mut(prop).unwrap() = nt;
        }
        for (p, t) in &mut self.extra {
            let mut nt = Track { keys: vec![], ease: t.ease.clone() };
            if let Some(v) = t.value_at(at as f32) {
                nt.keys = t.keys.iter().filter(|k| k.frame > at).map(|k| PropKey { frame: k.frame - at, ..k.clone() }).collect();
                nt.keys.insert(0, PropKey::new(0, v, false));
                t.keys.retain(|k| k.frame < at);
                t.set(at, v);
            }
            tail.extra.push((*p, nt));
        }
        tail
    }

    /// Remove every property key at `frame` (not the first frame).
    pub fn clear_keys_at(&mut self, frame: u32) -> bool {
        let mut any = false;
        let n = self.path.len();
        self.path.retain(|k| k.frame != frame);
        any |= n != self.path.len();
        for t in self.all_tracks_mut() {
            any |= t.remove(frame);
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
        for t in self.all_tracks_mut() {
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
        for t in self.all_tracks_mut() {
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
    pt(cubic(a.at.x, p1.x, p2.x, b.at.x, s), cubic(a.at.y, p1.y, p2.y, b.at.y, s))
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
        assert!(Ease::preset(EaseFamily::Back, EaseDir::In).apply(0.2) < 0.0, "back overshoots");
        assert_eq!(Ease::from_name("bounce out"), Some(Ease::preset(EaseFamily::Bounce, EaseDir::Out)));
        assert_eq!(Ease::from_name("Stop and Start In Out"), Some(Ease::preset(EaseFamily::StopStart, EaseDir::InOut)));
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
            assert!((c.apply(t) - q.apply(t)).abs() < 0.02, "t={t}: {} vs {}", c.apply(t), q.apply(t));
        }
    }

    #[test]
    fn motion_tracks_and_path() {
        let base = Pose { pos: pt(0.0, 0.0), rotation: 0.0, skew: 0.0, scale_x: 1.0, scale_y: 1.0, alpha: 1.0, brightness: 0.0 };
        let mut m = Motion::new(Pt::default());
        let mut after = base;
        after.pos = pt(100.0, 0.0);
        after.rotation = 90.0;
        m.record(10, &base, &base, &after);
        assert_eq!(m.path.len(), 2);
        assert_eq!(m.rotation.keys.len(), 2);
        assert!(m.scale_x.keys.is_empty(), "unchanged properties get no keys");
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
        let lin_m = Motion { path_ease: None, ..m.clone() };
        assert!(e.dist(m.path[0].at) < lin_m.pose_at(&base, 2.0, 11, 0.0, None).pos.dist(m.path[0].at));
        // reverse
        let mut r = m.clone();
        r.reverse(11);
        assert_eq!(r.path[0].at, pt(100.0, 0.0));
        assert_eq!(r.rotation.keys[0].value, 90.0);
    }

    #[test]
    fn pose_roundtrip() {
        let mut el = crate::model::Element::shape("s", crate::model::Shape::new(crate::geom::Path::rect(0.0, 0.0, 10.0, 10.0), None, None));
        let pivot = pt(5.0, 5.0);
        let p = Pose { pos: pt(40.0, 30.0), rotation: 30.0, skew: 10.0, scale_x: 2.0, scale_y: 0.5, alpha: 0.5, brightness: -0.4 };
        p.apply(&mut el, pivot);
        let q = Pose::of(&el, pivot);
        assert!(q.pos.dist(p.pos) < 1e-3);
        for prop in Prop::ALL {
            assert!((q.get(prop) - p.get(prop)).abs() < 1e-3, "{prop:?}: {} vs {}", q.get(prop), p.get(prop));
        }
    }
}
