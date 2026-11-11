//! Steadying a hand: each sample is replaced by the average of the last few, so wobble and
//! pressure flicker are smoothed out before a tool sees them.
//!
//! The level is the number of samples averaged, so the line trails the pen by about half of it.
//! With `by_speed` the window shortens when the pen moves far between two samples (a quick,
//! deliberate move needs less steadying and should not lag) and grows back at half that rate. The
//! window can be fractional: the next older sample then counts with the fraction as its weight,
//! so the window changes length without the line jumping.

use std::collections::VecDeque;

use crate::InputSample;

/// The longest window, in samples.
pub const MAX_STABILIZER_LEVEL: u8 = 40;
/// The shortest window a speed-shortened stabilizer goes down to.
const MIN_SPEED_WINDOW: f64 = 2.0;
/// Below this level the window is not changed by speed: there is too little to shorten.
const MIN_SPEED_LEVEL: u8 = 3;

/// A moving average over the last `level` samples of position and pressure.
#[derive(Clone, Debug)]
pub struct Stabilizer {
    level: u8,
    /// The step between two samples, in stage units, at which the window stops growing; 0 when
    /// the window does not follow speed.
    reference_step: f64,
    /// The number of samples averaged now, between [`MIN_SPEED_WINDOW`] and `level`.
    window: f64,
    /// The newest sample last; never more than `level`, the most a window reads.
    recent: VecDeque<InputSample>,
    /// The last sample handed in, which the line catches up with on [`Stabilizer::finish`].
    last_raw: Option<InputSample>,
}

impl Stabilizer {
    /// A stabilizer averaging `level` samples (0 or 1 hands samples through unchanged; above
    /// [`MAX_STABILIZER_LEVEL`] is kept at it). `by_speed` shortens the window on fast moves;
    /// `zoom` is how many screen pixels a stage unit is, since how fast a hand moves is a matter
    /// of the screen and not of the stage.
    pub fn new(level: u8, by_speed: bool, zoom: f64) -> Self {
        let level = level.min(MAX_STABILIZER_LEVEL);
        let zoom = if zoom.is_finite() && zoom > 0.0 {
            zoom
        } else {
            1.0
        };
        let reference_step = if by_speed && level >= MIN_SPEED_LEVEL {
            (2.0 * f64::from(level) + 100.0) / zoom
        } else {
            0.0
        };
        Self {
            level,
            reference_step,
            window: f64::from(level),
            recent: VecDeque::with_capacity(usize::from(level)),
            last_raw: None,
        }
    }

    /// The number of samples averaged.
    pub fn level(&self) -> u8 {
        self.level
    }

    /// Takes the next sample and gives the steadied one.
    pub fn push(&mut self, sample: InputSample) -> InputSample {
        self.last_raw = Some(sample);
        if self.level <= 1 {
            return sample;
        }
        if self.reference_step > 0.0
            && let Some(previous) = self.recent.back()
        {
            let step = (sample.x - previous.x).hypot(sample.y - previous.y);
            let mut change = (self.reference_step - step) / self.reference_step;
            if change > 0.0 {
                change *= 0.5;
            }
            self.window = (self.window + change).clamp(MIN_SPEED_WINDOW, f64::from(self.level));
        }
        if self.recent.len() >= usize::from(self.level) {
            self.recent.pop_front();
        }
        self.recent.push_back(sample);
        self.average(sample)
    }

    /// The samples that bring the line from where it trails to where the pen last was, so a
    /// stroke ends where it was let go. Empty when nothing trails.
    pub fn finish(&mut self) -> Vec<InputSample> {
        let Some(last) = self.last_raw else {
            return Vec::new();
        };
        if self.level <= 1 {
            return Vec::new();
        }
        // Once the whole window holds the last sample, the average is exactly that sample.
        (0..self.level).map(|_| self.push(last)).collect()
    }

    /// The weighted average of the window, the newest sample first. While fewer samples than the
    /// window have come in, the first one stands in for the rest, so the line starts where the
    /// pen went down.
    fn average(&self, newest: InputSample) -> InputSample {
        let whole = self.window.floor();
        let fraction = self.window - whole;
        let count = whole as usize + usize::from(fraction > 0.0);
        // push always adds a sample first, but an empty window must not underflow.
        let Some(oldest) = self.recent.len().checked_sub(1) else {
            return newest;
        };
        let (mut x, mut y, mut pressure, mut total) = (0.0, 0.0, 0.0, 0.0);
        for age in 0..count {
            let weight = if (age as f64) < whole { 1.0 } else { fraction };
            let s = self.recent[oldest - age.min(oldest)];
            x += s.x * weight;
            y += s.y * weight;
            pressure += f64::from(s.pressure) * weight;
            total += weight;
        }
        if total <= 0.0 {
            return newest;
        }
        InputSample {
            x: x / total,
            y: y / total,
            pressure: (pressure / total).clamp(0.0, 1.0) as f32,
            kind: newest.kind,
        }
    }
}
