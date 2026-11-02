//! The dockable workspace layout: which panels exist and how they are arranged.
#![forbid(unsafe_code)]
//!
//! Panels have stable identities (`Pane`), never translated titles, so a saved layout keeps
//! working when labels change. A layout read from disk is untrusted: it is size-capped and
//! must contain every panel exactly once, otherwise it is rejected and the default is used.
//! The workspace never depends on a saved layout being present or valid.

use std::collections::HashMap;

use egui_tiles::{Container, Tile, TileId, Tiles, Tree};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Largest layout file accepted.
pub const MAX_LAYOUT_BYTES: usize = 256 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Pane {
    Tools,
    Stage,
    Library,
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
}

impl Pane {
    pub const ALL: [Pane; 11] = [
        Pane::Tools,
        Pane::Stage,
        Pane::Library,
        Pane::Timeline,
        Pane::Properties,
        Pane::Color,
        Pane::Align,
        Pane::Transform,
        Pane::History,
        Pane::Info,
        Pane::Output,
    ];

    /// The icon of the panel in the side rail, for the panels that have a place there.
    pub fn rail_icon(self) -> Option<&'static str> {
        Some(match self {
            Self::Properties => "panel-properties",
            Self::Library => "panel-library",
            Self::Color => "panel-color",
            Self::Align => "panel-align",
            Self::Transform => "panel-transform",
            Self::History => "panel-history",
            Self::Info => "panel-info",
            Self::Tools | Self::Stage | Self::Timeline | Self::Output => return None,
        })
    }

    /// The key of the panel's name in the interface's language.
    pub fn key(self) -> &'static str {
        match self {
            Self::Tools => "pane-tools",
            Self::Stage => "pane-stage",
            Self::Library => "pane-library",
            Self::Timeline => "pane-timeline",
            Self::Properties => "pane-properties",
            Self::Color => "pane-color",
            Self::Align => "pane-align",
            Self::Transform => "pane-transform",
            Self::History => "pane-history",
            Self::Info => "pane-info",
            Self::Output => "pane-output",
        }
    }

    /// The panel's name in English, for messages that are not in a person's language.
    pub fn title(self) -> &'static str {
        match self {
            Self::Tools => "Tools",
            Self::Stage => "Stage",
            Self::Library => "Library",
            Self::Timeline => "Timeline",
            Self::Properties => "Properties",
            Self::Color => "Color",
            Self::Align => "Align",
            Self::Transform => "Transform",
            Self::History => "History",
            Self::Info => "Info",
            Self::Output => "Output",
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

const TREE_ID: &str = "flare_workspace_layout";

/// The tool strip down the whole left edge, from the top bar to the bottom, then the stage and
/// one stack of tabbed panels (Properties, Library, Color, Align, Transform, History) with the
/// timeline and output as tabs underneath them. The strip is narrow, one tool wide.
pub fn default_tree() -> Tree<Pane> {
    let mut tiles = Tiles::default();
    let tools = tiles.insert_pane(Pane::Tools);
    let stage = tiles.insert_pane(Pane::Stage);
    let timeline = tiles.insert_pane(Pane::Timeline);
    let output = tiles.insert_pane(Pane::Output);
    let properties = tiles.insert_pane(Pane::Properties);
    let library = tiles.insert_pane(Pane::Library);
    let color = tiles.insert_pane(Pane::Color);
    let align = tiles.insert_pane(Pane::Align);
    let transform = tiles.insert_pane(Pane::Transform);
    let history = tiles.insert_pane(Pane::History);
    let info = tiles.insert_pane(Pane::Info);
    let side = tiles.insert_tab_tile(vec![
        properties, library, color, align, transform, history, info,
    ]);
    let bottom = tiles.insert_tab_tile(vec![timeline, output]);
    let top = tiles.insert_horizontal_tile(vec![stage, side]);
    let work = tiles.insert_vertical_tile(vec![top, bottom]);
    let root = tiles.insert_horizontal_tile(vec![tools, work]);
    set_share(&mut tiles, root, tools, TOOLS_SHARE);
    set_share(&mut tiles, root, work, 1.0 - TOOLS_SHARE);
    set_share(&mut tiles, top, stage, 0.715);
    set_share(&mut tiles, top, side, 0.285);
    set_share(&mut tiles, work, top, 0.68);
    set_share(&mut tiles, work, bottom, 0.32);
    Tree::new(egui::Id::unique(TREE_ID), root, tiles)
}

/// The tool strip's share of the window's width: 38 px of 1280, one tool wide.
const TOOLS_SHARE: f32 = 0.03;

fn set_share(tiles: &mut Tiles<Pane>, container: TileId, child: TileId, share: f32) {
    if let Some(Tile::Container(Container::Linear(linear))) = tiles.get_mut(container) {
        linear.shares.set_share(child, share);
    }
}

/// Every panel appears exactly once and the tree has a root.
pub fn check(tree: &Tree<Pane>) -> Result<(), LayoutError> {
    if tree.root().is_none() {
        return Err(LayoutError::Invalid("it has no root".into()));
    }
    let mut counts: HashMap<Pane, usize> = HashMap::new();
    for tile in tree.tiles.tiles() {
        if let Tile::Pane(pane) = tile {
            *counts.entry(*pane).or_default() += 1;
        }
    }
    for pane in Pane::ALL {
        match counts.get(&pane).copied().unwrap_or(0) {
            1 => {}
            0 => {
                return Err(LayoutError::Invalid(format!(
                    "the {} panel is missing",
                    pane.title()
                )));
            }
            _ => {
                return Err(LayoutError::Invalid(format!(
                    "the {} panel appears more than once",
                    pane.title()
                )));
            }
        }
    }
    Ok(())
}

pub fn to_json(tree: &Tree<Pane>) -> Result<String, serde_json::Error> {
    serde_json::to_string(tree)
}

/// Layouts saved before the Info panel existed have none; it joins the stack that holds History
/// (or the one that holds any panel) so such a layout still opens.
fn add_panels_older_layouts_lack(tree: &mut Tree<Pane>) {
    let find = |tree: &Tree<Pane>, wanted: Pane| {
        tree.tiles.iter().find_map(|(id, tile)| match tile {
            Tile::Pane(pane) if *pane == wanted => Some(*id),
            _ => None,
        })
    };
    if find(tree, Pane::Info).is_some() {
        return;
    }
    let Some(history) = find(tree, Pane::History) else {
        return;
    };
    let Some(parent) = tree.tiles.parent_of(history) else {
        return;
    };
    let info = tree.tiles.insert_pane(Pane::Info);
    if let Some(Tile::Container(container)) = tree.tiles.get_mut(parent) {
        container.add_child(info);
    }
}

/// The earlier default had the tool strip in the same row as the stage, above the timeline. A
/// saved layout like that opens as the new default, which is what a reset would give.
fn tool_strip_is_beside_the_stage(tree: &Tree<Pane>) -> bool {
    let find = |wanted: Pane| {
        tree.tiles.iter().find_map(|(id, tile)| match tile {
            Tile::Pane(pane) if *pane == wanted => Some(*id),
            _ => None,
        })
    };
    let (Some(tools), Some(stage)) = (find(Pane::Tools), find(Pane::Stage)) else {
        return false;
    };
    let (Some(a), Some(b)) = (tree.tiles.parent_of(tools), tree.tiles.parent_of(stage)) else {
        return false;
    };
    // Side by side in one row; a tab group holding both is somebody's own arrangement.
    let in_a_row = matches!(
        tree.tiles.get(a),
        Some(Tile::Container(Container::Linear(linear)))
            if linear.dir == egui_tiles::LinearDir::Horizontal
    );
    a == b && in_a_row
}

/// A named layout profile: a starting arrangement of the panels, the way the
/// Animate/Toonz profiles give a different workspace for each task.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LayoutProfile {
    Animate,
    Cleanup,
    Compose,
    Draw,
    Library,
    Rigging,
    Sound,
    Timing,
    Default,
    OpenToonz,
    Flippaclip,
    Moho,
}

