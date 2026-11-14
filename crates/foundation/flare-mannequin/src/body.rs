//! The two body types and the proportions a person can change. Lengths are in figure units: the
//! default figure stands one unit tall, its soles at zero.

use thiserror::Error;

use crate::math::Vec3;

/// Which figure: a man's build or a woman's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Body {
    #[default]
    Male,
    Female,
}

impl Body {
    pub const ALL: [Body; 2] = [Body::Male, Body::Female];

    pub fn id(self) -> &'static str {
        match self {
            Self::Male => "male",
            Self::Female => "female",
        }
    }

    pub fn from_id(id: &str) -> Option<Body> {
        Self::ALL.into_iter().find(|b| b.id() == id)
    }
}

/// A proportion that can be changed, as a multiple of the body type's own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Proportion {
    /// The size of the head.
    Head,
    /// How far apart the shoulders are.
    Shoulders,
    /// How wide the hips are.
    Hips,
    /// The length of the arms.
    Arms,
    /// The length of the legs.
    Legs,
    /// The length of the body from the hips to the neck.
    Torso,
    /// How thick every part is.
    Build,
}

impl Proportion {
    pub const ALL: [Proportion; 7] = [
        Self::Head,
        Self::Shoulders,
        Self::Hips,
        Self::Arms,
        Self::Legs,
        Self::Torso,
        Self::Build,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Self::Head => "head",
            Self::Shoulders => "shoulders",
            Self::Hips => "hips",
            Self::Arms => "arms",
            Self::Legs => "legs",
            Self::Torso => "torso",
            Self::Build => "build",
        }
    }

    pub fn from_id(id: &str) -> Option<Proportion> {
        Self::ALL.into_iter().find(|p| p.id() == id)
    }

    /// The multiples the proportion may take.
    pub fn range(self) -> (f64, f64) {
        match self {
            Self::Head => (0.7, 1.6),
            Self::Shoulders | Self::Hips => (0.7, 1.4),
            Self::Arms | Self::Legs | Self::Torso => (0.8, 1.25),
            Self::Build => (0.6, 1.6),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Error)]
pub enum BodyError {
    #[error("{name} must be between {min} and {max}, not {value}")]
    OutOfRange {
        name: &'static str,
        value: f64,
        min: f64,
        max: f64,
    },
}

/// The proportions, each a multiple of the body type's (1 is as built).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Proportions([f64; 7]);

impl Default for Proportions {
    fn default() -> Self {
        Self([1.0; 7])
    }
}

impl Proportions {
    pub fn get(&self, p: Proportion) -> f64 {
        self.0[p as usize]
    }

    /// Sets a proportion; a value outside its range or not a number is refused.
    pub fn set(&mut self, p: Proportion, value: f64) -> Result<(), BodyError> {
        let (min, max) = p.range();
        if !(value.is_finite() && (min..=max).contains(&value)) {
            return Err(BodyError::OutOfRange {
                name: p.id(),
                value,
                min,
                max,
            });
        }
        self.0[p as usize] = value;
        Ok(())
    }

    /// Sets a proportion, bringing the value into range (a slider's way).
    pub fn set_clamped(&mut self, p: Proportion, value: f64) {
        let (min, max) = p.range();
        if value.is_finite() {
            self.0[p as usize] = value.clamp(min, max);
        }
    }
}

/// Every length and thickness of one figure, worked out once from its body and proportions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Build {
    pub head: f64,
    pub shoulder_half: f64,
    pub hip_half: f64,
    pub upper_arm: f64,
    pub forearm: f64,
    pub hand: f64,
    pub thigh: f64,
    pub shin: f64,
    pub abdomen: f64,
    pub chest: f64,
    pub neck: f64,
    /// Thicknesses.
    pub pelvis_r: f64,
    pub waist_r: f64,
    pub belly_r: f64,
    pub chest_r: f64,
    pub upper_arm_r: (f64, f64),
    pub forearm_r: (f64, f64),
    pub thigh_r: (f64, f64),
    pub shin_r: (f64, f64),
    pub limb: f64,
    pub female: bool,
    /// The proportions the body's surface is shaped by.
    pub thick: f64,
    pub hips: f64,
    pub shoulders: f64,
    pub torso: f64,
}

