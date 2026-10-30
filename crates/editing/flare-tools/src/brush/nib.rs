//! The tip a brush paints with: an ellipse or a rectangle, turned to an angle and squashed to a
//! flatness, as in Animate's Nib Options.

use glamx::DVec2;

/// Flattest nib allowed, as a fraction of its full width.
const MIN_FLATNESS: f64 = 0.01;

/// The outline of a nib.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NibShape {
    #[default]
    Round,
    Square,
}

/// A nib: its shape, the angle of its long axis in degrees, and how thick it is across that axis
/// relative to along it. A round nib at full flatness is a circle, so it paints like an ordinary
/// brush.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Nib {
    shape: NibShape,
    angle_degrees: f64,
    flatness: f64,
}

impl Default for Nib {
    fn default() -> Self {
        Self::ROUND
    }
}

impl Nib {
    /// The circular nib every brush starts with.
    pub const ROUND: Self = Self {
        shape: NibShape::Round,
        angle_degrees: 0.0,
        flatness: 1.0,
    };

    /// A nib of `shape`. `angle_degrees` is wrapped into a turn and `flatness_percent` kept
    /// between 1 and 100; a value that is not a number falls back to the plain setting.
    pub fn new(shape: NibShape, angle_degrees: f64, flatness_percent: f64) -> Self {
        let angle = if angle_degrees.is_finite() {
            angle_degrees.rem_euclid(360.0)
        } else {
            0.0
        };
        let flatness = if flatness_percent.is_finite() {
            (flatness_percent / 100.0).clamp(MIN_FLATNESS, 1.0)
        } else {
            1.0
        };
        Self {
            shape,
            angle_degrees: angle,
            flatness,
        }
    }

    pub fn shape(&self) -> NibShape {
        self.shape
    }

    pub fn angle_degrees(&self) -> f64 {
        self.angle_degrees
    }

    /// The flatness as a percentage, 1 to 100.
    pub fn flatness_percent(&self) -> f64 {
        self.flatness * 100.0
    }

    /// How far a unit nib reaches to each side of a line travelling along `direction`: 1 for a
    /// circle, less for a flat nib crossing its long axis. A stopped pointer gets the longest
    /// reach.
    pub fn reach_across(&self, direction: DVec2) -> f64 {
        let along = direction.normalize_or_zero();
        if along == DVec2::ZERO {
            return 1.0;
        }
        let side = DVec2::new(-along.y, along.x);
        self.support(side).dot(side).abs()
    }

    /// The closed outline of the nib at `radius`, `steps` points around it, for drawing a cursor.
    pub fn outline_points(&self, radius: f64, steps: usize) -> Vec<DVec2> {
        let steps = steps.clamp(8, 256);
        (0..steps)
            .map(|k| {
                let angle = std::f64::consts::TAU * k as f64 / steps as f64;
                self.support(DVec2::new(angle.cos(), angle.sin())) * radius
            })
            .collect()
    }

    /// The point of a unit nib farthest in the direction `toward`; the nib is symmetric, so the
    /// far side is the negative of it. A zero direction gives no offset.
    pub(super) fn support(&self, toward: DVec2) -> DVec2 {
        let (sin, cos) = self.angle_degrees.to_radians().sin_cos();
        let (along, across) = (DVec2::new(cos, sin), DVec2::new(-sin, cos));
        let (x, y) = (toward.dot(along), toward.dot(across));
        let b = self.flatness;
        let (u, v) = match self.shape {
            NibShape::Round => {
                let reach = (x * x + b * b * y * y).sqrt();
                if reach <= f64::EPSILON {
                    return DVec2::ZERO;
                }
                (x / reach, b * b * y / reach)
            }
            NibShape::Square => {
                if toward == DVec2::ZERO {
                    return DVec2::ZERO;
                }
                (
                    if x >= 0.0 { 1.0 } else { -1.0 },
                    if y >= 0.0 { b } else { -b },
                )
            }
        };
        along * u + across * v
    }
}
