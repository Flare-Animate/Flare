#![allow(clippy::expect_used)]

use egui_tiles::{Container, Tile, Tiles, Tree};

use super::*;
use crate::arrange::find;

/// A complete layout of the old kind: every panel docked, as a bare tree.
fn all_docked(side: &[Pane]) -> Tree<Pane> {
    let side: Vec<Pane> = side.iter().copied().filter(|p| p.can_collapse()).collect();
    arrange::arrangement(&side)
}

fn value(json: &str) -> serde_json::Value {
    let mut v: serde_json::Value = serde_json::from_str(json).expect("json");
    // Each tree has its own id; the arrangement is what must match.
    if let Some(tree) = v.get_mut("tree").and_then(|t| t.as_object_mut()) {
        tree.remove("id");
    } else if let Some(tree) = v.as_object_mut() {
        tree.remove("id");
    }
    v
}

#[test]
fn the_default_layout_has_every_panel_exactly_once_docked_or_in_the_strip() {
    let layout = default_layout();
    check(&layout).expect("valid");
    let docked = layout
        .tree
        .tiles
        .tiles()
        .filter(|t| matches!(t, Tile::Pane(_)))
        .count();
    assert_eq!(docked + layout.strip.len(), Pane::ALL.len());
    for pane in DEFAULT_SIDE {
        assert!(find(&layout.tree, pane).is_some(), "{pane:?} is docked");
    }
    for pane in DEFAULT_STRIP {
        assert!(layout.in_strip(pane) && find(&layout.tree, pane).is_none());
    }
}

#[test]
fn the_tool_strip_runs_the_full_height_in_its_own_column() {
    let tree = default_layout().tree;
    let id_of = |wanted: Pane| find(&tree, wanted).expect("pane");
    let (tools, timeline) = (id_of(Pane::Tools), id_of(Pane::Timeline));
    let root = tree.root().expect("root");
    assert_eq!(tree.tiles.parent_of(tools), Some(root));
    // The timeline is inside the other child of the root, beside the strip, not under it.
    let mut at = tree.tiles.parent_of(timeline);
    while let Some(id) = at {
        if tree.tiles.parent_of(id) == Some(root) {
            assert_ne!(id, tools);
            break;
        }
        at = tree.tiles.parent_of(id);
    }
    assert!(at.is_some(), "the timeline is under the root");
    let Some(Tile::Container(Container::Linear(root))) = tree.tiles.get(root) else {
        panic!("the root is a row");
    };
    assert!(root.shares[tools] <= 0.04, "narrow: {}", root.shares[tools]);
}

#[test]
fn the_side_panels_run_the_full_height_and_the_timeline_sits_under_the_stage_alone() {
    let tree = default_layout().tree;
    let id_of = |wanted: Pane| find(&tree, wanted).expect("pane");
    let stage = id_of(Pane::Stage);
    let timeline_stack = tree.tiles.parent_of(id_of(Pane::Timeline)).expect("stack");
    let column = tree.tiles.parent_of(stage).expect("column");
    assert!(matches!(
        tree.tiles.get(column),
        Some(Tile::Container(Container::Linear(l))) if l.dir == egui_tiles::LinearDir::Vertical
    ));
    assert_eq!(tree.tiles.parent_of(timeline_stack), Some(column));
    let side = tree.tiles.parent_of(id_of(Pane::Properties)).expect("side");
    assert_eq!(tree.tiles.parent_of(side), tree.tiles.parent_of(column));
    assert_ne!(
        tree.tiles.parent_of(side),
        Some(column),
        "not under the stage"
    );
}

