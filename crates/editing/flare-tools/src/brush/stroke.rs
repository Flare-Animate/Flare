//! How wide a brush stroke is at each of its points: the pen's pressure through the brush's
//! curve, the speed of the hand, and the tapers at its ends. The live preview and the finished
//! outline both take their widths from here, so what is drawn while dragging is what is kept.

use std::f64::consts::FRAC_PI_2;

use flare_input::{InputSample, PressureCurve};
use glamx::DVec2;

use super::{Nib, SPEED_REACH};

/// The longest taper, in stage units.
pub const MAX_TAPER: f64 = 2000.0;

/// How a stroke thins at its start and its end: over a length of the line, the pressure eases
/// in from none (and out to none) along a quarter sine, which starts and ends a line more softly
/// than a straight ramp. A stroke shorter than the two lengths together tapers over all of
/// itself, both lengths cut in proportion. As it acts on pressure, the tip is as thin as the
/// brush's lightest touch, and a mouse, which presses fully, gets tapered ends too.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Taper {
    start: f64,
    end: f64,
}

impl Taper {
    /// No taper at either end.
    pub const NONE: Self = Self {
        start: 0.0,
        end: 0.0,
    };

    /// A taper over `start` stage units at the start and `end` at the end, each kept between 0
    /// and [`MAX_TAPER`] (a value that is not a number is no taper).
    pub fn new(start: f64, end: f64) -> Self {
        let keep = |length: f64| {
            if length.is_finite() {
                length.clamp(0.0, MAX_TAPER)
            } else {
                0.0
            }
        };
        Self {
            start: keep(start),
            end: keep(end),
        }
    }

    pub fn start(&self) -> f64 {
        self.start
    }

    pub fn end(&self) -> f64 {
        self.end
    }

    /// The share of the pen's pressure kept at `along` stage units into a line `total` long,
    /// from 0 at a tapered tip to 1 outside the tapers.
    pub fn factor(&self, along: f64, total: f64) -> f64 {
        let wanted = self.start + self.end;
        if wanted <= 0.0 || !(total.is_finite() && total > 0.0) {
            return 1.0;
        }
        let cut = if wanted > total { total / wanted } else { 1.0 };
        let (start, end) = (self.start * cut, self.end * cut);
        let along = along.clamp(0.0, total);
        let mut factor = 1.0;
        if start > 0.0 && along < start {
            factor *= (along / start * FRAC_PI_2).sin();
        }
        let left = total - along;
        if end > 0.0 && left < end {
            factor *= (left / end * FRAC_PI_2).sin();
        }
        factor
    }
}

/// Everything about the brush that decides a stroke's shape: the width at full pressure, the
/// pressure curve, the nib, how much the width answers speed instead of pressure, and the tapers.
#[derive(Clone, Debug, PartialEq)]
pub struct BrushStroke {
    pub width: f64,
    pub curve: PressureCurve,
    pub nib: Nib,
    /// From 0 to 1. The samples are one frame apart, so a long step between them is a fast
    /// move; at full `velocity` a fast move thins the line down to the curve's minimum and a slow
    /// one keeps it at its full width, which is how a Grease Pencil or an ink pen seems to behave
    /// without a tablet.
    pub velocity: f32,
    pub taper: Taper,
}

impl BrushStroke {
    /// A round brush `width` wide that follows `curve`, with no speed response or taper.
    pub fn new(width: f64, curve: PressureCurve) -> Self {
        Self {
            width,
            curve,
            nib: Nib::ROUND,
            velocity: 0.0,
            taper: Taper::NONE,
        }
    }

    /// The points of `samples` a stroke is made of: a sample where the pen did not move adds
    /// nothing to the line, so only the first at each place is kept.
    pub fn spine(samples: &[InputSample]) -> Vec<InputSample> {
        let mut kept: Vec<InputSample> = Vec::with_capacity(samples.len());
        for s in samples {
            if kept
                .last()
                .is_none_or(|last| (last.x, last.y) != (s.x, s.y))
            {
                kept.push(*s);
            }
        }
        kept
    }

    /// The fraction of the full width at each point of `spine` (as [`BrushStroke::spine`] gives
    /// it), from the curve's minimum to 1.
    pub fn width_factors(&self, spine: &[InputSample]) -> Vec<f64> {
        let velocity = if self.velocity.is_finite() {
            f64::from(self.velocity.clamp(0.0, 1.0))
        } else {
            0.0
        };
        let at = |i: usize| DVec2::new(spine[i].x, spine[i].y);
        let mut along = Vec::with_capacity(spine.len());
        let mut total = 0.0;
        for i in 0..spine.len() {
            if i > 0 {
                total += (at(i) - at(i - 1)).length();
            }
            along.push(total);
        }
        let min = f64::from(self.curve.min());
        let last = spine.len().saturating_sub(1);
        (0..spine.len())
            .map(|i| {
                let pressure = spine[i].pressure * self.taper.factor(along[i], total) as f32;
                let from_pressure = f64::from(self.curve.width_factor(pressure));
                if velocity <= 0.0 {
                    return from_pressure;
                }
                // The step to the neighbours, as a fraction of the step that counts as "at speed".
                let step = (at((i + 1).min(last)) - at(i.saturating_sub(1))).length();
                let at_speed = (step / 2.0 / SPEED_REACH).clamp(0.0, 1.0);
                let from_speed = 1.0 - (1.0 - min) * at_speed;
                from_pressure + (from_speed - from_pressure) * velocity
            })
            .collect()
    }

    /// How far the stroke reaches to each side of each point of `spine`: half the width there,
    /// times how far the nib reaches across the line in the direction it is heading. This is what
    /// a ribbon preview of the stroke draws.
    pub fn half_widths(&self, spine: &[InputSample]) -> Vec<f64> {
        let last = spine.len().saturating_sub(1);
        self.width_factors(spine)
            .into_iter()
            .enumerate()
            .map(|(i, factor)| {
                let (a, b) = (spine[i.saturating_sub(1)], spine[(i + 1).min(last)]);
                let reach = self.nib.reach_across(DVec2::new(b.x - a.x, b.y - a.y));
                self.width / 2.0 * factor * reach
            })
            .collect()
    }
}
