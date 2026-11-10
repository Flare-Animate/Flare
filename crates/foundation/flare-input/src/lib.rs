//! What a pointing device reports, whatever the device: a mouse, a pen on a tablet, or a finger.
//!
//! The contract is deliberately small. A sample is a position, a pressure and the kind of device
//! it came from; a device that cannot sense pressure reports full pressure and says it is a
//! mouse, so tools never need a special case for "no tablet". Turning raw device events into
//! samples is the front end's job (`flare-desktop`'s window, a browser's pointer events); what to
//! do with them is the tools'.
#![forbid(unsafe_code)]

mod pen;

pub use pen::{PenFeed, PressureAxis, pen_pressure};

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
/// lighter touch, above 1 needs a firmer one). A curve drawn point by point [`PressureCurve::from_points`]
/// replaces `gamma`: the width follows the line joining the points instead.
#[derive(Clone, Debug, PartialEq)]
pub struct PressureCurve {
    min: f32,
    gamma: f32,
    /// The drawn pressure-to-width points, `(0.0..=1.0, 0.0..=1.0)`, in order of pressure.
    /// Empty means the `gamma` shape is used.
    points: Vec<(f32, f32)>,
}

/// Most points a drawn pressure curve keeps.
pub const MAX_CURVE_POINTS: usize = 16;

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
            points: Vec::new(),
        }
    }

    /// A curve drawn point by point, as Krita's and Blender Grease Pencil's strength or size
    /// lists are. Points outside the unit square or not finite are dropped, the rest come back
    /// in order of pressure with no two on the same pressure, and the lowest pressure is
    /// `min` while the highest keeps full width. Fewer than [`MAX_CURVE_POINTS`] are taken;
    /// fewer usable than two fall back to the plain `min`/`gamma` shape.
    pub fn from_points(min: f32, points: &[(f32, f32)]) -> Self {
        let mut curve = Self::new(min, 1.0);
        let mut drawn: Vec<(f32, f32)> = points
            .iter()
            .copied()
            .filter(|(pressure, width)| {
                pressure.is_finite()
                    && width.is_finite()
                    && *pressure >= 0.0
                    && *pressure <= 1.0
                    && *width >= 0.0
                    && *width <= 1.0
            })
            .take(MAX_CURVE_POINTS)
            .collect();
        drawn.sort_by(|a, b| a.0.total_cmp(&b.0));
        drawn.dedup_by(|a, b| a.0 == b.0);
        // One point carries no line between two widths: the plain shape then.
        if drawn.len() < 2 {
            return curve;
        }
        // The ends are the curve's own: full width somewhere above no pressure.
        if drawn[0].0 > 0.0 {
            drawn.insert(0, (0.0, curve.min));
        } else {
            drawn[0].1 = curve.min;
        }
        let last = drawn.len() - 1;
        if drawn[last].0 < 1.0 {
            drawn.push((1.0, 1.0));
        } else {
            drawn[last].1 = 1.0;
        }
        // Ending points both in, at most MAX_CURVE_POINTS stay on, spread out over the line.
        if drawn.len() > MAX_CURVE_POINTS {
            let stepable = drawn.len() - 1;
            drawn = (0..MAX_CURVE_POINTS)
                .map(|i| drawn[i * stepable / (MAX_CURVE_POINTS - 1)])
                .collect();
        }
        curve.points = drawn;
        curve
    }

    /// The width fraction at no pressure.
    pub fn min(&self) -> f32 {
        self.min
    }

    /// The drawn points, empty when the `gamma` shape is in use.
    pub fn points(&self) -> &[(f32, f32)] {
        &self.points
    }

    /// The fraction of the full width for `pressure`, from `min` to 1.
    pub fn width_factor(&self, pressure: f32) -> f32 {
        if self.points.is_empty() {
            let p = pressure.clamp(0.0, 1.0).powf(self.gamma);
            return self.min + (1.0 - self.min) * p;
        }
        let p = pressure.clamp(0.0, 1.0);
        let after = self
            .points
            .partition_point(|(at, _)| *at < p)
            .clamp(1, self.points.len() - 1);
        let (before, b) = self.points[after - 1];
        let (after_x, a) = self.points[after];
        if after_x <= before {
            return b;
        }
        let t = ((p - before) / (after_x - before)).clamp(0.0, 1.0);
        b + (a - b) * t
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

#[cfg(test)]
mod pen_tests;
