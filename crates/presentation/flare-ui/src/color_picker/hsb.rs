//! Hue, saturation and brightness over the sRGB values, as Animate's and most painting programs'
//! color pickers count them: pure red is 0°, 100%, 100%, and the grey of 128 is 50% bright. (egui's
//! own `Hsva` is over linear light, so its numbers are not the ones a person reads elsewhere.)

use egui::Color32;

/// A color as the picker edits it, every part from 0 to 1; the hue is a fraction of a turn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hsb {
    pub hue: f32,
    pub saturation: f32,
    pub brightness: f32,
    pub alpha: f32,
}

fn unit(v: f32) -> f32 {
    if v.is_finite() {
        v.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn byte(v: f32) -> u8 {
    (unit(v) * 255.0).round() as u8
}

impl Hsb {
    /// The parts of a color. A grey has no hue of its own, and gets 0.
    pub fn from_color(color: Color32) -> Self {
        let [r, g, b, a] = color.to_srgba_unmultiplied().map(|c| f32::from(c) / 255.0);
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let span = max - min;
        let hue = if span <= 0.0 {
            0.0
        } else if max == r {
            ((g - b) / span).rem_euclid(6.0) / 6.0
        } else if max == g {
            ((b - r) / span + 2.0) / 6.0
        } else {
            ((r - g) / span + 4.0) / 6.0
        };
        Self {
            hue,
            saturation: if max <= 0.0 { 0.0 } else { span / max },
            brightness: max,
            alpha: a,
        }
    }

    /// The color these parts make. Parts out of range (or not numbers) are brought into range.
    pub fn to_color(self) -> Color32 {
        let [r, g, b] = self.rgb();
        Color32::from_rgba_unmultiplied(byte(r), byte(g), byte(b), byte(self.alpha))
    }

    /// The red, green and blue, from 0 to 1, with no alpha.
    pub fn rgb(self) -> [f32; 3] {
        let (s, v) = (unit(self.saturation), unit(self.brightness));
        let h = if self.hue.is_finite() {
            self.hue.rem_euclid(1.0) * 6.0
        } else {
            0.0
        };
        let sector = h.floor();
        let f = h - sector;
        let (p, q, t) = (v * (1.0 - s), v * (1.0 - s * f), v * (1.0 - s * (1.0 - f)));
        match sector as u8 {
            0 => [v, t, p],
            1 => [q, v, p],
            2 => [p, v, t],
            3 => [p, q, v],
            4 => [t, p, v],
            _ => [v, p, q],
        }
    }

    /// The same, opaque: what a track of the picker shows each part against.
    pub fn opaque(self) -> Color32 {
        Self { alpha: 1.0, ..self }.to_color()
    }
}
