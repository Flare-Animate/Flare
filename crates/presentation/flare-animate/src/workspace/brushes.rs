//! Custom brushes: nibs a person has named and kept, as in Animate's Brush list. New makes one
//! from the nib in hand, changing the nib while one is chosen edits it, and Delete removes it.

use flare_tools::brush::{Nib, NibShape};
use serde::{Deserialize, Serialize};

use super::*;

/// Most custom brushes kept.
pub const MAX_BRUSHES: usize = 32;
/// Longest brush name kept, in characters.
const MAX_NAME_CHARS: usize = 40;

/// A named nib, as kept in the preferences.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BrushPreset {
    pub name: String,
    pub square: bool,
    pub angle: f32,
    pub flatness: f32,
}

impl Default for BrushPreset {
    fn default() -> Self {
        Self {
            name: "Custom brush".to_owned(),
            square: false,
            angle: 0.0,
            flatness: 100.0,
        }
    }
}

impl BrushPreset {
    pub(super) fn from_nib(name: String, nib: Nib) -> Self {
        Self {
            name,
            square: nib.shape() == NibShape::Square,
            angle: nib.angle_degrees() as f32,
            flatness: nib.flatness_percent() as f32,
        }
    }

    pub fn nib(&self) -> Nib {
        let shape = if self.square {
            NibShape::Square
        } else {
            NibShape::Round
        };
        Nib::new(shape, f64::from(self.angle), f64::from(self.flatness))
    }

    /// The preset with a usable name and its numbers in range.
    pub(super) fn sanitized(&self) -> Self {
        let name: String = self
            .name
            .chars()
            .filter(|c| !c.is_control())
            .take(MAX_NAME_CHARS)
            .collect();
        let name = if name.trim().is_empty() {
            "Custom brush".to_owned()
        } else {
            name
        };
        Self::from_nib(name, self.nib())
    }
}

impl Workspace {
    /// The custom brushes, in the order they were made.
    pub fn custom_brushes(&self) -> &[BrushPreset] {
        &self.brushes
    }

    /// The custom brush in hand, if the nib is one of them rather than the plain brush.
    pub fn selected_brush(&self) -> Option<usize> {
        self.brush_selected.filter(|i| *i < self.brushes.len())
    }

    /// Keeps the nib in hand as a new custom brush and chooses it. Refuses past the limit.
    pub fn new_custom_brush(&mut self) {
        if self.brushes.len() >= MAX_BRUSHES {
            return self.report(format!("Only {MAX_BRUSHES} custom brushes can be kept"));
        }
        let name = (1..)
            .map(|n| format!("Custom brush {n}"))
            .find(|name| self.brushes.iter().all(|b| &b.name != name))
            .unwrap_or_else(|| "Custom brush".to_owned());
        self.brushes
            .push(BrushPreset::from_nib(name, self.brush_nib));
        self.brush_selected = Some(self.brushes.len() - 1);
    }

    /// Chooses a custom brush, or the plain round brush for `None` or a place that is not there.
    pub fn choose_brush(&mut self, index: Option<usize>) {
        match index.and_then(|i| self.brushes.get(i).map(|b| (i, b.nib()))) {
            Some((i, nib)) => {
                self.brush_selected = Some(i);
                self.brush_nib = nib;
            }
            None => {
                self.brush_selected = None;
                self.brush_nib = Nib::ROUND;
            }
        }
    }

    /// Removes the chosen custom brush; the plain brush is not one and cannot be removed.
    pub fn delete_custom_brush(&mut self) {
        if let Some(i) = self.selected_brush() {
            self.brushes.remove(i);
            self.choose_brush(None);
        }
    }

    /// Sets the nib in hand; if a custom brush is chosen, it is edited to match.
    pub(super) fn set_brush_nib(&mut self, nib: Nib) {
        self.brush_nib = nib;
        if let Some(i) = self.selected_brush() {
            let name = std::mem::take(&mut self.brushes[i].name);
            self.brushes[i] = BrushPreset::from_nib(name, nib);
        }
    }
}
