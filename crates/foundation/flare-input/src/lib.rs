//! What a pointing device reports, whatever the device: a mouse, a pen on a tablet, or a finger.
//!
//! The contract is deliberately small. A sample is a position, a pressure and the kind of device
//! it came from; a device that cannot sense pressure reports full pressure and says it is a
//! mouse, so tools never need a special case for "no tablet". Turning raw device events into
//! samples is the front end's job (`flare-desktop`'s window, a browser's pointer events); what to
//! do with them is the tools'.
#![forbid(unsafe_code)]

/// The kind of device a sample came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PointerKind {
    /// No pressure sensing: pressure is always full.
    #[default]
    Mouse,
    /// A pen on a tablet or screen.
    Pen,
    Touch,
}

/// One reading of a pointing device, in stage coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InputSample {
    pub x: f64,
    pub y: f64,
    /// 0 (no force) to 1 (full force). Always 1 for a mouse.
    pub pressure: f32,
    pub kind: PointerKind,
}

impl InputSample {
    /// A sample, or `None` when a coordinate is not finite. Pressure outside 0 to 1 (or not a
    /// number) is clamped, since devices differ in how they report it.
    pub fn new(x: f64, y: f64, pressure: f32, kind: PointerKind) -> Option<Self> {
        if !(x.is_finite() && y.is_finite()) {
            return None;
        }
        let pressure = if pressure.is_nan() {
            1.0
        } else {
            pressure.clamp(0.0, 1.0)
        };
        Some(Self {
            x,
            y,
            pressure,
            kind,
        })
    }

    /// A mouse reading: full pressure.
    pub fn mouse(x: f64, y: f64) -> Option<Self> {
        Self::new(x, y, 1.0, PointerKind::Mouse)
    }
}

/// How hard a pen is pressed becomes how wide a stroke is: `min` of the width at no pressure, the
/// full width at full pressure, and `gamma` bending the middle (below 1 reaches full width with a
/// lighter touch, above 1 needs a firmer one).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PressureCurve {
    min: f32,
    gamma: f32,
}

impl PressureCurve {
    /// A curve; `min` is kept in 0 to 1 and `gamma` positive, so every width is sensible.
    pub fn new(min: f32, gamma: f32) -> Self {
        Self {
            min: if min.is_finite() {
                min.clamp(0.0, 1.0)
            } else {
                0.0
            },
            gamma: if gamma.is_finite() && gamma > 0.0 {
                gamma
            } else {
                1.0
            },
        }
    }

    /// The fraction of the full width for `pressure`, from `min` to 1.
    pub fn width_factor(&self, pressure: f32) -> f32 {
        let p = pressure.clamp(0.0, 1.0).powf(self.gamma);
        self.min + (1.0 - self.min) * p
    }
}

impl Default for PressureCurve {
    /// A fine line at a light touch, full width at full pressure.
    fn default() -> Self {
        Self::new(0.15, 1.0)
    }
}

#[cfg(test)]
mod tests;
