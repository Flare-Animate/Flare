//! One dab of a Blur or Smudge stroke: where it sits, how far it reaches, how strong it is, and
//! how much of each pixel it covers. The rules follow the report on Clip Studio Paint's brushes
//! (its entry in docs/ENGINEERING.md): a one-pixel distance ramp for the edge, one Gaussian for a
//! soft edge, and small dabs drawn no thinner than 1.8 pixels.

/// What the soft profile is worth where it is cut, three standard deviations out: taken off, so a
/// soft dab fades to nothing at its rim instead of ending in a faint step.
const GAUSS_END: f32 = 0.011_108_996;

/// The thinnest a dab is drawn, in pixels. A thinner one is drawn this wide and fainter in
/// proportion, so a fine stroke fades out instead of breaking into dots.
pub(super) const MIN_DRAWN: f32 = 1.8;

/// Hardness, on Clip Studio's 0 to 255 scale, that a small soft dab is pushed towards.
const SMALL_HARD: f32 = 252.45;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Dab {
    /// The center, in bitmap pixels (pixel `(x, y)` has its center at `x + 0.5, y + 0.5`).
    pub(super) x: f32,
    pub(super) y: f32,
    /// How far from the center it reaches, after a soft dab is drawn wider than its size.
    pub(super) radius: f32,
    /// How much one dab adds, 0 to 1, before the stroke's ceiling.
    pub(super) strength: f32,
    /// The edge is a one-pixel ramp; off, it is cut at the radius (a soft edge needs no ramp).
    ramp: bool,
    soft: bool,
    /// How much the soft profile is raised before it is clipped to 1: hardness flattens its top.
    gain: f32,
}

impl Dab {
    /// A dab of `diameter` pixels at `(x, y)`, with `hardness` from 0 (soft) to 1 (a hard edge).
    pub(super) fn new(x: f32, y: f32, diameter: f32, hardness: f32, strength: f32) -> Self {
        let mut size = if diameter.is_finite() {
            diameter.max(0.0)
        } else {
            0.0
        };
        let mut hard = hardness.clamp(0.0, 1.0) * 255.0;
        let mut strength = strength.clamp(0.0, 1.0);
        let soft = hard < 255.0;
        // A large soft dab fades well inside its rim already.
        let ramp = !(soft && size > 6.0 && (255.0 - hard) * size > 50.0);
        // A small soft dab would dissolve into a faint smudge: harden it, keeping its weight.
        if soft && size < 4.0 && hard < SMALL_HARD {
            let harder = if size > 2.0 {
                hard + (SMALL_HARD - hard) * (4.0 - size) * 0.5
            } else {
                SMALL_HARD
            };
            strength *= (300.0 - harder) / (300.0 - hard);
            hard = harder;
        }
        if size < MIN_DRAWN {
            strength *= size / MIN_DRAWN;
            size = MIN_DRAWN;
        }
        let h = hard / 255.0;
        let mut radius = size / 2.0;
        let mut gain = 0.0;
        if soft {
            radius *= 1.0 + 0.75 * (1.0 - h);
            gain = radius.min(16.0) / (16.0 * (1.0 - h));
        }
        Self {
            x,
            y,
            radius,
            strength,
            ramp,
            soft,
            gain,
        }
    }

    /// How much of pixel `(px, py)` the dab covers, 0 to 1.
    pub(super) fn coverage(&self, px: u32, py: u32) -> f32 {
        let dx = px as f32 + 0.5 - self.x;
        let dy = py as f32 + 0.5 - self.y;
        let d2 = dx * dx + dy * dy;
        let r = self.radius;
        let edge = if self.ramp {
            (r - d2.sqrt()).clamp(0.0, 1.0)
        } else if d2 < r * r {
            1.0
        } else {
            0.0
        };
        if edge <= 0.0 || !self.soft {
            return edge;
        }
        let profile = ((-4.5 * d2 / (r * r)).exp() - GAUSS_END) * self.gain;
        edge * profile.clamp(0.0, 1.0)
    }

    /// The pixels the dab may touch in a `width` by `height` bitmap, as `[x0, x1) x [y0, y1)`;
    /// `None` when it is off the bitmap.
    pub(super) fn bounds(&self, width: u32, height: u32) -> Option<[u32; 4]> {
        let reach = self.radius + 1.0;
        let clip = |v: f32, max: u32| v.clamp(0.0, max as f32) as u32;
        let x0 = clip((self.x - reach).floor(), width);
        let x1 = clip((self.x + reach).ceil(), width);
        let y0 = clip((self.y - reach).floor(), height);
        let y1 = clip((self.y + reach).ceil(), height);
        (x0 < x1 && y0 < y1).then_some([x0, x1, y0, y1])
    }
}

/// The distance between dabs as a fraction of the dab's size, as Clip Studio's automatic spacing
/// has it: 6% for a hard edge, whose steps would show, up to 21% for a soft one.
pub(super) fn spacing(hardness: f32) -> f32 {
    0.06 * (1.0 + (1.0 - hardness.clamp(0.0, 1.0)) * 2.55)
}

/// What one dab adds when dabs sit `ratio` of their size apart: closer dabs each add less, so a
/// stroke builds up at the same rate whatever its spacing.
pub(super) fn per_dab(ratio: f32) -> f32 {
    if ratio < 0.33 {
        (3.0 * ratio).max(0.0)
    } else {
        1.0
    }
}

#[cfg(test)]
#[path = "dab_tests.rs"]
mod tests;