#[test]
fn the_timeline_and_output_are_tabs_of_one_stack_so_the_bottom_has_a_tab_bar() {
    let tree = default_layout().tree;
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
fn layouts_survive_a_json_round_trip() {
    let mut layout = default_layout();
    layout.dock(Pane::Color);
    let json = to_json(&layout).expect("serialize");
    let restored = from_json(&json).expect("restore");
    assert_eq!(restored.strip, layout.strip);
    // Tiles live in a hash map, so compare the parsed structure, not the key order.
    assert_eq!(value(&to_json(&restored).expect("again")), value(&json));
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
    let json = to_json(&default_layout())
        .expect("serialize")
        .replace("\"Library\"", "\"Mixer\"");
    assert!(matches!(from_json(&json), Err(LayoutError::Parse(_))));

    // A panel missing: the timeline renamed to a second stage.
    let json = to_json(&default_layout())
        .expect("serialize")
        .replace("\"Timeline\"", "\"Stage\"");
    let error = from_json(&json).expect_err("duplicate stage, missing timeline");
    assert!(error.to_string().contains("not usable"), "{error}");

    // A panel both docked and in the strip.
    let mut twice = default_layout();
    twice.strip.push(Pane::Properties);
    assert!(check(&twice).is_err());
    let error = from_json(&to_json(&twice).expect("json")).expect_err("twice");
    assert!(error.to_string().contains("more than once"), "{error}");

    // The stage in the strip.
    let mut stage = default_layout();
    arrange::remove(&mut stage.tree, Pane::Stage);
    stage.strip.push(Pane::Stage);
    let error = from_json(&to_json(&stage).expect("json")).expect_err("stage in the strip");
    assert!(error.to_string().contains("cannot be kept"), "{error}");

    let empty = Layout {
        tree: Tree::<Pane>::empty(egui::Id::unique(TREE_ID)),
        strip: Vec::new(),
    };
    assert!(matches!(check(&empty), Err(LayoutError::Invalid(_))));
}

#[test]
fn collapsing_puts_a_docked_panel_in_the_strip_and_docking_brings_it_back_in_front() {
    let mut layout = default_layout();
    layout.collapse(Pane::Library).expect("collapses");
    assert!(layout.in_strip(Pane::Library) && find(&layout.tree, Pane::Library).is_none());
    assert_eq!(layout.strip.last(), Some(&Pane::Library), "at the end");
    check(&layout).expect("still complete");
    layout.collapse(Pane::Library).expect("twice is no change");
    assert_eq!(
        layout.strip.iter().filter(|p| **p == Pane::Library).count(),
        1
    );

    layout.dock(Pane::Color);
    let color = find(&layout.tree, Pane::Color).expect("docked");
    assert!(!layout.in_strip(Pane::Color));
    assert_eq!(
        layout.tree.tiles.parent_of(color),
        layout
            .tree
            .tiles
            .parent_of(find(&layout.tree, Pane::Properties).expect("p")),
        "it joins the stack on the right"
    );
    assert!(layout.tree.active_tiles().contains(&color), "in front");
    check(&layout).expect("still complete");

    for pane in [Pane::Tools, Pane::Stage, Pane::Timeline, Pane::Output] {
        assert!(layout.collapse(pane).is_err(), "{pane:?} stays docked");
        assert!(find(&layout.tree, pane).is_some());
    }
}

#[test]
fn docking_with_nothing_on_the_right_makes_a_new_stack_there() {
    let mut layout = default_layout();
    for pane in DEFAULT_SIDE {
        layout.collapse(pane).expect("collapses");
    }
    layout
        .tree
        .simplify(&egui_tiles::SimplificationOptions::default());
    check(&layout).expect("complete");
    layout.dock(Pane::Info);
    let info = find(&layout.tree, Pane::Info).expect("docked");
    let stack = layout.tree.tiles.parent_of(info).expect("stack");
    assert!(matches!(
        layout.tree.tiles.get(stack),
        Some(Tile::Container(Container::Tabs(_)))
    ));
    assert_eq!(layout.tree.tiles.parent_of(stack), layout.tree.root());
    check(&layout).expect("complete");
}

#[test]
fn a_bare_tree_from_before_the_strip_opens_with_every_panel_docked() {
    // Somebody's own arrangement, saved as the tree alone.
    let mut tiles = Tiles::default();
    let tools = tiles.insert_pane(Pane::Tools);
    let rest: Vec<_> = Pane::ALL
        .into_iter()
        .filter(|p| *p != Pane::Tools)
        .map(|p| tiles.insert_pane(p))
        .collect();
    let tabs = tiles.insert_tab_tile(rest);
    let column = tiles.insert_vertical_tile(vec![tools, tabs]);
    let own = Tree::new(egui::Id::unique(TREE_ID), column, tiles);
    let json = serde_json::to_string(&own).expect("json");
    let opened = from_json(&json).expect("opens");
    assert!(opened.strip.is_empty(), "nothing moves to the strip");
    assert_eq!(
        value(&serde_json::to_string(&opened.tree).expect("json")),
        value(&json)
    );
}

#[test]
fn a_layout_with_the_strip_above_the_timeline_opens_as_the_new_default_without_complaint() {
    // The earliest default: tools, stage and the stack in one row, over the bottom tabs.
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
    let opened = from_json(&serde_json::to_string(&old).expect("json")).expect("opens");
    assert_eq!(
        value(&to_json(&opened).expect("json")),
        value(&to_json(&default_layout()).expect("json"))
    );
}

#[test]
fn a_layout_with_the_timeline_under_the_side_panels_opens_as_the_new_default() {
    // The previous default: the stage and the side stack in one row, over the bottom tabs.
    let mut tiles = Tiles::default();
    let tools = tiles.insert_pane(Pane::Tools);
    let stage = tiles.insert_pane(Pane::Stage);
    let timeline = tiles.insert_pane(Pane::Timeline);
    let output = tiles.insert_pane(Pane::Output);
    let rest: Vec<_> = Pane::ALL
        .into_iter()
        .filter(|p| p.can_collapse())
        .map(|p| tiles.insert_pane(p))
        .collect();
    let side = tiles.insert_tab_tile(rest);
    let bottom = tiles.insert_tab_tile(vec![timeline, output]);
    let top = tiles.insert_horizontal_tile(vec![stage, side]);
    let work = tiles.insert_vertical_tile(vec![top, bottom]);
    let root = tiles.insert_horizontal_tile(vec![tools, work]);
    let old = Tree::new(egui::Id::unique(TREE_ID), root, tiles);
    assert!(migrate::timeline_runs_under_the_side_panels(&old));
    assert!(!migrate::timeline_runs_under_the_side_panels(
        &default_layout().tree
    ));
    let opened = from_json(&serde_json::to_string(&old).expect("json")).expect("opens");
    assert_eq!(
        value(&to_json(&opened).expect("json")),
        value(&to_json(&default_layout()).expect("json"))
    );
}

#[test]
fn a_bare_tree_saved_before_the_info_panel_still_opens_with_it_beside_history() {
    let mut tree = all_docked(&Pane::ALL);
    // What an older file looks like: the same arrangement without the Info tab.
    assert!(arrange::remove(&mut tree, Pane::Info));
    let json = serde_json::to_string(&tree).expect("serialize");
    let opened = from_json(&json).expect("an older layout is migrated, not refused");
    let info = find(&opened.tree, Pane::Info).expect("info");
    let history = find(&opened.tree, Pane::History).expect("history");
    assert_eq!(
        opened.tree.tiles.parent_of(info),
        opened.tree.tiles.parent_of(history)
    );
}

#[test]
fn a_new_layout_with_info_in_the_strip_is_not_given_a_second_info() {
    let layout = default_layout();
    assert!(layout.in_strip(Pane::Info));
    let opened = from_json(&to_json(&layout).expect("json")).expect("opens");
    assert!(find(&opened.tree, Pane::Info).is_none());
}
