//! The dockable workspace layout: which panels exist and how they are arranged.
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
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn the_default_layout_has_every_panel_exactly_once() {
        let tree = default_tree();
        check(&tree).expect("valid");
        assert_eq!(
            tree.tiles
                .tiles()
                .filter(|t| matches!(t, Tile::Pane(_)))
                .count(),
            Pane::ALL.len()
        );
    }

    #[test]
    fn the_tool_strip_runs_the_full_height_in_its_own_column() {
        let tree = default_tree();
        let id_of = |wanted: Pane| {
            tree.tiles
                .iter()
                .find_map(|(id, t)| matches!(t, Tile::Pane(p) if *p == wanted).then_some(*id))
                .expect("pane")
        };
        let (tools, stage, timeline) = (
            id_of(Pane::Tools),
            id_of(Pane::Stage),
            id_of(Pane::Timeline),
        );
        assert_eq!(
            tree.tiles.parent_of(tools),
            tree.root(),
            "the strip is a child of the root"
        );
        assert_ne!(tree.tiles.parent_of(tools), tree.tiles.parent_of(stage));
        // The timeline is inside the other child of the root, beside the strip, not under it.
        let mut at = tree.tiles.parent_of(timeline);
        let mut inside_work = false;
        while let Some(id) = at {
            if Some(id) == tree.root() {
                break;
            }
            inside_work = true;
            at = tree.tiles.parent_of(id);
        }
        assert!(inside_work);
        // Narrow: a share small enough for one tool at common window widths.
        let Some(Tile::Container(Container::Linear(root))) =
            tree.root().and_then(|r| tree.tiles.get(r))
        else {
            panic!("the root is a row");
        };
        assert!(root.shares[tools] <= 0.04, "narrow: {}", root.shares[tools]);
    }

    #[test]
    fn a_layout_with_the_strip_above_the_timeline_opens_as_the_new_default_without_complaint() {
        // The earlier default: tools, stage and the stack in one row, over the bottom tabs.
        let mut tiles = Tiles::default();
        let tools = tiles.insert_pane(Pane::Tools);
        let stage = tiles.insert_pane(Pane::Stage);
        let rest: Vec<_> = Pane::ALL
            .into_iter()
            .filter(|p| !matches!(p, Pane::Tools | Pane::Stage))
            .map(|p| tiles.insert_pane(p))
            .collect();
        let tabs = tiles.insert_tab_tile(rest);
        let row = tiles.insert_horizontal_tile(vec![tools, stage, tabs]);
        let old = Tree::new(egui::Id::unique(TREE_ID), row, tiles);
        check(&old).expect("it is a complete layout");
        let opened = from_json(&to_json(&old).expect("json")).expect("it opens, as the default");
        let tools = opened
            .tiles
            .iter()
            .find_map(|(id, t)| matches!(t, Tile::Pane(Pane::Tools)).then_some(*id))
            .expect("tools");
        assert_eq!(
            opened.tiles.parent_of(tools),
            opened.root(),
            "the strip has its own column"
        );
        // The new default is left as it is.
        let value = |tree: &Tree<Pane>| {
            serde_json::from_str::<serde_json::Value>(&to_json(tree).expect("json")).expect("json")
        };
        let fresh = default_tree();
        assert_eq!(
            value(&from_json(&to_json(&fresh).expect("json")).expect("opens")),
            value(&fresh)
        );
    }

    #[test]
    fn layouts_survive_a_json_round_trip() {
        let json = to_json(&default_tree()).expect("serialize");
        let restored = from_json(&json).expect("restore");
        // Tiles live in a hash map, so compare the parsed structure, not the key order.
        let value = |text: &str| serde_json::from_str::<serde_json::Value>(text).expect("json");
        assert_eq!(
            value(&to_json(&restored).expect("serialize again")),
            value(&json)
        );
    }

    #[test]
    fn unusable_layouts_are_rejected_with_a_reason() {
        assert!(matches!(
            from_json("{ not json"),
            Err(LayoutError::Parse(_))
        ));
        assert!(matches!(from_json("{}"), Err(LayoutError::Parse(_))));
        let huge = " ".repeat(MAX_LAYOUT_BYTES + 1);
        assert!(matches!(from_json(&huge), Err(LayoutError::TooLarge)));

        // A panel this version does not know about.
        let json = to_json(&default_tree())
            .expect("serialize")
            .replace("\"Library\"", "\"Mixer\"");
        assert!(matches!(from_json(&json), Err(LayoutError::Parse(_))));

        // A panel missing: drop the timeline pane's tile entry by renaming it to a duplicate.
        let json = to_json(&default_tree())
            .expect("serialize")
            .replace("\"Timeline\"", "\"Stage\"");
        let error = from_json(&json).expect_err("duplicate stage, missing timeline");
        assert!(error.to_string().contains("not usable"), "{error}");

        let empty = Tree::<Pane>::empty(egui::Id::unique(TREE_ID));
        assert!(matches!(check(&empty), Err(LayoutError::Invalid(_))));
    }

    #[test]
    fn the_timeline_and_output_are_tabs_of_one_stack_so_the_bottom_has_a_tab_bar() {
        let tree = default_tree();
        let tabs_holding = |pane: Pane| {
            tree.tiles.iter().find_map(|(id, tile)| match tile {
                Tile::Container(Container::Tabs(tabs)) => tabs
                    .children
                    .iter()
                    .any(|c| matches!(tree.tiles.get(*c), Some(Tile::Pane(p)) if *p == pane))
                    .then_some(*id),
                _ => None,
            })
        };
        let timeline = tabs_holding(Pane::Timeline).expect("timeline is in a tab stack");
        assert_eq!(tabs_holding(Pane::Output), Some(timeline));
    }

    #[test]
    fn a_layout_saved_before_the_info_panel_still_opens_with_it_beside_history() {
        let mut tree = default_tree();
        let info = tree
            .tiles
            .iter()
            .find_map(|(id, t)| matches!(t, Tile::Pane(Pane::Info)).then_some(*id))
            .expect("info");
        // What an older file looks like: the same arrangement without the Info tab.
        let parent = tree.tiles.parent_of(info).expect("parent");
        if let Some(Tile::Container(container)) = tree.tiles.get_mut(parent) {
            container.remove_child(info);
        }
        tree.tiles.remove(info);
        assert!(check(&tree).is_err(), "the old layout lacks a panel");
        let json = to_json(&tree).expect("serialize");
        let opened = from_json(&json).expect("an older layout is migrated, not refused");
        check(&opened).expect("every panel is there now");
        let info = opened
            .tiles
            .iter()
            .find_map(|(id, t)| matches!(t, Tile::Pane(Pane::Info)).then_some(*id))
            .expect("info");
        let history = opened
            .tiles
            .iter()
            .find_map(|(id, t)| matches!(t, Tile::Pane(Pane::History)).then_some(*id))
            .expect("history");
        assert_eq!(
            opened.tiles.parent_of(info),
            opened.tiles.parent_of(history)
        );
    }
}
