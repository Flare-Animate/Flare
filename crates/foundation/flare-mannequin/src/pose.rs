//! A pose: how far each joint is turned, and how far the whole figure is lifted or moved.

use thiserror::Error;

use crate::math::{Angles, Vec3, wrap_degrees};
use crate::skeleton::{JOINT_COUNT, Joint, Side};

/// How far the figure may be moved from where it stands, in figure units each way.
pub const MAX_LIFT: f64 = 2.0;

#[derive(Debug, Clone, PartialEq, Error)]
pub enum PoseError {
    #[error("the angles of {joint} must be numbers")]
    NotFinite { joint: &'static str },
    #[error("{joint} turns {axis} from {min} to {max} degrees, not {value}")]
    OutOfRange {
        joint: &'static str,
        axis: &'static str,
        value: f64,
        min: f64,
        max: f64,
    },
    #[error("the figure can be moved at most {MAX_LIFT} each way, and only by numbers")]
    Lift,
}

/// The angles of every joint, and the figure's move from where it stands.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pose {
    angles: [Angles; JOINT_COUNT],
    lift: Vec3,
}

impl Default for Pose {
    fn default() -> Self {
        Self {
            angles: [Angles::ZERO; JOINT_COUNT],
            lift: Vec3::ZERO,
        }
    }
}

impl Pose {
    pub fn angles(&self, joint: Joint) -> Angles {
        self.angles[joint.index()]
    }

    /// Turns a joint, holding the angles inside its limits (the pelvis's wrap round instead).
    pub fn set(&mut self, joint: Joint, angles: Angles) {
        if !angles.is_finite() {
            return;
        }
        let angles = if joint == Joint::Pelvis {
            Angles::new(
                wrap_degrees(angles.bend),
                wrap_degrees(angles.twist),
                wrap_degrees(angles.side),
            )
        } else {
            joint.limits().clamp(angles)
        };
        self.angles[joint.index()] = angles;
    }

    /// Turns a joint, refusing angles outside its limits, as a file or a command asks.
    pub fn set_checked(&mut self, joint: Joint, angles: Angles) -> Result<(), PoseError> {
        if !angles.is_finite() {
            return Err(PoseError::NotFinite { joint: joint.id() });
        }
        let limits = joint.limits();
        for (axis, value, (min, max)) in [
            ("bend", angles.bend, limits.bend),
            ("twist", angles.twist, limits.twist),
            ("side", angles.side, limits.side),
        ] {
            if !(min..=max).contains(&value) {
                return Err(PoseError::OutOfRange {
                    joint: joint.id(),
                    axis,
                    value,
                    min,
                    max,
                });
            }
        }
        self.angles[joint.index()] = angles;
        Ok(())
    }

    pub fn lift(&self) -> Vec3 {
        self.lift
    }

    /// Moves the figure from where it stands; held to [`MAX_LIFT`] each way.
    pub fn set_lift(&mut self, lift: Vec3) {
        if lift.is_finite() {
            self.lift = Vec3::new(
                lift.x.clamp(-MAX_LIFT, MAX_LIFT),
                lift.y.clamp(-MAX_LIFT, MAX_LIFT),
                lift.z.clamp(-MAX_LIFT, MAX_LIFT),
            );
        }
    }

    pub fn set_lift_checked(&mut self, lift: Vec3) -> Result<(), PoseError> {
        let inside = |v: f64| v.is_finite() && v.abs() <= MAX_LIFT;
        if !(inside(lift.x) && inside(lift.y) && inside(lift.z)) {
            return Err(PoseError::Lift);
        }
        self.lift = lift;
        Ok(())
    }

    /// The pose seen in a mirror: left and right swap, and the middle turns the other way.
    #[must_use]
    pub fn mirrored(&self) -> Self {
        let mut out = *self;
        for joint in Joint::ALL {
            let from = self.angles(joint.opposite());
            out.angles[joint.index()] = match joint.side() {
                // A side joint's angles are already mirrored for its side.
                Side::Left | Side::Right => from,
                Side::Center => from.mirrored(),
            };
        }
        out.lift = Vec3::new(-self.lift.x, self.lift.y, self.lift.z);
        out
    }

    /// Gives the other side of the body this side's pose, as a mirror would.
    pub fn copy_side(&mut self, from: Side) {
        for joint in Joint::ALL.into_iter().filter(|j| j.side() == from) {
            self.angles[joint.opposite().index()] = self.angles[joint.index()];
        }
    }
}
