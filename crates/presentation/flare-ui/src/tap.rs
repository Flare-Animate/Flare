//! A modifier key tapped on its own: pressed and let go quickly with nothing else pressed in
//! between, so it can do something of its own without taking anything from the chords and the
//! modified clicks (Alt-click, Alt-drag) it is part of.

/// The longest press that still counts as a tap, in seconds.
pub const TAP_SECONDS: f64 = 0.5;

/// Watches one modifier frame by frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ModifierTap {
    /// When the key went down, while it is down.
    since: Option<f64>,
    /// Something else happened while it was down: no tap.
    spoiled: bool,
}

impl ModifierTap {
    /// One frame: whether the key is down now, whether anything else is pressed or happened this
    /// frame (another key, another modifier, a button, typing), and the time in seconds. Answers
    /// whether this frame ends a tap.
    pub fn update(&mut self, down: bool, other: bool, now: f64) -> bool {
        match (self.since, down) {
            (None, false) => false,
            (None, true) => {
                self.since = Some(now);
                self.spoiled = other;
                false
            }
            (Some(_), true) => {
                self.spoiled |= other;
                false
            }
            (Some(start), false) => {
                self.since = None;
                let held = now - start;
                !self.spoiled && !other && held.is_finite() && (0.0..=TAP_SECONDS).contains(&held)
            }
        }
    }
}

#[cfg(test)]
#[path = "tap_tests.rs"]
mod tests;
