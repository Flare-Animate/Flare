//! A whole arrangement: the docked panels and the panels kept as icons in the strip.

use std::collections::HashMap;

use egui_tiles::{Tile, Tree};
use serde::{Deserialize, Serialize};

use crate::arrange::{self, DEFAULT_SIDE, DEFAULT_STRIP};
use crate::migrate;
use crate::{LayoutError, MAX_LAYOUT_BYTES, Pane};

/// Where every panel is: docked in `tree`, or an icon in `strip` (top to bottom) that opens the
/// panel beside the strip. Each panel is in exactly one of the two.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Layout {
    pub tree: Tree<Pane>,
    #[serde(default)]
    pub strip: Vec<Pane>,
}

/// Properties, Library and Assets on the right; the rest of the side panels in the strip.
pub fn default_layout() -> Layout {
    Layout {
        tree: arrange::arrangement(&DEFAULT_SIDE),
        strip: DEFAULT_STRIP.to_vec(),
    }
}

impl Layout {
    /// Whether `pane` is in the strip rather than docked.
    pub fn in_strip(&self, pane: Pane) -> bool {
        self.strip.contains(&pane)
    }

    /// Moves a docked panel to the end of the strip. The tool strip, the stage, the timeline and
    /// the output have no icon and stay docked.
    pub fn collapse(&mut self, pane: Pane) -> Result<(), LayoutError> {
        if !pane.can_collapse() {
            return Err(LayoutError::Invalid(format!(
                "the {} panel cannot be kept in the strip",
                pane.title()
            )));
        }
        if self.in_strip(pane) {
            return Ok(());
        }
        if arrange::remove(&mut self.tree, pane) {
            self.strip.push(pane);
        }
        Ok(())
    }

    /// Moves a panel from the strip into the stack on the right, in front.
    pub fn dock(&mut self, pane: Pane) {
        let Some(at) = self.strip.iter().position(|p| *p == pane) else {
            return;
        };
        self.strip.remove(at);
        arrange::dock_beside_side_stack(&mut self.tree, pane);
    }
}

/// Every panel appears exactly once, docked or in the strip; the strip holds only panels that can
/// be kept there; and the tree has a root.
pub fn check(layout: &Layout) -> Result<(), LayoutError> {
    if layout.tree.root().is_none() {
        return Err(LayoutError::Invalid("it has no root".into()));
    }
    let mut counts: HashMap<Pane, usize> = HashMap::new();
    for tile in layout.tree.tiles.tiles() {
        if let Tile::Pane(pane) = tile {
            *counts.entry(*pane).or_default() += 1;
        }
    }
    for pane in &layout.strip {
        if !pane.can_collapse() {
            return Err(LayoutError::Invalid(format!(
                "the {} panel cannot be kept in the strip",
                pane.title()
            )));
        }
        *counts.entry(*pane).or_default() += 1;
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

pub fn to_json(layout: &Layout) -> Result<String, serde_json::Error> {
    serde_json::to_string(layout)
}

/// Parses and validates a saved layout. A file from before the strip holds the docked tree alone;
/// it opens with an empty strip, and an earlier default opens as the current one.
pub fn from_json(json: &str) -> Result<Layout, LayoutError> {
    if json.len() > MAX_LAYOUT_BYTES {
        return Err(LayoutError::TooLarge);
    }
    let value: serde_json::Value = serde_json::from_str(json)?;
    let is_layout = value.get("tree").is_some();
    let mut layout = if is_layout {
        serde_json::from_value::<Layout>(value)?
    } else {
        Layout {
            tree: serde_json::from_value(value)?,
            strip: Vec::new(),
        }
    };
    if !is_layout {
        migrate::add_panels_older_layouts_lack(&mut layout.tree);
    }
    check(&layout)?;
    if !is_layout
        && (migrate::tool_strip_is_beside_the_stage(&layout.tree)
            || migrate::timeline_runs_under_the_side_panels(&layout.tree))
    {
        // An earlier default: it opens as the new one, with nothing to report.
        return Ok(default_layout());
    }
    Ok(layout)
}
