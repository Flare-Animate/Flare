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
    /// Messages from the last actions: errors and refusals that the status bar shows once.
    Output,
}

impl Pane {
    pub const ALL: [Pane; 10] = [
        Pane::Tools,
        Pane::Stage,
        Pane::Library,
        Pane::Timeline,
        Pane::Properties,
        Pane::Color,
        Pane::Align,
        Pane::Transform,
        Pane::History,
        Pane::Output,
    ];

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

/// The panel stack, the stage and the properties side by side, the tool strip at the far
/// right, and the timeline and output as tabs underneath.
///
/// Library, Color, Align, Transform and History share the left as tabs.
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
    let left = tiles.insert_tab_tile(vec![library, color, align, transform, history]);
    let bottom = tiles.insert_tab_tile(vec![timeline, output]);
    let top = tiles.insert_horizontal_tile(vec![left, stage, properties, tools]);
    let root = tiles.insert_vertical_tile(vec![top, bottom]);
    set_share(&mut tiles, top, left, 0.20);
    set_share(&mut tiles, top, stage, 0.55);
    set_share(&mut tiles, top, properties, 0.19);
    set_share(&mut tiles, top, tools, 0.06);
    set_share(&mut tiles, root, top, 0.68);
    set_share(&mut tiles, root, bottom, 0.32);
    Tree::new(egui::Id::unique(TREE_ID), root, tiles)
}

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

/// Parses and validates a saved layout.
pub fn from_json(json: &str) -> Result<Tree<Pane>, LayoutError> {
    if json.len() > MAX_LAYOUT_BYTES {
        return Err(LayoutError::TooLarge);
    }
    let tree: Tree<Pane> = serde_json::from_str(json)?;
    check(&tree)?;
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
}
