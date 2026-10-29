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
    InsertKeyframe,
    InsertBlankKeyframe,
    ClearKeyframe,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FrameOp {
    pub operation: FrameOperation,
    /// Frame number, counting from 0.
    pub at: u32,
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
