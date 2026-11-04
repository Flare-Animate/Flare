//! The data a profile is made of: panels, the dock trees of rooms, toolbars and defaults.
//! Plain serde types; `load` checks them before anything uses them.

use serde::{Deserialize, Serialize};

/// Every panel the interface has or plans. A profile must place all of them in its rooms.
/// The ones that are not built yet are shown dimmed where they appear (docs/UI.md).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PanelId {
    Tools,
    ToolOptions,
    Properties,
    Library,
    Color,
    Align,
    Transform,
    History,
    Info,
    Timeline,
    Output,
    Xsheet,
    Schematic,
    FunctionEditor,
    LevelStrip,
    LevelPalette,
    StyleEditor,
    Cleanup,
    CleanupSettings,
    Browser,
    SceneCast,
    FxBrowser,
    FxSettings,
    MotionPath,
    Plastic,
    ScriptConsole,
    Scan,
    Tasks,
}

impl PanelId {
    /// Every panel, once each. A test checks this against the enum's variants.
    pub const ALL: [Self; 28] = [
        Self::Tools,
        Self::ToolOptions,
        Self::Properties,
        Self::Library,
        Self::Color,
        Self::Align,
        Self::Transform,
        Self::History,
        Self::Info,
        Self::Timeline,
        Self::Output,
        Self::Xsheet,
        Self::Schematic,
        Self::FunctionEditor,
        Self::LevelStrip,
        Self::LevelPalette,
        Self::StyleEditor,
        Self::Cleanup,
        Self::CleanupSettings,
        Self::Browser,
        Self::SceneCast,
        Self::FxBrowser,
        Self::FxSettings,
        Self::MotionPath,
        Self::Plastic,
        Self::ScriptConsole,
        Self::Scan,
        Self::Tasks,
    ];

    /// Whether Flare has this panel today. The rest are planned mocks.
    pub fn is_built(self) -> bool {
        matches!(
            self,
            Self::Tools
                | Self::ToolOptions
                | Self::Properties
                | Self::Library
                | Self::Color
                | Self::Align
                | Self::Transform
                | Self::History
                | Self::Info
                | Self::Timeline
                | Self::Output
        )
    }
}

/// Which way a split divides its space.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Axis {
    /// The first part is on the left, the second on the right.
    Horizontal,
    /// The first part is on top, the second below.
    Vertical,
}

/// A dock tree: splits divide space, stacks hold tabbed panels.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DockNode {
    Split {
        axis: Axis,
        /// The share of the space the first part takes, from 0.05 to 0.95.
        first_fraction: f32,
        first: Box<DockNode>,
        second: Box<DockNode>,
    },
    Stack {
        panels: Vec<PanelId>,
        /// The tab that is shown first; an index into `panels`.
        active: usize,
    },
}

/// One arrangement of the interface, with its own dock tree.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Room {
    pub id: String,
    pub title: String,
    pub layout: DockNode,
}

/// A toolbar the profile shows, by id.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Toolbar {
    pub id: String,
    pub visible: bool,
}

/// A whole workspace profile.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    /// The file format version; only 1 is read.
    pub schema: u32,
    pub id: String,
    pub title: String,
    /// The keyboard shortcut set this profile starts with.
    pub shortcut_set: String,
    /// The theme this profile starts with, by its code (`dark`, `light`, `liquidglass`).
    pub theme: String,
    pub toolbars: Vec<Toolbar>,
    pub rooms: Vec<Room>,
    /// The index of the room shown first.
    pub default_room: usize,
}
