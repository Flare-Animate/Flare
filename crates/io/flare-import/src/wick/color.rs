//! Colours in Wick. A Paper.js colour is an array of red, green, blue and an optional alpha, each
//! from 0 to 1; anything else (a gradient, for one) is reported rather than guessed. A project's
//! background is a CSS string such as `rgb(35,35,35)`.

use flare_core::Rgba8;
use serde_json::Value;

/// What a colour field held.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Paint {
    /// No field, or `null`.
    None,
    Solid(Rgba8),
    /// Present, but not a solid colour (a gradient). Not imported.
    Other,
}

pub(super) fn paint(value: Option<&Value>) -> Paint {
    match value {
        None | Some(Value::Null) => Paint::None,
        Some(Value::Array(parts)) if parts.len() == 3 || parts.len() == 4 => {
            let numbers: Option<Vec<f64>> = parts.iter().map(Value::as_f64).collect();
            match numbers {
                Some(n) => {
                    let alpha = n.get(3).copied().unwrap_or(1.0);
                    Paint::Solid(rgba(n[0], n[1], n[2], alpha))
                }
                None => Paint::Other,
            }
        }
        Some(_) => Paint::Other,
    }
}

/// `rgb(r,g,b)` or `rgba(r,g,b,a)`, with channels from 0 to 255 and alpha from 0 to 1. `None` for
/// anything else.
pub(super) fn css(text: &str) -> Option<Rgba8> {
    let inner = text.trim();
    let inner = inner
        .strip_prefix("rgba(")
        .or_else(|| inner.strip_prefix("rgb("))?
        .strip_suffix(')')?;
    let parts: Vec<f64> = inner
        .split(',')
        .map(|p| p.trim().parse::<f64>().ok())
        .collect::<Option<Vec<f64>>>()?;
    match parts.as_slice() {
        [r, g, b] => Some(rgba8(*r, *g, *b, 255.0)),
        [r, g, b, a] => Some(rgba8(*r, *g, *b, a * 255.0)),
        _ => None,
    }
}

fn rgba(r: f64, g: f64, b: f64, a: f64) -> Rgba8 {
    rgba8(r * 255.0, g * 255.0, b * 255.0, a * 255.0)
}

fn rgba8(r: f64, g: f64, b: f64, a: f64) -> Rgba8 {
    let c = |v: f64| v.round().clamp(0.0, 255.0) as u8;
    Rgba8::new(c(r), c(g), c(b), c(a))
}
