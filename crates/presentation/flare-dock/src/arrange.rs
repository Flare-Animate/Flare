//! Building an arrangement of docked panels.

use egui_tiles::{Container, Tile, TileId, Tiles, Tree};

use crate::{Pane, TREE_ID};

/// The panels of the default stack on the right, as Animate's Essentials has Properties, Library
/// and Assets there.
pub const DEFAULT_SIDE: [Pane; 3] = [Pane::Properties, Pane::Library, Pane::Assets];

/// The panels kept as icons in the strip at the right edge by default; each opens beside the
/// strip with the stack still in view.
pub const DEFAULT_STRIP: [Pane; 8] = [
    Pane::Layers,
    Pane::Color,
    Pane::Align,
    Pane::Transform,
    Pane::Info,
    Pane::History,
    Pane::MotionEditor,
    Pane::FindReplace,
];

/// The tool strip down the whole left edge, then the stage with the timeline and output as tabs
/// under it, then one stack holding `side` running the window's height, as Animate's panels on
/// the right do, so a long Properties panel is not cut short by the timeline. Panels in neither
/// place are left out, for the strip to hold.
pub fn arrangement(side: &[Pane]) -> Tree<Pane> {
    let mut tiles = Tiles::default();
    let tools = tiles.insert_pane(Pane::Tools);
    let stage = tiles.insert_pane(Pane::Stage);
    let timeline = tiles.insert_pane(Pane::Timeline);
    let output = tiles.insert_pane(Pane::Output);
    let bottom = tiles.insert_tab_tile(vec![timeline, output]);
    let middle = tiles.insert_vertical_tile(vec![stage, bottom]);
    set_share(&mut tiles, middle, stage, 0.68);
    set_share(&mut tiles, middle, bottom, 0.32);
    let side: Vec<TileId> = side
        .iter()
        .filter(|p| p.can_collapse())
        .map(|p| tiles.insert_pane(*p))
        .collect();
    let work = if side.is_empty() {
        middle
    } else {
        let side = tiles.insert_tab_tile(side);
        let work = tiles.insert_horizontal_tile(vec![middle, side]);
        set_share(&mut tiles, work, middle, 0.77);
        set_share(&mut tiles, work, side, 0.23);
        work
    };
    let root = tiles.insert_horizontal_tile(vec![tools, work]);
    set_share(&mut tiles, root, tools, TOOLS_SHARE);
    set_share(&mut tiles, root, work, 1.0 - TOOLS_SHARE);
    Tree::new(egui::Id::unique(TREE_ID), root, tiles)
}

/// The tool strip's share of the window's width: 38 px of 1280, one tool wide.
const TOOLS_SHARE: f32 = 0.03;

fn set_share(tiles: &mut Tiles<Pane>, container: TileId, child: TileId, share: f32) {
    if let Some(Tile::Container(Container::Linear(linear))) = tiles.get_mut(container) {
        linear.shares.set_share(child, share);
    }
}

/// The tile showing `wanted`, if it is docked.
pub(crate) fn find(tree: &Tree<Pane>, wanted: Pane) -> Option<TileId> {
    tree.tiles.iter().find_map(|(id, tile)| match tile {
        Tile::Pane(pane) if *pane == wanted => Some(*id),
        _ => None,
    })
}

/// Puts `pane` into the tree as a tab of the stack on the right (the one holding the first docked
/// panel that can be collapsed), or, with no such stack, as a new stack at the right edge.
pub(crate) fn dock_beside_side_stack(tree: &mut Tree<Pane>, pane: Pane) {
    let target = Pane::ALL
        .into_iter()
        .filter(|p| *p != pane && p.can_collapse())
        .find_map(|p| find(tree, p));
    let new = tree.tiles.insert_pane(pane);
    if let Some(target) = target {
        match tree.tiles.parent_of(target) {
            Some(parent)
                if matches!(
                    tree.tiles.get(parent),
                    Some(Tile::Container(Container::Tabs(_)))
                ) =>
            {
                if let Some(Tile::Container(container)) = tree.tiles.get_mut(parent) {
                    container.add_child(new);
                    if let Container::Tabs(tabs) = container {
                        tabs.set_active(new);
                    }
                }
                return;
            }
            Some(parent) => {
                // A lone panel shows without a tab row: it becomes a stack of the two in its place.
                let stack = tree.tiles.insert_tab_tile(vec![]);
                if let Some(Tile::Container(container)) = tree.tiles.get_mut(parent) {
                    replace_child(container, target, stack);
                }
                if let Some(Tile::Container(Container::Tabs(tabs))) = tree.tiles.get_mut(stack) {
                    tabs.add_child(target);
                    tabs.add_child(new);
                    tabs.set_active(new);
                }
                return;
            }
            None => {}
        }
    }
    // Nothing docked on the right: a new stack at the right edge of the window.
    let stack = tree.tiles.insert_tab_tile(vec![new]);
    match tree.root() {
        Some(root)
            if matches!(
                tree.tiles.get(root),
                Some(Tile::Container(Container::Linear(l))) if l.dir == egui_tiles::LinearDir::Horizontal
            ) =>
        {
            if let Some(Tile::Container(container)) = tree.tiles.get_mut(root) {
                container.add_child(stack);
            }
        }
        Some(root) => {
            let row = tree.tiles.insert_horizontal_tile(vec![root, stack]);
            tree.root = Some(row);
        }
        None => tree.root = Some(stack),
    }
}

fn replace_child(container: &mut Container, old: TileId, new: TileId) {
    match container {
        Container::Linear(linear) => {
            if let Some(at) = linear.children.iter().position(|c| *c == old) {
                linear.children[at] = new;
                linear.shares.replace_with(old, new);
            }
        }
        Container::Tabs(tabs) => {
            if let Some(at) = tabs.children.iter().position(|c| *c == old) {
                tabs.children[at] = new;
            }
        }
        Container::Grid(_) => {
            container.remove_child(old);
            container.add_child(new);
        }
    }
}

/// Takes `pane`'s tile out of the tree; empty containers are pruned when the tree is next drawn.
pub(crate) fn remove(tree: &mut Tree<Pane>, pane: Pane) -> bool {
    let Some(tile) = find(tree, pane) else {
        return false;
    };
    if let Some(parent) = tree.tiles.parent_of(tile)
        && let Some(Tile::Container(container)) = tree.tiles.get_mut(parent)
    {
        container.remove_child(tile);
    }
    tree.tiles.remove(tile);
    true
}
