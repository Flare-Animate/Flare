//! The dockable workspace layout: which panels exist and how they are arranged.
#![forbid(unsafe_code)]
//!
//! Panels have stable identities (`Pane`), never translated titles, so a saved layout keeps
//! working when labels change. A layout read from disk is untrusted: it is size-capped and
//! must contain every panel exactly once, otherwise it is rejected and the default is used.
//! The workspace never depends on a saved layout being present or valid.

use serde::{Deserialize, Serialize};
use thiserror::Error;

mod arrange;
mod layout;
mod migrate;
mod resize;
mod workspaces;

pub use arrange::{DEFAULT_SIDE, DEFAULT_STRIP, Plan, arrange, arrangement};
pub use layout::{Layout, check, default_layout, from_json, to_json};
pub use resize::{MIN_STAGE_SIDE, MIN_TOOL_STRIP, keep_panel_sizes};
pub use workspaces::{
    Custom, MAX_CUSTOM, MAX_NAME_CHARS, MAX_WORKSPACES_BYTES, Preset, WorkspaceId, Workspaces,
    workspaces_from_json, workspaces_to_json,
};

/// Largest layout file accepted.
pub const MAX_LAYOUT_BYTES: usize = 256 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Pane {
    Tools,
    Stage,
    Library,
    Layers,
    Timeline,
    Properties,
    Color,
    Align,
    Transform,
    History,
    /// The size and position of the selection, and what is under the pointer.
    Info,
    /// Messages from the last actions: errors and refusals that the status bar shows once.
    Output,
    /// Per-property graphs for a tween: the Motion Editor.
    MotionEditor,
    /// Find and Replace across text, fonts, colors and symbols.
    FindReplace,
    /// Shared and external libraries.
    Assets,
    /// The web-safe colors, greys and the person's own swatches.
    Swatches,
}

impl Pane {
    pub const ALL: [Pane; 16] = [
        Pane::Tools,
        Pane::Stage,
        Pane::Library,
        Pane::Layers,
        Pane::Timeline,
        Pane::Properties,
        Pane::Color,
        Pane::Align,
        Pane::Transform,
        Pane::History,
        Pane::Info,
        Pane::Output,
        Pane::MotionEditor,
        Pane::FindReplace,
        Pane::Assets,
        Pane::Swatches,
    ];

    /// The icon of the panel in the strip at the right edge, for the panels that can be kept
    /// there.
    pub fn rail_icon(self) -> Option<&'static str> {
        Some(match self {
            Self::Properties => "panel-properties",
            Self::Library => "panel-library",
            Self::Layers => "panel-layers",
            Self::Color => "panel-color",
            Self::Align => "panel-align",
            Self::Transform => "panel-transform",
            Self::History => "panel-history",
            Self::Info => "panel-info",
            Self::MotionEditor => "panel-motion-editor",
            Self::FindReplace => "panel-find",
            Self::Assets => "panel-assets",
            Self::Swatches => "panel-swatches",
            Self::Tools | Self::Stage | Self::Timeline | Self::Output => return None,
        })
    }

    /// Whether the panel can be kept as an icon in the strip. The tool strip, the stage, the
    /// timeline and the output are the frame of the workspace and stay docked.
    pub fn can_collapse(self) -> bool {
        self.rail_icon().is_some()
    }

    /// The key of the panel's name in the interface's language.
    pub fn key(self) -> &'static str {
        match self {
            Self::Tools => "pane-tools",
            Self::Stage => "pane-stage",
            Self::Library => "pane-library",
            Self::Layers => "pane-layers",
            Self::Timeline => "pane-timeline",
            Self::Properties => "pane-properties",
            Self::Color => "pane-color",
            Self::Align => "pane-align",
            Self::Transform => "pane-transform",
            Self::History => "pane-history",
            Self::Info => "pane-info",
            Self::Output => "pane-output",
            Self::MotionEditor => "pane-motion-editor",
            Self::FindReplace => "pane-find-replace",
            Self::Assets => "pane-assets",
            Self::Swatches => "pane-swatches",
        }
    }

    /// The panel's name in English, for messages that are not in a person's language.
    pub fn title(self) -> &'static str {
        match self {
            Self::Tools => "Tools",
            Self::Stage => "Stage",
            Self::Library => "Library",
            Self::Layers => "Layers",
            Self::Timeline => "Timeline",
            Self::Properties => "Properties",
            Self::Color => "Color",
            Self::Align => "Align",
            Self::Transform => "Transform",
            Self::History => "History",
            Self::Info => "Info",
            Self::Output => "Output",
            Self::MotionEditor => "Motion Editor",
            Self::FindReplace => "Find and Replace",
            Self::Assets => "Assets",
            Self::Swatches => "Swatches",
        }
    }
}

#[derive(Debug, Error)]
pub enum LayoutError {
    #[error("the layout is larger than {MAX_LAYOUT_BYTES} bytes")]
    TooLarge,
    #[error("the layout could not be read: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("the layout is not usable: {0}")]
    Invalid(String),
}

pub(crate) const TREE_ID: &str = "flare_workspace_layout";

#[cfg(test)]
mod tests;
