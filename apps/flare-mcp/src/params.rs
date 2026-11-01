//! The arguments of each tool. Their doc comments become the schema descriptions agents read.

use schemars::JsonSchema;
use serde::Deserialize;

/// Arguments of a tool that changes nothing but needs a revision check.
#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct Guarded {
    /// Fail unless the project is at this revision (from get_info).
    pub expected_revision: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FrameArgs {
    /// Frame number, counting from 0.
    pub frame: u32,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ExportPng {
    /// Frame number, counting from 0.
    pub frame: u32,
    /// File name only: letters, digits, '.', '_' or '-'.
    pub name: String,
    /// Output width in pixels; defaults to the stage width.
    pub width: Option<u32>,
    /// Output height in pixels; defaults to the stage height.
    pub height: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DrawRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// Fill, RRGGBB or RRGGBBAA hex. Defaults to black.
    pub color: Option<String>,
    /// Outline color, RRGGBB or RRGGBBAA hex. No outline when omitted.
    pub stroke: Option<String>,
    /// Outline width; defaults to 2.
    pub stroke_width: Option<f64>,
    /// First frame; defaults to 0.
    pub start: Option<u32>,
    /// Frames the rectangle lasts; defaults to 1.
    pub duration: Option<u32>,
    /// Layer id; defaults to the first layer.
    pub layer: Option<String>,
    /// Fail unless the project is at this revision (from get_info).
    pub expected_revision: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddLayer {
    pub name: String,
    /// Fail unless the project is at this revision (from get_info).
    pub expected_revision: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddSymbol {
    pub name: String,
    /// Give the symbol one rectangle: its left edge.
    pub x: Option<f64>,
    pub y: Option<f64>,
    /// Setting a width makes the symbol hold a rectangle.
    pub width: Option<f64>,
    pub height: Option<f64>,
    /// Fill of that rectangle, RRGGBB or RRGGBBAA hex.
    pub color: Option<String>,
    /// Fail unless the project is at this revision (from get_info).
    pub expected_revision: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PlaceInstance {
    /// Symbol id (from get_info or add_symbol).
    pub symbol: String,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub start: Option<u32>,
    pub duration: Option<u32>,
    /// Interpolate to the next exposure of the same symbol.
    pub tween: Option<bool>,
    /// Layer id; defaults to the first layer.
    pub layer: Option<String>,
    /// Fail unless the project is at this revision (from get_info).
    pub expected_revision: Option<u64>,
}

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
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FrameOp {
    pub operation: FrameOperation,
    /// Frame number, counting from 0 (the first, for a range).
    pub at: u32,
    /// The last frame of the range, for `remove_frames` and `reverse_frames`.
    pub to: Option<u32>,
    /// Layer id; defaults to the first layer.
    pub layer: Option<String>,
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
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct LayerOp {
    pub operation: LayerOperation,
    /// Layer id; defaults to the first layer.
    pub layer: Option<String>,
    /// The new name, for `rename`.
    pub name: Option<String>,
    /// Fail unless the project is at this revision (from get_info).
    pub expected_revision: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetStage {
    pub width: Option<u32>,
    pub height: Option<u32>,
    /// RRGGBB or RRGGBBAA hex.
    pub background: Option<String>,
    /// Fail unless the project is at this revision (from get_info).
    pub expected_revision: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RenameScene {
    pub name: String,
    /// Fail unless the project is at this revision (from get_info).
    pub expected_revision: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetCamera {
    /// Frame number, counting from 0, that the camera is pointed from.
    pub frame: u32,
    /// The point of the scene at the middle of the picture. Stays as it is when omitted.
    pub x: Option<f64>,
    pub y: Option<f64>,
    /// 1 shows the stage as it is, 2 twice as large; from 0.01 to 100. Stays when omitted.
    pub zoom: Option<f64>,
    /// Degrees, clockwise. Stays when omitted.
    pub rotation: Option<f64>,
    /// "motion" moves the camera to the next keyframe's; "none" holds it until then. Keeps what
    /// the keyframe has when omitted.
    pub motion: Option<String>,
    /// The motion's ease, from -100 (starts slowly) to 100 (ends slowly).
    pub ease: Option<i8>,
    /// Fail unless the project is at this revision (from get_info).
    pub expected_revision: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ExportName {
    /// File name only: letters, digits, '.', '_' or '-'. The extension is added when missing.
    pub name: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PublishSwf {
    /// File name only; `.swf` is added when missing.
    pub name: String,
    /// Write an uncompressed SWF.
    pub no_compress: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PublishHtml5 {
    /// File name only; `.html` is added when missing.
    pub name: String,
    /// Stop on the last frame instead of looping.
    pub no_loop: Option<bool>,
    /// Leave the stage at the top left instead of centering it.
    pub no_center: Option<bool>,
    /// Scale the stage to fit the browser window.
    pub responsive: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ExportXfl {
    /// File name only. Ending in `.fla` writes a zipped .fla; any other name is a folder of XFL.
    pub name: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ImportProject {
    /// An XFL folder, `.xfl` or `.fla` already inside the export directory (a file name only).
    pub source: String,
    /// File name for the new project; `.flare` is added when missing.
    pub name: String,
}
