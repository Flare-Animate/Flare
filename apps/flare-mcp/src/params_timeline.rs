//! Parameters of the tools that work on frames and layers.

use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FrameOperation {
    InsertFrame,
    RemoveFrame,
    /// Takes the frames from `at` to `to` out; what follows moves left.
    RemoveFrames,
    /// Reverses the keyframes from `at` to `to`, which must lie wholly inside the range.
    ReverseFrames,
    InsertKeyframe,
    InsertBlankKeyframe,
    ClearKeyframe,
    /// Moves the keyframe holding frame `at` so it starts at `to`, on `onto` (default: the same
    /// layer), replacing the frames it lands on and leaving its old frames empty.
    MoveKeyframe,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FrameOp {
    pub operation: FrameOperation,
    /// Frame number, counting from 0 (the first, for a range).
    pub at: u32,
    /// The last frame of the range, for `remove_frames` and `reverse_frames`; for
    /// `move_keyframe`, the frame the keyframe goes to.
    pub to: Option<u32>,
    /// Layer id; defaults to the first layer.
    pub layer: Option<String>,
    /// For `move_keyframe`: the id of the layer it goes to; defaults to its own.
    pub onto: Option<String>,
    /// Fail unless the project is at this revision (from get_info).
    pub expected_revision: Option<u64>,
}

#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LayerOperation {
    Rename,
    Delete,
    Hide,
    Show,
    Lock,
    Unlock,
    MoveUp,
    MoveDown,
    /// Move the layer (a folder with everything in it) to just above `before`, or to the bottom
    /// without it, into `folder` or out of any folder without it.
    Move,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct LayerOp {
    pub operation: LayerOperation,
    /// Layer id; defaults to the first layer.
    pub layer: Option<String>,
    /// The new name, for `rename`.
    pub name: Option<String>,
    /// For `move`: the id of the layer to go just above; without it, the bottom.
    pub before: Option<String>,
    /// For `move`: the id of a folder above the new place to go into; without it, out of any.
    pub folder: Option<String>,
    /// Fail unless the project is at this revision (from get_info).
    pub expected_revision: Option<u64>,
}