/// The ankle stands this high, so a foot rests on the ground.
pub(crate) const ANKLE_HEIGHT: f64 = 0.04;
/// The hips sit this far below the pelvis joint.
pub(crate) const HIP_DROP: f64 = 0.04;

impl Build {
    pub fn new(body: Body, p: &Proportions) -> Self {
        let female = body == Body::Female;
        let k = p.get(Proportion::Build);
        let (arms, legs, torso) = (
            p.get(Proportion::Arms),
            p.get(Proportion::Legs),
            p.get(Proportion::Torso),
        );
        let pick = |m: f64, f: f64| if female { f } else { m };
        Self {
            head: pick(1.0, 1.04) * p.get(Proportion::Head),
            shoulder_half: pick(0.105, 0.094) * p.get(Proportion::Shoulders),
            hip_half: pick(0.055, 0.062) * p.get(Proportion::Hips),
            upper_arm: 0.17 * arms,
            forearm: 0.15 * arms,
            hand: 0.10 * arms,
            thigh: 0.235 * legs,
            shin: 0.215 * legs,
            abdomen: 0.11 * torso,
            chest: 0.13 * torso,
            neck: 0.05 * torso,
            pelvis_r: pick(0.062, 0.07) * k * p.get(Proportion::Hips).sqrt(),
            waist_r: pick(0.07, 0.056) * k,
            belly_r: pick(0.075, 0.07) * k,
            chest_r: pick(0.085, 0.074) * k * p.get(Proportion::Shoulders).sqrt(),
            upper_arm_r: (pick(0.038, 0.033) * k, pick(0.032, 0.027) * k),
            forearm_r: (pick(0.031, 0.026) * k, pick(0.024, 0.02) * k),
            thigh_r: (pick(0.062, 0.066) * k, pick(0.044, 0.042) * k),
            shin_r: (pick(0.042, 0.038) * k, pick(0.028, 0.025) * k),
            limb: pick(1.0, 0.9) * k,
            female,
            thick: k,
            hips: p.get(Proportion::Hips),
            shoulders: p.get(Proportion::Shoulders),
            torso,
        }
    }

    /// Where the pelvis joint stands with the legs straight, so the feet are on the ground.
    pub fn pelvis_height(&self) -> f64 {
        ANKLE_HEIGHT + self.shin + self.thigh + HIP_DROP
    }

    /// Where each joint sits from its parent at rest, in the parent's frame.
    pub fn offset(&self, joint: crate::Joint) -> Vec3 {
        use crate::Joint::*;
        let shoulder_y = self.chest - 0.025;
        match joint {
            Pelvis => Vec3::new(0.0, self.pelvis_height(), 0.0),
            Spine => Vec3::new(0.0, 0.06 * self.abdomen / 0.11, 0.0),
            Chest => Vec3::new(0.0, self.abdomen, 0.0),
            Neck => Vec3::new(0.0, self.chest, 0.0),
            Head => Vec3::new(0.0, self.neck, 0.0),
            LeftShoulder => Vec3::new(self.shoulder_half, shoulder_y, 0.0),
            RightShoulder => Vec3::new(-self.shoulder_half, shoulder_y, 0.0),
            LeftElbow | RightElbow => Vec3::new(0.0, -self.upper_arm, 0.0),
            LeftWrist | RightWrist => Vec3::new(0.0, -self.forearm, 0.0),
            LeftHip => Vec3::new(self.hip_half, -HIP_DROP, 0.0),
            RightHip => Vec3::new(-self.hip_half, -HIP_DROP, 0.0),
            LeftKnee | RightKnee => Vec3::new(0.0, -self.thigh, 0.0),
            LeftAnkle | RightAnkle => Vec3::new(0.0, -self.shin, 0.0),
        }
    }
}
