use std::collections::BTreeMap;

use flare_core::{InstanceId, SymbolId};
use flare_document::SymbolKind;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const RIG_VERSION: u32 = 1;
pub const MAX_BONES: usize = 256;
pub const MAX_POSE_KEYS: usize = 4096;
pub const MAX_IK_CHAIN: usize = 16;
pub const MAX_IK_ITERATIONS: u32 = 64;
/// Coordinates and lengths beyond this are refused so no product can overflow.
pub const MAX_COORD: f64 = 1.0e7;

#[derive(Debug, Clone, PartialEq, Error)]
pub enum RigError {
    #[error("a rig holds at most {MAX_BONES} bones")]
    TooManyBones,
    #[error("a rig holds at most {MAX_POSE_KEYS} pose keys")]
    TooManyKeys,
    #[error("no such bone {0}")]
    NoSuchBone(usize),
    #[error("bone parent {0} must be an earlier bone")]
    BadParent(usize),
    #[error("a bone name must be 1 to 64 characters")]
    BoneName,
    #[error("{0} must be a finite number within range")]
    NotFinite(&'static str),
    #[error("an angle limit needs min <= max, each within -2pi..=2pi")]
    BadLimit,
    #[error("an IK chain needs 1 to {MAX_IK_CHAIN} bones ending at the tip, all inside the rig")]
    BadChain,
    #[error("rig version {0} is newer than this build reads")]
    NewerVersion(u32),
}

pub(crate) fn finite(value: f64, what: &'static str) -> Result<f64, RigError> {
    if value.is_finite() && value.abs() <= MAX_COORD {
        Ok(value)
    } else {
        Err(RigError::NotFinite(what))
    }
}

/// How a rig's symbol keeps time: a Movie Clip on its own clock while the pose drives it, a
/// Graphic locked to the parent playhead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Clock {
    #[default]
    Locked,
    Free,
}

impl From<SymbolKind> for Clock {
    fn from(kind: SymbolKind) -> Self {
        match kind {
            SymbolKind::MovieClip => Self::Free,
            _ => Self::Locked,
        }
    }
}

/// How a pose eases toward the next key. Mapped to `flare-tween`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Interp {
    Constant,
    #[default]
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
}

/// A bone's allowed pose rotation (radians, relative to its rest angle).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AngleLimit {
    pub min: f64,
    pub max: f64,
}

impl AngleLimit {
    pub fn new(min: f64, max: f64) -> Result<Self, RigError> {
        let tau = std::f64::consts::TAU;
        if min.is_finite() && max.is_finite() && min <= max && min >= -tau && max <= tau {
            Ok(Self { min, max })
        } else {
            Err(RigError::BadLimit)
        }
    }

    pub fn clamp(&self, angle: f64) -> f64 {
        angle.clamp(self.min, self.max)
    }
}

/// A joint. `parent` is an earlier bone, so a rig is acyclic by construction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bone {
    pub name: String,
    pub parent: Option<usize>,
    pub length: f64,
    /// Angle of the bone in its parent's frame when no pose rotates it.
    pub rest_angle: f64,
    #[serde(default)]
    pub limit: Option<AngleLimit>,
    /// The child instance of the rig's symbol this bone moves (a bone moves one instance).
    #[serde(default)]
    pub target: Option<InstanceId>,
}

/// Drive the chain of `chain` bones ending at `tip` so the tip reaches `target` (rig space).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct IkGoal {
    pub tip: usize,
    pub chain: usize,
    pub target: [f64; 2],
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Pose {
    /// Per-bone rotation added to the rest angle; absent means 0.
    #[serde(default)]
    pub rotations: BTreeMap<usize, f64>,
    #[serde(default)]
    pub ik: Vec<IkGoal>,
    /// Root translation of the whole skeleton.
    #[serde(default)]
    pub root: [f64; 2],
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct PoseKey {
    pub pose: Pose,
    #[serde(default)]
    pub interp: Interp,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rig {
    /// Files written before a field existed load with its default; newer files are refused.
    #[serde(default = "one")]
    pub version: u32,
    pub symbol: SymbolId,
    #[serde(default)]
    pub clock: Clock,
    #[serde(default)]
    pub bones: Vec<Bone>,
    /// Keyed on the parent timeline's frame.
    #[serde(default)]
    pub keys: BTreeMap<u32, PoseKey>,
}

fn one() -> u32 {
    1
}

impl Rig {
    /// A rig with no bones for `symbol`, on the clock its kind gives it.
    pub fn new(symbol: SymbolId, kind: SymbolKind) -> Self {
        Self {
            version: RIG_VERSION,
            symbol,
            clock: kind.into(),
            bones: Vec::new(),
            keys: BTreeMap::new(),
        }
    }

    /// Checks a rig read from a file: counts, parents, limits and every number.
    pub fn validate(&self) -> Result<(), RigError> {
        if self.version > RIG_VERSION {
            return Err(RigError::NewerVersion(self.version));
        }
        if self.bones.len() > MAX_BONES {
            return Err(RigError::TooManyBones);
        }
        if self.keys.len() > MAX_POSE_KEYS {
            return Err(RigError::TooManyKeys);
        }
        for (i, bone) in self.bones.iter().enumerate() {
            crate::edit::check_bone(bone, i)?;
        }
        for key in self.keys.values() {
            crate::edit::check_pose(&key.pose, self.bones.len())?;
        }
        Ok(())
    }
}
