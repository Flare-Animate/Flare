//! Parameters of the game tools. Fields are `name=value` pairs, as the command line takes them.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddGameEntity {
    /// The entity's name: 1 to 128 characters, no control characters.
    pub name: String,
    /// The ID of a library symbol the entity draws, if any.
    pub symbol: Option<String>,
    pub expected_revision: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetGameComponent {
    pub entity: u32,
    /// The component's kind: transform, sprite, body, control, camera, ui, audio, particle or
    /// script.
    pub kind: String,
    /// The component's fields by name, with their values as text (for example "frames": "4").
    /// A misspelt name is an error.
    #[serde(default)]
    pub fields: BTreeMap<String, String>,
    pub expected_revision: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GameComponentRef {
    pub entity: u32,
    /// The kind of component to remove.
    pub kind: String,
    pub expected_revision: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GameEntityRef {
    pub entity: u32,
    pub expected_revision: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetGameGravity {
    /// Horizontal gravity, in pixels per second squared.
    pub x: f64,
    /// Vertical gravity, in pixels per second squared (positive is down).
    pub y: f64,
    pub expected_revision: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RunGame {
    /// Frames to step, at most 100000.
    pub frames: u32,
    /// Frames per second. Default 60.
    pub fps: Option<f64>,
    /// Frames separated by `;`, keys in a frame by `,` (for example `right;right,space;`). Frames
    /// past the end of the script take no input.
    pub script: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ExportGame {
    /// File name only (letters, digits, '.', '_' or '-'). Default "game". Writes `<name>.json`,
    /// `<name>.png` and `<name>.sheet.json` into the export directory.
    pub name: Option<String>,
    /// First frame of the sprite sheet (0-based). Default 0.
    pub start: Option<u32>,
    /// Last frame, inclusive. Default: the end of the scene.
    pub end: Option<u32>,
    /// Frames per row. Default: about square.
    pub columns: Option<u32>,
}
