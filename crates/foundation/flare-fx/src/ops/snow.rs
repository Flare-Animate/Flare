//! Falling snow over the stage. Every flake comes from the seed and its own number alone, and
//! where it is follows from the frame, so a frame always shows the same snow: scrubbing back
//! and exporting give the picture the stage showed. Flakes nearer the eye are larger, faster and
//! brighter; each sways side to side as it falls and wraps round the stage's edges.

use flare_core::Rgba8;

use crate::Pixels;

/// What [`snow`] draws, with lengths already in pixels.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Snowfall {
    pub flakes: u32,
    /// A near flake's width.
    pub size: f64,
    /// How far a near flake falls in one frame.
    pub speed: f64,
    /// How far a near flake is carried sideways in one frame.
    pub wind: f64,
    /// How far a flake swings to each side.
    pub sway: f64,
    pub seed: u32,
    pub color: Rgba8,
    pub opacity: f64,
}

/// A small, fast generator of numbers that are the same for the same start (SplitMix64).
struct Dice(u64);

impl Dice {
    fn new(seed: u32, flake: u32) -> Self {
        Self((u64::from(seed) << 32 | u64::from(flake)) ^ 0x5DEE_CE66_D1CE_4E5B)
    }

    /// A number from 0 to 1.
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        (z >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// Draws `fall` over `pixels` at `frame`, inside the stage rectangle `stage` (left, top, right,
/// bottom, in pixels), which flakes never leave.
pub(crate) fn snow(pixels: &mut Pixels, fall: &Snowfall, stage: [f64; 4], frame: u32) {
    let [left, top, right, bottom] = stage;
    let (width, height) = (right - left, bottom - top);
    if !(width > 0.0 && height > 0.0) || fall.opacity <= 0.0 {
        return;
    }
    let t = f64::from(frame);
    let margin = fall.size + fall.sway;
    let (span_w, span_h) = (width + 2.0 * margin, height + 2.0 * margin);
    // Only the part of the stage inside the picture is drawn on.
    let clip = [
        left.max(0.0),
        top.max(0.0),
        right.min(f64::from(pixels.width())),
        bottom.min(f64::from(pixels.height())),
    ];
    if clip[0] >= clip[2] || clip[1] >= clip[3] {
        return;
    }
    for flake in 0..fall.flakes {
        let mut dice = Dice::new(fall.seed, flake);
        let depth = 0.45 + 0.55 * dice.next();
        let radius = fall.size * depth * (0.7 + 0.3 * dice.next()) / 2.0;
        let speed = fall.speed * depth * (0.8 + 0.4 * dice.next());
        let start = [dice.next() * span_w, dice.next() * span_h];
        let phase = dice.next() * std::f64::consts::TAU;
        let rate = 0.03 + 0.05 * dice.next();
        let swing = fall.sway * depth * (phase + rate * t).sin();
        let x = (start[0] + fall.wind * depth * t + swing).rem_euclid(span_w) - margin + left;
        let y = (start[1] + speed * t).rem_euclid(span_h) - margin + top;
        let alpha = fall.opacity * (0.55 + 0.45 * depth);
        disc(pixels, [x, y], radius, fall.color, alpha, clip);
    }
}

/// A soft round flake at `center`, `radius` pixels, laid over `pixels` inside `clip`.
fn disc(
    pixels: &mut Pixels,
    center: [f64; 2],
    radius: f64,
    color: Rgba8,
    alpha: f64,
    clip: [f64; 4],
) {
    // A flake smaller than a pixel is a faint pixel, not nothing.
    let (radius, alpha) = if radius < 0.5 {
        (0.5, alpha * (radius / 0.5).powi(2))
    } else {
        (radius, alpha)
    };
    let soft = 0.5 + radius * 0.5;
    let reach = radius + 0.5;
    let x0 = (center[0] - reach).floor().max(clip[0]) as i64;
    let x1 = (center[0] + reach).ceil().min(clip[2]) as i64;
    let y0 = (center[1] - reach).floor().max(clip[1]) as i64;
    let y1 = (center[1] + reach).ceil().min(clip[3]) as i64;
    let width = pixels.width() as usize;
    let strength = alpha * f64::from(color.a) / 255.0;
    let data = pixels.data_mut();
    for y in y0..y1 {
        for x in x0..x1 {
            let (dx, dy) = (x as f64 + 0.5 - center[0], y as f64 + 0.5 - center[1]);
            let coverage = ((reach - (dx * dx + dy * dy).sqrt()) / soft).clamp(0.0, 1.0);
            let a = coverage * strength;
            if a <= 0.0 {
                continue;
            }
            let i = (y as usize * width + x as usize) * 4;
            let Some(p) = data.get_mut(i..i + 4) else {
                continue;
            };
            let rest = 1.0 - a;
            for (c, value) in [color.r, color.g, color.b, 255].into_iter().enumerate() {
                let mixed = f64::from(value) * a + f64::from(p[c]) * rest;
                p[c] = mixed.round().clamp(0.0, 255.0) as u8;
            }
        }
    }
}
