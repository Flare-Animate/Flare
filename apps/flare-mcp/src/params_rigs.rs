//! Parameters of the tools that edit the rigs a project keeps.

use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ProjectRigSymbol {
    /// The symbol (its id) whose rig this is.
    pub symbol: String,
    /// Fail unless the project is at this revision (from get_info).
    pub expected_revision: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ProjectRigAddBone {
    pub symbol: String,
    pub name: String,
    /// Index of an earlier bone.
    pub parent: Option<usize>,
    pub length: f64,
    pub rest_angle: Option<f64>,
    /// Pose rotation limits in radians.
    pub limit: Option<[f64; 2]>,
    /// The child instance this bone moves.
    pub target: Option<String>,
    pub expected_revision: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ProjectRigSetPose {
    pub symbol: String,
    pub frame: u32,
    /// [bone index, radians] pairs.
    #[serde(default)]
    pub rotations: Vec<(usize, f64)>,
    /// [tip bone, chain length, target x, target y] goals.
    #[serde(default)]
    pub ik: Vec<(usize, usize, f64, f64)>,
    /// constant, linear, ease-in, ease-out or ease-in-out.
    pub interp: Option<String>,
    pub expected_revision: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ProjectRigEval {
    pub symbol: String,
    pub frame: u32,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct StopMotionStart {
    /// Captures a second, 1 to 120; defaults to 12.
    pub fps: Option<u32>,
    /// Timeline frames each new frame is held for, 1 to 60; defaults to 1.
    pub hold: Option<u32>,
    pub expected_revision: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct StopMotionAddFrame {
    /// A PNG file in the export directory, added as the session's next frame.
    pub source: String,
    /// Timeline frames it is held for; the session's hold without it.
    pub hold: Option<u32>,
    pub expected_revision: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct StopMotionFrame {
    /// The frame, counted from 0.
    pub index: usize,
    /// For stop_motion_set_hold: timeline frames to hold it for, 1 to 60.
    pub hold: Option<u32>,
    pub expected_revision: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct StopMotionLay {
    /// Name of the new layer; defaults to "Stop motion".
    pub layer_name: Option<String>,
    pub expected_revision: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct StopMotionRevision {
    pub expected_revision: Option<u64>,
}
