//! Saved layouts from earlier versions: what they lack is added, and earlier defaults open as the
//! current one.

use egui_tiles::{Container, Tile, Tree};

use crate::Pane;

/// Layouts saved before the Info panel existed have none; it joins the stack that holds History
/// (or the one that holds any panel) so such a layout still opens.
pub(crate) fn add_panels_older_layouts_lack(tree: &mut Tree<Pane>) {
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
pub(crate) fn tool_strip_is_beside_the_stage(tree: &Tree<Pane>) -> bool {
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
pub(crate) fn timeline_runs_under_the_side_panels(tree: &Tree<Pane>) -> bool {
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
