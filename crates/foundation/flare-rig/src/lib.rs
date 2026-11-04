//! Skeletal rigs for symbols: a bone hierarchy attached to a symbol's timeline, keyframed poses
//! eased by `flare-tween`, forward kinematics and bounded IK. Pure data and pure functions; the
//! document, the file and the surfaces call it.
//!
//! The solver is Flare's own. The ideas (a bone chain solved in local angles with per-joint
//! limits, cyclic coordinate descent) are the textbook ones; see docs/ARCHITECTURE.md, "Symbols
//! and rigging", for what was read and under which licence.

mod edit;
mod eval;
mod ik;
mod model;

pub use edit::RigCommand;
pub use eval::{BoneTransform, evaluate_rig, rig_symbol_frame};
pub use ik::{Joint, solve_ccd, solve_two_bone};
pub use model::{
    AngleLimit, Bone, Clock, IkGoal, Interp, MAX_BONES, MAX_IK_CHAIN, MAX_IK_ITERATIONS,
    MAX_POSE_KEYS, Pose, PoseKey, RIG_VERSION, Rig, RigError,
};
