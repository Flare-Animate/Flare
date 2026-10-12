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
        BlendMode::ALL
            .into_iter()
            .find(|b| b.name().to_lowercase().replace(' ', "") == k)
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
    DropShadow {
        blur_x: f32,
        blur_y: f32,
        strength: f32,
        quality: u8,
        angle: f32,
        distance: f32,
        color: Color,
        knockout: bool,
        inner: bool,
        hide_object: bool,
    },
    Blur {
        blur_x: f32,
        blur_y: f32,
        quality: u8,
    },
    Glow {
        blur_x: f32,
        blur_y: f32,
        strength: f32,
        quality: u8,
        color: Color,
        knockout: bool,
        inner: bool,
    },
    Bevel {
        blur_x: f32,
        blur_y: f32,
        strength: f32,
        quality: u8,
        shadow: Color,
        highlight: Color,
        angle: f32,
        distance: f32,
        knockout: bool,
        kind: BevelType,
    },
    AdjustColor {
        brightness: f32,
        contrast: f32,
        saturation: f32,
        hue: f32,
    },
}

impl Filter {
    /// Animate's defaults for a newly added filter.
    pub fn default_of(name: &str) -> Option<Filter> {
        Some(match name.to_lowercase().replace([' ', '_'], "").as_str() {
            "dropshadow" => Filter::DropShadow {
                blur_x: 5.0,
                blur_y: 5.0,
                strength: 100.0,
                quality: 1,
                angle: 45.0,
                distance: 5.0,
                color: Color::rgb(0, 0, 0),
                knockout: false,
                inner: false,
                hide_object: false,
            },
            "blur" => Filter::Blur {
                blur_x: 5.0,
                blur_y: 5.0,
                quality: 1,
            },
            "glow" => Filter::Glow {
                blur_x: 5.0,
                blur_y: 5.0,
                strength: 100.0,
                quality: 1,
                color: Color::rgb(255, 0, 0),
                knockout: false,
                inner: false,
            },
            "bevel" => Filter::Bevel {
                blur_x: 5.0,
                blur_y: 5.0,
                strength: 100.0,
                quality: 1,
                shadow: Color::rgb(0, 0, 0),
                highlight: Color::rgb(255, 255, 255),
                angle: 45.0,
                distance: 5.0,
                knockout: false,
                kind: BevelType::Inner,
            },
            "adjustcolor" | "adjustcolour" => Filter::AdjustColor {
                brightness: 0.0,
                contrast: 0.0,
                saturation: 0.0,
                hue: 0.0,
            },
            _ => return None,
        })
    }
    pub const NAMES: [&'static str; 5] = ["Drop Shadow", "Blur", "Glow", "Bevel", "Adjust Color"];
    pub fn name(&self) -> &'static str {
        match self {
            Filter::DropShadow { .. } => "Drop Shadow",
            Filter::Blur { .. } => "Blur",
            Filter::Glow { .. } => "Glow",
            Filter::Bevel { .. } => "Bevel",
            Filter::AdjustColor { .. } => "Adjust Color",
        }
    }
    /// How far (in px, before view scaling) this filter can paint outside the object.
    pub fn margin(&self) -> f32 {
        match self {
            Filter::DropShadow {
                blur_x,
                blur_y,
                distance,
                ..
            }
            | Filter::Bevel {
                blur_x,
                blur_y,
                distance,
                ..
            } => blur_x.max(*blur_y) * 1.5 + distance.abs() + 2.0,
            Filter::Blur { blur_x, blur_y, .. } | Filter::Glow { blur_x, blur_y, .. } => {
                blur_x.max(*blur_y) * 1.5 + 2.0
            }
            Filter::AdjustColor { .. } => 0.0,
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
                (None, Some(y)) => {
                    if t > 0.0 {
                        Some(y.clone())
                    } else {
                        None
                    }
                }
                (None, None) => None,
            })
            .collect()
    }

    pub fn lerp(&self, o: &Filter, t: f32) -> Filter {
        let l = |a: f32, b: f32| a + (b - a) * t;
        let pick = |a: bool, b: bool| if t < 0.5 { a } else { b };
        match (self, o) {
            (
                Filter::DropShadow {
                    blur_x,
                    blur_y,
                    strength,
                    quality,
                    angle,
                    distance,
                    color,
                    knockout,
                    inner,
                    hide_object,
                },
                Filter::DropShadow {
                    blur_x: bx,
                    blur_y: by,
                    strength: s,
                    angle: an,
                    distance: d,
                    color: c,
                    knockout: k,
                    inner: i,
                    hide_object: h,
                    ..
                },
            ) => Filter::DropShadow {
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
            (
                Filter::Blur {
                    blur_x,
                    blur_y,
                    quality,
                },
                Filter::Blur {
                    blur_x: bx,
                    blur_y: by,
                    ..
                },
            ) => Filter::Blur {
                blur_x: l(*blur_x, *bx),
                blur_y: l(*blur_y, *by),
                quality: *quality,
            },
            (
                Filter::Glow {
                    blur_x,
                    blur_y,
                    strength,
                    quality,
                    color,
                    knockout,
                    inner,
                },
                Filter::Glow {
                    blur_x: bx,
                    blur_y: by,
                    strength: s,
                    color: c,
                    knockout: k,
                    inner: i,
                    ..
                },
            ) => Filter::Glow {
                blur_x: l(*blur_x, *bx),
                blur_y: l(*blur_y, *by),
                strength: l(*strength, *s),
                quality: *quality,
                color: color.lerp(*c, t),
                knockout: pick(*knockout, *k),
                inner: pick(*inner, *i),
            },
            (
                Filter::Bevel {
                    blur_x,
                    blur_y,
                    strength,
                    quality,
                    shadow,
                    highlight,
                    angle,
                    distance,
                    knockout,
                    kind,
                },
                Filter::Bevel {
                    blur_x: bx,
                    blur_y: by,
                    strength: s,
                    shadow: sh,
                    highlight: hi,
                    angle: an,
                    distance: d,
                    knockout: k,
                    ..
                },
            ) => Filter::Bevel {
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
            (
                Filter::AdjustColor {
                    brightness,
                    contrast,
                    saturation,
                    hue,
                },
                Filter::AdjustColor {
                    brightness: b,
                    contrast: c,
                    saturation: s,
                    hue: h,
                },
            ) => Filter::AdjustColor {
                brightness: l(*brightness, *b),
                contrast: l(*contrast, *c),
                saturation: l(*saturation, *s),
                hue: l(*hue, *h),
            },
            _ => {
                if t < 0.5 {
                    self.clone()
                } else {
                    o.clone()
                }
            }
        }
    }
}

/// Effects applied to everything one element draws (one offscreen layer).
#[derive(Clone, Debug, PartialEq)]
pub struct FxGroup {
    pub filters: Vec<Filter>,
    pub blend: BlendMode,
}
