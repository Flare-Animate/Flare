//! Pen pressure that arrives beside the window's own events: a native tablet source (the X
//! server's XInput2, on Linux) writes it from its thread and the stage reads it each frame.

use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

/// How long a hovering pen still counts as the device in use. A mouse used after the pen must not
/// inherit the pen's last (near zero) reading, and a pen that stopped reporting has gone.
const HOVER_FRESH: Duration = Duration::from_millis(300);

/// The range a device reports pressure over, so any tablet's numbers become 0 to 1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PressureAxis {
    min: f64,
    max: f64,
}

impl PressureAxis {
    /// The axis, or `None` when the range is not a real one (not finite, or empty), which is
    /// what a device without pressure reports.
    pub fn new(min: f64, max: f64) -> Option<Self> {
        (min.is_finite() && max.is_finite() && max > min).then_some(Self { min, max })
    }

    /// `raw` as a fraction of the range, clamped to 0 to 1.
    pub fn normalize(&self, raw: f64) -> f32 {
        (((raw - self.min) / (self.max - self.min)).clamp(0.0, 1.0)) as f32
    }
}

#[derive(Clone, Copy, Debug)]
struct State {
    pressure: f32,
    /// The tip is on the surface; the reading holds however long the pen rests there.
    down: bool,
    at: Instant,
}

/// The latest pressure a native tablet source has seen. Cloning shares it; a feed nobody writes
/// to reads as no pen, so the stage treats the device as a mouse.
#[derive(Clone, Debug, Default)]
pub struct PenFeed {
    state: Arc<Mutex<Option<State>>>,
}

impl PenFeed {
    fn write(&self, state: Option<State>) {
        // A reader that panicked cannot leave a half-written plain value, so keep going.
        *self.state.lock().unwrap_or_else(PoisonError::into_inner) = state;
    }

    /// The pen touches the surface with `pressure` (0 to 1).
    pub fn press(&self, pressure: f32, now: Instant) {
        self.write(Some(State {
            pressure: pressure.clamp(0.0, 1.0),
            down: true,
            at: now,
        }));
    }

    /// The pen is near the surface and reports `pressure`.
    pub fn hover(&self, pressure: f32, now: Instant) {
        self.write(Some(State {
            pressure: pressure.clamp(0.0, 1.0),
            down: false,
            at: now,
        }));
    }

    /// The pen left the surface: until it reports again, the device in use is not it.
    pub fn lift(&self) {
        self.write(None);
    }

    /// The pressure of the pen if it is the device in use at `now`.
    pub fn reading(&self, now: Instant) -> Option<f32> {
        let state = (*self.state.lock().unwrap_or_else(PoisonError::into_inner))?;
        (state.down || now.saturating_duration_since(state.at) < HOVER_FRESH)
            .then_some(state.pressure)
    }
}