impl LayoutProfile {
    pub const ALL: [LayoutProfile; 12] = [
        Self::Animate,
        Self::Cleanup,
        Self::Compose,
        Self::Draw,
        Self::Library,
        Self::Rigging,
        Self::Sound,
        Self::Timing,
        Self::Default,
        Self::OpenToonz,
        Self::Flippaclip,
        Self::Moho,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Animate => "Animate",
            Self::Cleanup => "Cleanup",
            Self::Compose => "Compose",
            Self::Draw => "Draw",
            Self::Library => "Library",
            Self::Rigging => "Rigging",
            Self::Sound => "Sound",
            Self::Timing => "Timing",
            Self::Default => "Default",
            Self::OpenToonz => "OpenToonz",
            Self::Flippaclip => "Flippaclip",
            Self::Moho => "Moho",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|p| p.name() == name)
    }
}

/// The panel arrangement for a profile. Profiles that are not yet specialised
/// fall back to the default tree; the specialist layouts land as they are ported.
pub fn profile_tree(profile: LayoutProfile) -> Tree<Pane> {
    match profile {
        LayoutProfile::Animate | LayoutProfile::Default | LayoutProfile::OpenToonz | LayoutProfile::Flippaclip | LayoutProfile::Moho => default_tree(),
        // Stubbed: specialised arrangements to be ported from the Rust-Rewrite
        // profiles (see docs/LAYOUT_PROFILES.md). For now they use the default.
        LayoutProfile::Cleanup
        | LayoutProfile::Compose
        | LayoutProfile::Draw
        | LayoutProfile::Library
        | LayoutProfile::Rigging
        | LayoutProfile::Sound
        | LayoutProfile::Timing => default_tree(),
    }
}

/// Parses and validates a saved layout.
pub fn from_json(json: &str) -> Result<Tree<Pane>, LayoutError> {
    if json.len() > MAX_LAYOUT_BYTES {
        return Err(LayoutError::TooLarge);
    }
    let mut tree: Tree<Pane> = serde_json::from_str(json)?;
    add_panels_older_layouts_lack(&mut tree);
    check(&tree)?;
    if tool_strip_is_beside_the_stage(&tree) {
        // The earlier default: it opens as the new one, with nothing to report.
        return Ok(default_tree());
    }
    Ok(tree)
}

#[cfg(test)]
mod tests;
