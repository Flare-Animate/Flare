#![allow(clippy::expect_used)]

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
