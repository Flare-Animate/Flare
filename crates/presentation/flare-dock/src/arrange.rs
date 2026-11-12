//! Building an arrangement of docked panels.

use egui_tiles::{Container, Tile, TileId, Tiles, Tree};

use crate::{Pane, TREE_ID};

/// The panels of the default stack on the right, as Animate's Essentials has Properties, Library
/// and Assets there.
pub const DEFAULT_SIDE: [Pane; 3] = [Pane::Properties, Pane::Library, Pane::Assets];

/// The panels kept as icons in the strip at the right edge by default; each opens beside the
/// strip with the stack still in view.
pub const DEFAULT_STRIP: [Pane; 9] = [
    Pane::Layers,
    Pane::Color,
    Pane::Swatches,
    Pane::Align,
    Pane::Transform,
    Pane::Info,
    Pane::History,
    Pane::MotionEditor,
    Pane::FindReplace,
];

/// Where the docked panels go: stacks in a column left of the stage, the timeline above or below
/// the stage, and stacks in a column on the right running the window's height. The tool strip is
/// always down the left edge.
#[derive(Clone, Copy, Debug)]
pub struct Plan<'a> {
    pub timeline_on_top: bool,
    pub left: &'a [&'a [Pane]],
    pub right: &'a [&'a [Pane]],
}

/// The tool strip down the whole left edge, then the stage with the timeline and output as tabs
/// under it, then one stack holding `side` running the window's height, as Animate's panels on
/// the right do, so a long Properties panel is not cut short by the timeline. Panels in neither
/// place are left out, for the strip to hold.
pub fn arrangement(side: &[Pane]) -> Tree<Pane> {
    arrange(&Plan {
        timeline_on_top: false,
        left: &[],
        right: &[side],
    })
}

/// The tree `plan` describes. Panels it does not name are left out, for the strip to hold; a
/// panel that cannot be kept in the strip (the frame of the workspace) is never taken from its
/// place, and a panel named twice is placed once.
pub fn arrange(plan: &Plan<'_>) -> Tree<Pane> {
    let mut tiles = Tiles::default();
    let mut placed = Vec::new();
    let tools = tiles.insert_pane(Pane::Tools);
    let stage = tiles.insert_pane(Pane::Stage);
    let timeline = tiles.insert_pane(Pane::Timeline);
    let output = tiles.insert_pane(Pane::Output);
    let timeline_stack = tiles.insert_tab_tile(vec![timeline, output]);
    let middle = if plan.timeline_on_top {
        let middle = tiles.insert_vertical_tile(vec![timeline_stack, stage]);
        set_share(&mut tiles, middle, timeline_stack, 0.3);
        set_share(&mut tiles, middle, stage, 0.7);
        middle
    } else {
        let middle = tiles.insert_vertical_tile(vec![stage, timeline_stack]);
        set_share(&mut tiles, middle, stage, 0.68);
        set_share(&mut tiles, middle, timeline_stack, 0.32);
        middle
    };
    let left = column(&mut tiles, plan.left, &mut placed);
    let right = column(&mut tiles, plan.right, &mut placed);
    let mut row = Vec::new();
    let mut shares = Vec::new();
    if let Some(left) = left {
        row.push(left);
        shares.push(0.2);
    }
    row.push(middle);
    shares.push(1.0);
    if let Some(right) = right {
        row.push(right);
        shares.push(0.23);
    }
    let work = if row.len() == 1 {
        middle
    } else {
        let others: f32 = shares.iter().sum::<f32>() - 1.0;
        let work = tiles.insert_horizontal_tile(row.clone());
        for (child, share) in row.into_iter().zip(shares) {
            let share = if child == middle { 1.0 - others } else { share };
            set_share(&mut tiles, work, child, share);
        }
        work
    };
    let root = tiles.insert_horizontal_tile(vec![tools, work]);
    set_share(&mut tiles, root, tools, TOOLS_SHARE);
    set_share(&mut tiles, root, work, 1.0 - TOOLS_SHARE);
    Tree::new(egui::Id::unique(TREE_ID), root, tiles)
}

/// A column of stacks, top to bottom, sharing its height equally; none when it holds nothing.
fn column(tiles: &mut Tiles<Pane>, stacks: &[&[Pane]], placed: &mut Vec<Pane>) -> Option<TileId> {
    let mut column = Vec::new();
    for stack in stacks {
        let mut panes = Vec::new();
        for pane in stack.iter().copied() {
            if pane.can_collapse() && !placed.contains(&pane) {
                placed.push(pane);
                panes.push(tiles.insert_pane(pane));
            }
        }
        if !panes.is_empty() {
            column.push(tiles.insert_tab_tile(panes));
        }
    }
    match column.len() {
        0 => None,
        1 => column.first().copied(),
        _ => Some(tiles.insert_vertical_tile(column)),
    }
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
