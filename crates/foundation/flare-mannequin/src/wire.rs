//! The mannequin as JSON: a pose file, a whole figure (body, proportions, pose and view), and the
//! built-in poses. Everything read is untrusted: the size is bounded first, unknown names are
//! refused, and every number goes through the same checks a slider's does, so a file cannot
//! make a figure the panel could not.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::body::{Body, BodyError, Proportion, Proportions};
use crate::figure::Mannequin;
use crate::math::{Angles, Vec3};
use crate::pose::{Pose, PoseError};
use crate::skeleton::Joint;
use crate::view::{MAX_FOV, MAX_PITCH, View};

/// The format of a figure file; a newer one is refused.
pub const FIGURE_VERSION: u32 = 1;
/// The largest figure or pose file read.
pub const MAX_FIGURE_BYTES: usize = 16 * 1024;

#[derive(Debug, Clone, PartialEq, Error)]
pub enum WireError {
    #[error("a mannequin file is limited to {MAX_FIGURE_BYTES} bytes")]
    TooLarge,
    #[error("the mannequin file cannot be read: {0}")]
    Json(String),
    #[error("the mannequin file is version {0}; this Flare reads version {FIGURE_VERSION}")]
    Version(u32),
    #[error("there is no joint called {0:?}")]
    UnknownJoint(String),
    #[error("there is no body called {0:?}; there are male and female")]
    UnknownBody(String),
    #[error("there is no built-in pose called {0:?}")]
    UnknownPreset(String),
    #[error("there is no proportion called {0:?}")]
    UnknownProportion(String),
    #[error(
        "the view turns by numbers, tilts at most {MAX_PITCH} degrees and widens at most {MAX_FOV}"
    )]
    View,
    #[error("only a hand or a foot reaches for a point, not {0:?}")]
    NotAnEnd(String),
    #[error("{joint} cannot reach {point:?}")]
    OutOfReach {
        joint: &'static str,
        point: [f64; 3],
    },
    #[error("there is no side called {0:?}; there are front, back, left, right, top and bottom")]
    UnknownFacing(String),
    #[error(transparent)]
    Marks(#[from] crate::reference::MarksError),
    #[error(transparent)]
    Pose(#[from] PoseError),
    #[error(transparent)]
    Body(#[from] BodyError),
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PoseWire {
    #[serde(default)]
    joints: BTreeMap<String, [f64; 3]>,
    #[serde(default, skip_serializing_if = "is_zero")]
    lift: [f64; 3],
}

fn is_zero(v: &[f64; 3]) -> bool {
    v.iter().all(|c| *c == 0.0)
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ViewWire {
    yaw: f64,
    pitch: f64,
    fov: f64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FigureWire {
    version: u32,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    proportions: BTreeMap<String, f64>,
    #[serde(default)]
    pose: PoseWire,
    #[serde(default)]
    view: Option<ViewWire>,
}

/// A hundredth of a degree is finer than any hand poses, and keeps the files short.
fn round(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

impl PoseWire {
    pub(crate) fn from_pose(pose: &Pose) -> Self {
        let joints = Joint::ALL
            .into_iter()
            .filter(|j| pose.angles(*j) != Angles::ZERO)
            .map(|j| (j.id().to_owned(), pose.angles(j).as_array().map(round)))
            .collect();
        let lift = pose.lift();
        Self {
            joints,
            lift: [lift.x, lift.y, lift.z].map(|v| (v * 10_000.0).round() / 10_000.0),
        }
    }

    pub(crate) fn into_pose(self) -> Result<Pose, WireError> {
        let mut pose = Pose::default();
        for (name, angles) in self.joints {
            let joint = Joint::from_id(&name).ok_or(WireError::UnknownJoint(name))?;
            pose.set_checked(joint, Angles::from_array(angles))?;
        }
        let [x, y, z] = self.lift;
        pose.set_lift_checked(Vec3::new(x, y, z))?;
        Ok(pose)
    }
}

fn parse<'a, T: Deserialize<'a>>(text: &'a str) -> Result<T, WireError> {
    if text.len() > MAX_FIGURE_BYTES {
        return Err(WireError::TooLarge);
    }
    serde_json::from_str(text).map_err(|e| WireError::Json(e.to_string()))
}

impl Pose {
    /// The pose as JSON: `{"joints": {"left_elbow": [bend, twist, side], ...}, "lift": [x, y, z]}`,
    /// joints at rest left out.
    pub fn to_json(&self) -> String {
        serde_json::to_string(&PoseWire::from_pose(self)).unwrap_or_default()
    }

    /// Reads a pose written by [`to_json`](Self::to_json); a joint left out is at rest.
    pub fn from_json(text: &str) -> Result<Pose, WireError> {
        parse::<PoseWire>(text)?.into_pose()
    }
}

impl Mannequin {
    /// The whole figure as JSON, as the panel keeps it and the command line reads it.
    pub fn to_json(&self) -> String {
        let proportions = Proportion::ALL
            .into_iter()
            .filter(|p| self.proportions.get(*p) != 1.0)
            .map(|p| (p.id().to_owned(), round(self.proportions.get(p))))
            .collect();
        let wire = FigureWire {
            version: FIGURE_VERSION,
            body: Some(self.body.id().to_owned()),
            proportions,
            pose: PoseWire::from_pose(&self.pose),
            view: Some(ViewWire {
                yaw: round(self.view.yaw()),
                pitch: round(self.view.pitch()),
                fov: round(self.view.fov()),
            }),
        };
        serde_json::to_string(&wire).unwrap_or_default()
    }

    /// Reads a figure; what it leaves out is the default (a male figure at rest, the usual view).
    pub fn from_json(text: &str) -> Result<Mannequin, WireError> {
        let wire: FigureWire = parse(text)?;
        if wire.version != FIGURE_VERSION {
            return Err(WireError::Version(wire.version));
        }
        let body = match wire.body {
            Some(name) => Body::from_id(&name).ok_or(WireError::UnknownBody(name))?,
            None => Body::default(),
        };
        let mut proportions = Proportions::default();
        for (name, value) in wire.proportions {
            let p = Proportion::from_id(&name).ok_or(WireError::UnknownProportion(name))?;
            proportions.set(p, value)?;
        }
        let view = match wire.view {
            Some(v) => {
                if !(v.yaw.is_finite()
                    && v.pitch.is_finite()
                    && v.pitch.abs() <= MAX_PITCH
                    && v.fov.is_finite()
                    && (0.0..=MAX_FOV).contains(&v.fov))
                {
                    return Err(WireError::View);
                }
                View::new(v.yaw, v.pitch, v.fov)
            }
            None => View::default(),
        };
        Ok(Mannequin {
            body,
            proportions,
            pose: wire.pose.into_pose()?,
            view,
        })
    }
}

/// A pose that comes with Flare.
#[derive(Debug, Clone, PartialEq)]
pub struct Preset {
    /// Its name in files and commands, and the end of its message key (`mannequin-pose-<id>`).
    pub id: String,
    pub pose: Pose,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PresetWire {
    id: String,
    #[serde(default)]
    joints: BTreeMap<String, [f64; 3]>,
    #[serde(default)]
    lift: [f64; 3],
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PresetsWire {
    version: u32,
    poses: Vec<PresetWire>,
}

/// The built-in poses, read once from `data/poses.json` (a test checks that every one reads).
pub fn presets() -> &'static [Preset] {
    static PRESETS: OnceLock<Vec<Preset>> = OnceLock::new();
    PRESETS.get_or_init(|| read_presets(include_str!("../data/poses.json")).unwrap_or_default())
}

pub(crate) fn read_presets(text: &str) -> Result<Vec<Preset>, WireError> {
    let wire: PresetsWire =
        serde_json::from_str(text).map_err(|e| WireError::Json(e.to_string()))?;
    if wire.version != FIGURE_VERSION {
        return Err(WireError::Version(wire.version));
    }
    wire.poses
        .into_iter()
        .map(|p| {
            let pose = PoseWire {
                joints: p.joints,
                lift: p.lift,
            };
            Ok(Preset {
                id: p.id,
                pose: pose.into_pose()?,
            })
        })
        .collect()
}

/// The built-in pose called `id`.
pub fn preset(id: &str) -> Option<Pose> {
    presets().iter().find(|p| p.id == id).map(|p| p.pose)
}
