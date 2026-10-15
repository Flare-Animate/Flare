//! Filters and blend modes (Properties › Filters / Blending), as in Animate.
//! Values use Animate's units: blur in pixels, strength in %, angle in
//! degrees, distance in pixels, adjust-color sliders −100…100 (hue ±180).

use crate::model::Color;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum BlendMode {
    #[default]
    Normal,
    Layer,
    Darken,
    Multiply,
    Lighten,
    Screen,
    Overlay,
    HardLight,
    Add,
    Subtract,
    Difference,
    Invert,
    Alpha,
    Erase,
}

impl BlendMode {
    pub const ALL: [BlendMode; 14] = [
        BlendMode::Normal,
        BlendMode::Layer,
        BlendMode::Darken,
        BlendMode::Multiply,
        BlendMode::Lighten,
        BlendMode::Screen,
        BlendMode::Overlay,
        BlendMode::HardLight,
        BlendMode::Add,
        BlendMode::Subtract,
        BlendMode::Difference,
        BlendMode::Invert,
        BlendMode::Alpha,
        BlendMode::Erase,
    ];
    pub fn name(self) -> &'static str {
        match self {
            BlendMode::Normal => "Normal",
            BlendMode::Layer => "Layer",
            BlendMode::Darken => "Darken",
            BlendMode::Multiply => "Multiply",
            BlendMode::Lighten => "Lighten",
            BlendMode::Screen => "Screen",
            BlendMode::Overlay => "Overlay",
            BlendMode::HardLight => "Hard Light",
            BlendMode::Add => "Add",
            BlendMode::Subtract => "Subtract",
            BlendMode::Difference => "Difference",
            BlendMode::Invert => "Invert",
            BlendMode::Alpha => "Alpha",
            BlendMode::Erase => "Erase",
        }
    }
    pub fn from_name(s: &str) -> Option<BlendMode> {
        let k = s.to_lowercase().replace([' ', '_', '-'], "");
        BlendMode::ALL.into_iter().find(|b| b.name().to_lowercase().replace(' ', "") == k)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum BevelType {
    #[default]
    Inner,
    Outer,
    Full,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Filter {
    DropShadow { blur_x: f32, blur_y: f32, strength: f32, quality: u8, angle: f32, distance: f32, color: Color, knockout: bool, inner: bool, hide_object: bool },
    Blur { blur_x: f32, blur_y: f32, quality: u8 },
    Glow { blur_x: f32, blur_y: f32, strength: f32, quality: u8, color: Color, knockout: bool, inner: bool },
    Bevel { blur_x: f32, blur_y: f32, strength: f32, quality: u8, shadow: Color, highlight: Color, angle: f32, distance: f32, knockout: bool, kind: BevelType },
    AdjustColor { brightness: f32, contrast: f32, saturation: f32, hue: f32 },
    /// A glow coloured by a gradient ramp: the glow's intensity picks the colour
    /// (the first stop is usually transparent).
    GradientGlow { blur_x: f32, blur_y: f32, strength: f32, quality: u8, angle: f32, distance: f32, knockout: bool, kind: BevelType, stops: Vec<(f32, Color)> },
    /// A bevel coloured by a gradient ramp: shadow side → middle (usually
    /// transparent) → highlight side.
    GradientBevel { blur_x: f32, blur_y: f32, strength: f32, quality: u8, angle: f32, distance: f32, knockout: bool, kind: BevelType, stops: Vec<(f32, Color)> },
    /// Motion blur from the object's real movement: the object is drawn at
    /// `samples` instants across the shutter (`shutter` degrees of a frame,
    /// 180 = half a frame, trailing behind the current position) and averaged.
    MotionBlur { shutter: f32, samples: u8 },
    /// An effect from the EffectCraft library, by id (e.g. "ec.blur.directional"),
    /// run on everything the object draws. Parameters not listed keep the
    /// effect's defaults. Rendered by `ac-render` (raster only).
    Effect { id: String, #[serde(default)] params: Vec<(String, EffectValue)> },
}

/// A parameter value of an EffectCraft effect (the subset we can edit and tween).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EffectValue {
    Bool(bool),
    Num(f64),
    Vec2([f64; 2]),
    Color(Color),
}

impl EffectValue {
    fn lerp(&self, o: &EffectValue, t: f32) -> EffectValue {
        let td = t as f64;
        match (self, o) {
            (EffectValue::Num(a), EffectValue::Num(b)) => EffectValue::Num(a + (b - a) * td),
            (EffectValue::Vec2(a), EffectValue::Vec2(b)) => EffectValue::Vec2([a[0] + (b[0] - a[0]) * td, a[1] + (b[1] - a[1]) * td]),
            (EffectValue::Color(a), EffectValue::Color(b)) => EffectValue::Color(a.lerp(*b, t)),
            _ => if t < 0.5 { self.clone() } else { o.clone() },
        }
    }
}

/// Colour of a gradient ramp at `t` (0…1); stops sorted by position.
pub fn ramp(stops: &[(f32, Color)], t: f32) -> Color {
    let Some(first) = stops.first() else { return Color { r: 0, g: 0, b: 0, a: 0 } };
    if t <= first.0 {
        return first.1;
    }
    for w in stops.windows(2) {
        if t <= w[1].0 {
            let u = (t - w[0].0) / (w[1].0 - w[0].0).max(1e-6);
            return w[0].1.lerp(w[1].1, u);
        }
    }
    stops[stops.len() - 1].1
}

impl Filter {
    /// Animate's defaults for a newly added filter.
    pub fn default_of(name: &str) -> Option<Filter> {
        Some(match name.to_lowercase().replace([' ', '_'], "").as_str() {
            "dropshadow" => Filter::DropShadow { blur_x: 5.0, blur_y: 5.0, strength: 100.0, quality: 1, angle: 45.0, distance: 5.0, color: Color::rgb(0, 0, 0), knockout: false, inner: false, hide_object: false },
            "blur" => Filter::Blur { blur_x: 5.0, blur_y: 5.0, quality: 1 },
            "glow" => Filter::Glow { blur_x: 5.0, blur_y: 5.0, strength: 100.0, quality: 1, color: Color::rgb(255, 0, 0), knockout: false, inner: false },
            "bevel" => Filter::Bevel { blur_x: 5.0, blur_y: 5.0, strength: 100.0, quality: 1, shadow: Color::rgb(0, 0, 0), highlight: Color::rgb(255, 255, 255), angle: 45.0, distance: 5.0, knockout: false, kind: BevelType::Inner },
            "adjustcolor" | "adjustcolour" => Filter::AdjustColor { brightness: 0.0, contrast: 0.0, saturation: 0.0, hue: 0.0 },
            "gradientglow" => Filter::GradientGlow { blur_x: 5.0, blur_y: 5.0, strength: 100.0, quality: 1, angle: 45.0, distance: 5.0, knockout: false, kind: BevelType::Outer, stops: vec![(0.0, Color { r: 255, g: 255, b: 255, a: 0 }), (1.0, Color::rgb(0, 0, 0))] },
            "gradientbevel" => Filter::GradientBevel {
                blur_x: 5.0,
                blur_y: 5.0,
                strength: 100.0,
                quality: 1,
                angle: 45.0,
                distance: 5.0,
                knockout: false,
                kind: BevelType::Inner,
                stops: vec![(0.0, Color::rgb(255, 255, 255)), (0.5, Color { r: 255, g: 0, b: 0, a: 0 }), (1.0, Color::rgb(0, 0, 0))],
            },
            "motionblur" => Filter::MotionBlur { shutter: 180.0, samples: 12 },
            _ => return None,
        })
    }
    pub const NAMES: [&'static str; 8] = ["Drop Shadow", "Blur", "Glow", "Bevel", "Gradient Glow", "Gradient Bevel", "Adjust Color", "Motion Blur"];
    pub fn name(&self) -> &'static str {
        match self {
            Filter::DropShadow { .. } => "Drop Shadow",
            Filter::Blur { .. } => "Blur",
            Filter::Glow { .. } => "Glow",
            Filter::Bevel { .. } => "Bevel",
            Filter::AdjustColor { .. } => "Adjust Color",
            Filter::GradientGlow { .. } => "Gradient Glow",
            Filter::GradientBevel { .. } => "Gradient Bevel",
            Filter::MotionBlur { .. } => "Motion Blur",
            Filter::Effect { .. } => "Effect",
        }
    }
    /// A numeric effect parameter, if set.
    pub fn effect_num(&self, key: &str) -> Option<f64> {
        match self {
            Filter::Effect { params, .. } => params.iter().find(|(k, _)| k == key).and_then(|(_, v)| match v {
                EffectValue::Num(n) => Some(*n),
                _ => None,
            }),
            _ => None,
        }
    }
    /// How far (in px, before view scaling) this filter can paint outside the object.
    pub fn margin(&self) -> f32 {
        match self {
            Filter::DropShadow { blur_x, blur_y, distance, .. } | Filter::Bevel { blur_x, blur_y, distance, .. } | Filter::GradientGlow { blur_x, blur_y, distance, .. } | Filter::GradientBevel { blur_x, blur_y, distance, .. } => blur_x.max(*blur_y) * 1.5 + distance.abs() + 2.0,
            Filter::Blur { blur_x, blur_y, .. } | Filter::Glow { blur_x, blur_y, .. } => blur_x.max(*blur_y) * 1.5 + 2.0,
            Filter::AdjustColor { .. } | Filter::MotionBlur { .. } => 0.0,
            // Effects can paint anywhere; size the margin from their size-like
            // parameters (blur, radius, length…), at least 24 px.
            Filter::Effect { params, .. } => {
                let big = params
                    .iter()
                    .filter(|(k, _)| ["blur", "radius", "length", "size", "distance", "amount", "spread", "width"].iter().any(|w| k.to_lowercase().contains(w)))
                    .filter_map(|(_, v)| if let EffectValue::Num(n) = v { Some(n.abs() as f32) } else { None })
                    .fold(0.0f32, f32::max);
                (big * 1.5).clamp(24.0, 400.0)
            }
        }
    }

    /// Tween two filter lists: same-kind filters at the same index interpolate.
    pub fn lerp_list(a: &[Filter], b: &[Filter], t: f32) -> Vec<Filter> {
        if a.is_empty() && b.is_empty() {
            return vec![];
        }
        let n = a.len().max(b.len());
        (0..n)
            .filter_map(|i| match (a.get(i), b.get(i)) {
                (Some(x), Some(y)) => Some(x.lerp(y, t)),
                (Some(x), None) => Some(if t < 1.0 { x.clone() } else { return None }),
                (None, Some(y)) => if t > 0.0 { Some(y.clone()) } else { None },
                (None, None) => None,
            })
            .collect()
    }

    pub fn lerp(&self, o: &Filter, t: f32) -> Filter {
        let l = |a: f32, b: f32| a + (b - a) * t;
        let pick = |a: bool, b: bool| if t < 0.5 { a } else { b };
        match (self, o) {
            (Filter::DropShadow { blur_x, blur_y, strength, quality, angle, distance, color, knockout, inner, hide_object }, Filter::DropShadow { blur_x: bx, blur_y: by, strength: s, angle: an, distance: d, color: c, knockout: k, inner: i, hide_object: h, .. }) => Filter::DropShadow {
                blur_x: l(*blur_x, *bx),
                blur_y: l(*blur_y, *by),
                strength: l(*strength, *s),
                quality: *quality,
                angle: l(*angle, *an),
                distance: l(*distance, *d),
                color: color.lerp(*c, t),
                knockout: pick(*knockout, *k),
                inner: pick(*inner, *i),
                hide_object: pick(*hide_object, *h),
            },
            (Filter::Blur { blur_x, blur_y, quality }, Filter::Blur { blur_x: bx, blur_y: by, .. }) => Filter::Blur { blur_x: l(*blur_x, *bx), blur_y: l(*blur_y, *by), quality: *quality },
            (Filter::Glow { blur_x, blur_y, strength, quality, color, knockout, inner }, Filter::Glow { blur_x: bx, blur_y: by, strength: s, color: c, knockout: k, inner: i, .. }) => Filter::Glow {
                blur_x: l(*blur_x, *bx),
                blur_y: l(*blur_y, *by),
                strength: l(*strength, *s),
                quality: *quality,
                color: color.lerp(*c, t),
                knockout: pick(*knockout, *k),
                inner: pick(*inner, *i),
            },
            (Filter::Bevel { blur_x, blur_y, strength, quality, shadow, highlight, angle, distance, knockout, kind }, Filter::Bevel { blur_x: bx, blur_y: by, strength: s, shadow: sh, highlight: hi, angle: an, distance: d, knockout: k, .. }) => Filter::Bevel {
                blur_x: l(*blur_x, *bx),
                blur_y: l(*blur_y, *by),
                strength: l(*strength, *s),
                quality: *quality,
                shadow: shadow.lerp(*sh, t),
                highlight: highlight.lerp(*hi, t),
                angle: l(*angle, *an),
                distance: l(*distance, *d),
                knockout: pick(*knockout, *k),
                kind: *kind,
            },
            (Filter::GradientGlow { blur_x, blur_y, strength, quality, angle, distance, knockout, kind, stops }, Filter::GradientGlow { blur_x: bx, blur_y: by, strength: s, angle: an, distance: d, knockout: k, stops: st, .. }) => Filter::GradientGlow {
                blur_x: l(*blur_x, *bx),
                blur_y: l(*blur_y, *by),
                strength: l(*strength, *s),
                quality: *quality,
                angle: l(*angle, *an),
                distance: l(*distance, *d),
                knockout: pick(*knockout, *k),
                kind: *kind,
                stops: lerp_stops(stops, st, t),
            },
            (Filter::GradientBevel { blur_x, blur_y, strength, quality, angle, distance, knockout, kind, stops }, Filter::GradientBevel { blur_x: bx, blur_y: by, strength: s, angle: an, distance: d, knockout: k, stops: st, .. }) => Filter::GradientBevel {
                blur_x: l(*blur_x, *bx),
                blur_y: l(*blur_y, *by),
                strength: l(*strength, *s),
                quality: *quality,
                angle: l(*angle, *an),
                distance: l(*distance, *d),
                knockout: pick(*knockout, *k),
                kind: *kind,
                stops: lerp_stops(stops, st, t),
            },
            (Filter::AdjustColor { brightness, contrast, saturation, hue }, Filter::AdjustColor { brightness: b, contrast: c, saturation: s, hue: h }) => {
                Filter::AdjustColor { brightness: l(*brightness, *b), contrast: l(*contrast, *c), saturation: l(*saturation, *s), hue: l(*hue, *h) }
            }
            (Filter::MotionBlur { shutter, samples }, Filter::MotionBlur { shutter: s2, .. }) => Filter::MotionBlur { shutter: l(*shutter, *s2), samples: *samples },
            (Filter::Effect { id, params }, Filter::Effect { id: id2, params: p2 }) if id == id2 => Filter::Effect {
                id: id.clone(),
                params: params
                    .iter()
                    .map(|(k, v)| (k.clone(), p2.iter().find(|(k2, _)| k2 == k).map_or(v.clone(), |(_, w)| v.lerp(w, t))))
                    .chain(p2.iter().filter(|(k, _)| !params.iter().any(|(k1, _)| k1 == k)).cloned())
                    .collect(),
            },
            _ => if t < 0.5 { self.clone() } else { o.clone() },
        }
    }
}

fn lerp_stops(a: &[(f32, Color)], b: &[(f32, Color)], t: f32) -> Vec<(f32, Color)> {
    if a.len() != b.len() {
        return if t < 0.5 { a.to_vec() } else { b.to_vec() };
    }
    a.iter().zip(b).map(|(x, y)| (x.0 + (y.0 - x.0) * t, x.1.lerp(y.1, t))).collect()
}

/// Effects applied to everything one element draws (one offscreen layer).
#[derive(Clone, Debug, PartialEq)]
pub struct FxGroup {
    pub filters: Vec<Filter>,
    pub blend: BlendMode,
    /// Motion since the previous frame, in the items' (stage) space: maps where
    /// the object is now to where it was a frame earlier. Set for groups with a
    /// Motion Blur filter.
    pub motion: Option<crate::geom::Matrix>,
    /// Seconds on the object's own timeline (time-dependent effects).
    pub time: f32,
    /// Asset Warp mesh: the layer is drawn deformed (stage space).
    pub mesh: Option<std::sync::Arc<crate::deform::MeshMap>>,
}

impl FxGroup {
    pub fn new(filters: Vec<Filter>, blend: BlendMode) -> FxGroup {
        FxGroup { filters, blend, motion: None, time: 0.0, mesh: None }
    }
}
