//! Color changes that keep the shape: brightness, contrast, saturation and hue, and a tint.
//! Each pixel is taken out of premultiplied form, changed, and put back.

use flare_core::Rgba8;

use crate::Pixels;

/// What [`adjust`] does, each from -1 to 1 except the hue, in degrees.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Adjustment {
    pub brightness: f32,
    pub contrast: f32,
    pub saturation: f32,
    pub hue: f32,
}

/// The matrix that turns a color's hue by `degrees` and keeps its luminance: the standard one
/// (the W3C filter effects' `hueRotate`), built from the Rec. 709 luminance weights.
fn hue_matrix(degrees: f32) -> [[f32; 3]; 3] {
    let (sin, cos) = degrees.to_radians().sin_cos();
    [
        [
            0.213 + cos * 0.787 - sin * 0.213,
            0.715 - cos * 0.715 - sin * 0.715,
            0.072 - cos * 0.072 + sin * 0.928,
        ],
        [
            0.213 - cos * 0.213 + sin * 0.143,
            0.715 + cos * 0.285 + sin * 0.140,
            0.072 - cos * 0.072 - sin * 0.283,
        ],
        [
            0.213 - cos * 0.213 - sin * 0.787,
            0.715 - cos * 0.715 + sin * 0.715,
            0.072 + cos * 0.928 + sin * 0.072,
        ],
    ]
}

/// Changes every pixel's color by `change`; transparent pixels stay so.
pub(crate) fn adjust(pixels: &mut Pixels, change: Adjustment) {
    let hue = (change.hue != 0.0).then(|| hue_matrix(change.hue));
    let contrast = 1.0 + change.contrast;
    let saturation = 1.0 + change.saturation;
    for p in pixels.data_mut().as_chunks_mut::<4>().0.iter_mut() {
        let a = f32::from(p[3]);
        if a == 0.0 {
            continue;
        }
        let mut rgb = [0, 1, 2].map(|c| f32::from(p[c]) / a);
        if let Some(m) = hue {
            let [r, g, b] = rgb;
            rgb = [0, 1, 2].map(|row| m[row][0] * r + m[row][1] * g + m[row][2] * b);
        }
        let luma = 0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2];
        for c in &mut rgb {
            *c = luma + (*c - luma) * saturation;
            *c = (*c - 0.5) * contrast + 0.5 + change.brightness;
        }
        for c in 0..3 {
            p[c] = (rgb[c].clamp(0.0, 1.0) * a).round() as u8;
        }
    }
}

/// Mixes every pixel's color toward `color` by `amount` (0 to 1), keeping its alpha.
pub(crate) fn tint(pixels: &mut Pixels, color: Rgba8, amount: f32) {
    let amount = amount.clamp(0.0, 1.0);
    let target = [color.r, color.g, color.b].map(f32::from);
    for p in pixels.data_mut().as_chunks_mut::<4>().0.iter_mut() {
        let a = f32::from(p[3]) / 255.0;
        for c in 0..3 {
            let mixed = f32::from(p[c]) * (1.0 - amount) + target[c] * a * amount;
            p[c] = mixed.round().clamp(0.0, f32::from(p[3])) as u8;
        }
    }
}
