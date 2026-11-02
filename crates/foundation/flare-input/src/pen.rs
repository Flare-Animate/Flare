//! Pen pressure that arrives beside the window's own events: a native tablet source (the X
//! server's XInput2, on Linux) writes it from its thread and the stage reads it each frame.

use std::sync::{Arc, Mutex, PoisonError};

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

    /// `raw` as a fraction of the range, clamped to 0 to 1. Not a number reads as full pressure,
    /// as it does for a mouse, so a broken reading never thins a line.
    pub fn normalize(&self, raw: f64) -> f32 {
        if raw.is_nan() {
            return 1.0;
        }
        (((raw - self.min) / (self.max - self.min)).clamp(0.0, 1.0)) as f32
    }
}

/// The pressure of the pen if it is the device in use, as a native tablet source last saw it.
/// Cloning shares it; a feed nobody writes to reads as no pen, so the stage treats the device as
/// a mouse. The source says when the pen is no longer the device in use (another pointer moved),
/// so there is no timer here to guess it.
#[derive(Clone, Debug, Default)]
pub struct PenFeed {
    pressure: Arc<Mutex<Option<f32>>>,
}

impl PenFeed {
    /// The pen is the device in use with `pressure` (0 to 1), or, with `None`, it is not.
    pub fn set(&self, pressure: Option<f32>) {
        let pressure = pressure.map(|p| if p.is_nan() { 1.0 } else { p.clamp(0.0, 1.0) });
        // A writer that panicked cannot leave a half-written plain value, so keep going.
        *self.pressure.lock().unwrap_or_else(PoisonError::into_inner) = pressure;
    }

    /// The pen's pressure, if it is the device in use.
    pub fn reading(&self) -> Option<f32> {
        *self.pressure.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
