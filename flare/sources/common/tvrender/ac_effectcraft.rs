//! Bridge to the EffectCraft effects library (ArtCraft, MIT OR Apache-2.0).
//!
//! EffectCraft is a git dependency, not a copy: its effects run from its own
//! registry (`effectcraft_effects::lookup` + `apply`), so `cargo update -p
//! effectcraft-effects` brings in their fixes and new effects. This module only
//! converts pixels and parameters between the two projects.

use ac_core::{Color, EffectValue};
use effectcraft_effects as ec;
use effectcraft_keyframe::Value;
use effectcraft_project::ParamUi;
use effectcraft_raster::Image;
use std::collections::HashMap;
use tiny_skia as sk;

/// How a parameter is edited in our Properties panel.
#[derive(Clone, Debug, PartialEq)]
pub enum ParamKind {
    /// Number with a slider range and decimals.
    Number { min: f64, max: f64, decimals: u8 },
    /// Size in pixels (scaled with the view zoom when rendering).
    Pixels,
    Angle,
    Percent,
    Checkbox,
    Color,
    /// One of these options (the value is the 0-based index).
    Popup(Vec<String>),
    /// A point, as a fraction of the object's bounds (0.5, 0.5 = centre).
    Point,
}

#[derive(Clone, Debug)]
pub struct ParamInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub kind: ParamKind,
    pub default: EffectValue,
}

#[derive(Clone, Debug)]
pub struct EffectInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub category: &'static str,
    pub params: Vec<ParamInfo>,
}

/// Categories whose effects need things an animation object can't supply
/// (audio, 3D channels, other layers over time, footage tracking).
const SKIP_CATEGORIES: [&str; 4] = ["Audio", "3D Channel", "Time", "Obsolete"];

fn kind_of(ui: &ParamUi) -> Option<ParamKind> {
    Some(match ui {
        ParamUi::Number => ParamKind::Number { min: -1000.0, max: 1000.0, decimals: 2 },
        ParamUi::Slider { slider_min, slider_max, decimals, .. } => ParamKind::Number { min: *slider_min, max: *slider_max, decimals: *decimals },
        ParamUi::Percent => ParamKind::Percent,
        ParamUi::Angle => ParamKind::Angle,
        ParamUi::Pixels => ParamKind::Pixels,
        ParamUi::Point => ParamKind::Point,
        ParamUi::Color => ParamKind::Color,
        ParamUi::Checkbox => ParamKind::Checkbox,
        ParamUi::Popup { options } => ParamKind::Popup(options.clone()),
        _ => return None,
    })
}

fn to_ours(v: &Value, kind: &ParamKind) -> Option<EffectValue> {
    Some(match v {
        Value::Scalar(n) => EffectValue::Num(*n),
        Value::Enum(i) => EffectValue::Num(*i as f64),
        Value::Bool(b) => EffectValue::Bool(*b),
        // Points are stored as fractions of the layer size in the spec.
        Value::Vec2(p) if *kind == ParamKind::Point => EffectValue::Vec2(*p),
        Value::Color(c) => EffectValue::Color(Color { r: u8c(c[0]), g: u8c(c[1]), b: u8c(c[2]), a: u8c(c[3]) }),
        _ => return None,
    })
}

fn u8c(v: f64) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// Effects that can run on an object, with their editable parameters
/// (those whose every parameter we can represent). Sorted by category, name.
pub fn catalog() -> &'static [EffectInfo] {
    static CAT: std::sync::OnceLock<Vec<EffectInfo>> = std::sync::OnceLock::new();
    CAT.get_or_init(|| {
        ec::registry()
            .iter()
            .filter(|s| !SKIP_CATEGORIES.contains(&s.category))
            .filter_map(|s| {
                let params: Option<Vec<ParamInfo>> = s
                    .params
                    .iter()
                    .map(|p| {
                        let kind = kind_of(&p.ui)?;
                        let default = to_ours(&p.default, &kind)?;
                        Some(ParamInfo { id: p.id, name: p.name, kind, default })
                    })
                    .collect();
                Some(EffectInfo { id: s.id, name: s.name, category: s.category, params: params? })
            })
            .collect()
    })
}

pub fn info(id: &str) -> Option<&'static EffectInfo> {
    catalog().iter().find(|e| e.id == id)
}

/// Run effect `id` on `pix` (premultiplied RGBA, the object's offscreen
/// layer). `scale` is the view zoom (pixel-size parameters scale with it),
/// `time` the object's timeline time in seconds. Unknown effects leave the
/// pixels unchanged and return false.
pub fn apply(pix: &mut sk::Pixmap, id: &str, params: &[(String, EffectValue)], scale: f32, time: f32) -> bool {
    let Some(spec) = ec::lookup(id) else { return false };
    let (w, h) = (pix.width(), pix.height());
    let size = [w as f64, h as f64];
    let mut values: HashMap<String, Value> = spec.params.iter().map(|p| (p.id.to_string(), ec::default_value(p, size))).collect();
    for (k, v) in params {
        let Some(ps) = spec.params.iter().find(|p| p.id == k) else { continue };
        let val = match (v, &ps.default) {
            (EffectValue::Num(n), Value::Enum(_)) => Value::Enum(n.max(0.0) as u32),
            (EffectValue::Num(n), _) if matches!(ps.ui, ParamUi::Pixels) => Value::Scalar(n * scale as f64),
            (EffectValue::Num(n), _) => Value::Scalar(*n),
            (EffectValue::Bool(b), _) => Value::Bool(*b),
            (EffectValue::Vec2(p), _) => Value::Vec2([p[0] * size[0], p[1] * size[1]]),
            (EffectValue::Color(c), _) => Value::Color([c.r as f64 / 255.0, c.g as f64 / 255.0, c.b as f64 / 255.0, c.a as f64 / 255.0]),
        };
        values.insert(k.clone(), val);
    }
    let params = ec::Params { values };
    let img = Image { width: w, height: h, data: pix.data().chunks_exact(4).map(|p| [p[0] as f32 / 255.0, p[1] as f32 / 255.0, p[2] as f32 / 255.0, p[3] as f32 / 255.0]).collect() };
    let ctx = ec::EffectCtx { params: &params, time: time as f64, layer_size: size, seed: 1, adjustment: false, env: ec::EffectEnv::default() };
    let out = ec::apply(spec, &ctx, ec::Buf { img, offset: [0.0, 0.0], scale: 1.0 });
    // The result may have grown or moved: layer point p sits at p * scale + offset.
    let same = out.img.width == w && out.img.height == h && out.offset == [0.0, 0.0] && (out.scale - 1.0).abs() < 1e-9;
    let data = pix.data_mut();
    for y in 0..h {
        for x in 0..w {
            let px = if same {
                out.img.data[(y * w + x) as usize]
            } else {
                let (sx, sy) = out.to_px([x as f64 + 0.5, y as f64 + 0.5]);
                out.img.sample_bilinear(sx, sy)
            };
            let i = ((y * w + x) * 4) as usize;
            let a = (px[3].clamp(0.0, 1.0) * 255.0).round();
            data[i + 3] = a as u8;
            for c in 0..3 {
                data[i + c] = (px[c].max(0.0) * 255.0).round().min(a) as u8;
            }
        }
    }
    true
}
