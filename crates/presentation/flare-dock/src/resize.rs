//! Panels keep their size when the window changes size, as Animate's do: the stage takes up the
//! difference. Without this every split shares the window in proportion, so a smaller window
//! squeezes the tool strip narrower than a tool and the panels on the right narrower than their
//! controls, while a large stage keeps space it could give up.

use egui::Rect;
use egui_tiles::{Container, LinearDir, Tile, TileId, Tiles, Tree};

use crate::Pane;

/// The least the stage's side of a split keeps before the panels beside it give way too.
pub const MIN_STAGE_SIDE: f32 = 240.0;

/// The tool strip is never narrower than one tool and its margins.
pub const MIN_TOOL_STRIP: f32 = 36.0;

/// Called just before `tree` is drawn in `area`, with `gap` between tiles (the dock's
/// `gap_width`). Every split whose size changed since the last draw keeps the size, in points,
/// of each side that does not hold the stage; the side holding the stage gets the rest, down to
/// [`MIN_STAGE_SIDE`]. A split without the stage shares its size in proportion, as before. A
/// tree not drawn yet has no sizes to keep and is left alone.
pub fn keep_panel_sizes(tree: &mut Tree<Pane>, area: Rect, gap: f32) {
    if let Some(root) = tree.root() {
        fit(&mut tree.tiles, root, area, gap);
    }
}

fn fit(tiles: &mut Tiles<Pane>, tile: TileId, area: Rect, gap: f32) {
    let Some(old) = tiles.rect(tile) else {
        return;
    };
    let Some(Tile::Container(container)) = tiles.get(tile) else {
        return;
    };
    match container {
        Container::Linear(linear) => {
            let horizontal = linear.dir == LinearDir::Horizontal;
            let children: Vec<TileId> = linear
                .children
                .iter()
                .copied()
                .filter(|c| tiles.is_visible(*c))
                .collect();
            let Some(before) = children
                .iter()
                .map(|c| tiles.rect(*c))
                .collect::<Option<Vec<Rect>>>()
            else {
                return;
            };
            let along = |r: Rect| if horizontal { r.width() } else { r.height() };
            let gaps = gap * children.len().saturating_sub(1) as f32;
            let room = (along(area) - gaps).max(0.0);
            let old_lengths: Vec<f32> = before.iter().map(|r| along(*r)).collect();
            let least: Vec<f32> = children
                .iter()
                .map(|c| least_width(tiles, *c, horizontal))
                .collect();
            let stage = children.iter().position(|c| holds(tiles, *c, Pane::Stage));
            let squeezed = old_lengths.iter().zip(&least).any(|(l, m)| *l + 0.5 < *m);
            let lengths = if (along(area) - along(old)).abs() > 0.5 || squeezed {
                let lengths = lengths_for(&old_lengths, &least, stage, room);
                if let Some(Tile::Container(Container::Linear(linear))) = tiles.get_mut(tile) {
                    for (child, length) in children.iter().zip(&lengths) {
                        linear.shares.set_share(*child, length.max(1.0));
                    }
                }
                lengths
            } else {
                old_lengths
            };
            let mut start = if horizontal { area.left() } else { area.top() };
            for (child, length) in children.iter().zip(lengths) {
                let rect = if horizontal {
                    Rect::from_x_y_ranges(start..=start + length, area.y_range())
                } else {
                    Rect::from_x_y_ranges(area.x_range(), start..=start + length)
                };
                fit(tiles, *child, rect, gap);
                start += length + gap;
            }
        }
        // A tab's panel fills the stack less its row of tabs; that inset stays the same.
        Container::Tabs(tabs) => {
            let children: Vec<TileId> = tabs.children.clone();
            for child in children {
                if let Some(inner) = tiles.rect(child) {
                    let rect = Rect::from_min_max(
                        area.min + (inner.min - old.min),
                        area.max + (inner.max - old.max),
                    );
                    fit(tiles, child, rect, gap);
                }
            }
        }
        Container::Grid(_) => {}
    }
}

/// The new lengths of a split's children, `old` before, filling `room`. Without the stage
/// (`stage` is its index) they share `room` in proportion. With it, the others keep their
/// lengths, at least `least`, and the stage takes the rest; when that would leave the stage less
/// than [`MIN_STAGE_SIDE`], the others give way in proportion.
pub(crate) fn lengths_for(old: &[f32], least: &[f32], stage: Option<usize>, room: f32) -> Vec<f32> {
    let total: f32 = old.iter().sum();
    let Some(stage) = stage else {
        if total <= 0.0 {
            return vec![room / old.len().max(1) as f32; old.len()];
        }
        return old.iter().map(|l| room * l / total).collect();
    };
    let mut lengths: Vec<f32> = old.iter().zip(least).map(|(l, m)| l.max(*m)).collect();
    lengths[stage] = 0.0;
    let others: f32 = lengths.iter().sum();
    let floor = MIN_STAGE_SIDE.min(room);
    if room - others < floor && others > 0.0 {
        let scale = (room - floor).max(0.0) / others;
        for length in &mut lengths {
            *length *= scale;
        }
    }
    let others: f32 = lengths.iter().sum();
    lengths[stage] = (room - others).max(0.0);
    lengths
}

/// Whether `tile` is `wanted` or holds it.
fn holds(tiles: &Tiles<Pane>, tile: TileId, wanted: Pane) -> bool {
    match tiles.get(tile) {
        Some(Tile::Pane(pane)) => *pane == wanted,
        Some(Tile::Container(container)) => container
            .children()
            .any(|child| holds(tiles, *child, wanted)),
        None => false,
    }
}

/// The least width a split's child takes across: one tool for the tool strip, nothing else.
fn least_width(tiles: &Tiles<Pane>, tile: TileId, horizontal: bool) -> f32 {
    match tiles.get(tile) {
        Some(Tile::Pane(Pane::Tools)) if horizontal => MIN_TOOL_STRIP,
        _ => 0.0,
    }
}

#[cfg(test)]
#[path = "resize_tests.rs"]
mod tests;
