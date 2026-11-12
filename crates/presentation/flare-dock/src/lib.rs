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
}

impl Pane {
    pub const ALL: [Pane; 15] = [
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
    ];

    /// The icon of the panel in the side rail, for the panels that have a place there.
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
            Self::Tools
            | Self::Stage
            | Self::Timeline
            | Self::Output
            | Self::MotionEditor
            | Self::FindReplace
            | Self::Assets => return None,
        })
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
    let layers = tiles.insert_pane(Pane::Layers);
    let color = tiles.insert_pane(Pane::Color);
    let align = tiles.insert_pane(Pane::Align);
    let transform = tiles.insert_pane(Pane::Transform);
    let history = tiles.insert_pane(Pane::History);
    let info = tiles.insert_pane(Pane::Info);
    let motion_editor = tiles.insert_pane(Pane::MotionEditor);
    let find_replace = tiles.insert_pane(Pane::FindReplace);
    let assets = tiles.insert_pane(Pane::Assets);
    let side = tiles.insert_tab_tile(vec![
        properties,
        library,
        layers,
        color,
        align,
        transform,
        history,
        info,
        motion_editor,
        find_replace,
        assets,
    ]);
    let bottom = tiles.insert_tab_tile(vec![timeline, output]);
    // As in Animate, the panels on the right run the window's whole height and the timeline sits
    // under the stage alone, so a long Properties panel is not cut short by the timeline.
    let middle = tiles.insert_vertical_tile(vec![stage, bottom]);
    let work = tiles.insert_horizontal_tile(vec![middle, side]);
    let root = tiles.insert_horizontal_tile(vec![tools, work]);
    set_share(&mut tiles, root, tools, TOOLS_SHARE);
    set_share(&mut tiles, root, work, 1.0 - TOOLS_SHARE);
    set_share(&mut tiles, work, middle, 0.77);
    set_share(&mut tiles, work, side, 0.23);
    set_share(&mut tiles, middle, stage, 0.68);
    set_share(&mut tiles, middle, bottom, 0.32);
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

/// The default before the right-hand panels ran the window's height: the stage shared a row with
/// them and the timeline ran under both. A saved layout like that opens as the new default.
fn timeline_runs_under_the_side_panels(tree: &Tree<Pane>) -> bool {
    let find = |wanted: Pane| {
        tree.tiles.iter().find_map(|(id, tile)| match tile {
            Tile::Pane(pane) if *pane == wanted => Some(*id),
            _ => None,
        })
    };
    let (Some(stage), Some(timeline), Some(properties)) = (
        find(Pane::Stage),
        find(Pane::Timeline),
        find(Pane::Properties),
    ) else {
        return false;
    };
    let parent = |id| tree.tiles.parent_of(id);
    let is_linear = |id, dir| {
        matches!(
            tree.tiles.get(id),
            Some(Tile::Container(Container::Linear(linear))) if linear.dir == dir
        )
    };
    let (Some(row), Some(timeline_stack), Some(side)) =
        (parent(stage), parent(timeline), parent(properties))
    else {
        return false;
    };
    let (Some(column), Some(side_row)) = (parent(row), parent(side)) else {
        return false;
    };
    is_linear(row, egui_tiles::LinearDir::Horizontal)
        && side_row == row
        && is_linear(column, egui_tiles::LinearDir::Vertical)
        && parent(timeline_stack) == Some(column)
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
    StudioGhibli,
}

impl LayoutProfile {
    pub const ALL: [LayoutProfile; 13] = [
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
        Self::StudioGhibli,
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
            Self::StudioGhibli => "StudioGhibli",
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
        LayoutProfile::Animate
        | LayoutProfile::Default
        | LayoutProfile::OpenToonz
        | LayoutProfile::Flippaclip
        | LayoutProfile::Moho
        | LayoutProfile::StudioGhibli => default_tree(),
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
    if tool_strip_is_beside_the_stage(&tree) || timeline_runs_under_the_side_panels(&tree) {
        // The earlier default: it opens as the new one, with nothing to report.
        return Ok(default_tree());
    }
    Ok(tree)
}

#[cfg(test)]
mod tests;
