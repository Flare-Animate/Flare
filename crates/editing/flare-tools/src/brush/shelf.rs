//! The custom brushes: nibs a person has named and kept, as in Animate's Brush list. New makes one
//! from the nib in hand, changing the nib while one is chosen edits it, and Delete removes it.

use serde::{Deserialize, Serialize};

use super::{Nib, NibShape};

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
    pub fn from_nib(name: String, nib: Nib) -> Self {
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
    pub fn sanitized(&self) -> Self {
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

/// There are already [`MAX_BRUSHES`] custom brushes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShelfFull;

/// The nib in hand and the custom brushes kept, one of which may be the one in hand.
#[derive(Clone, Debug, PartialEq)]
pub struct BrushShelf {
    nib: Nib,
    presets: Vec<BrushPreset>,
    selected: Option<usize>,
}

impl Default for BrushShelf {
    fn default() -> Self {
        Self {
            nib: Nib::ROUND,
            presets: Vec::new(),
            selected: None,
        }
    }
}

impl BrushShelf {
    /// A shelf with `nib` in hand and the `presets` kept (cleaned up, and no more than the limit).
    /// A chosen brush wins over the loose nib, as the nib is then the brush's own.
    pub fn restore(nib: Nib, presets: &[BrushPreset], selected: Option<usize>) -> Self {
        let mut shelf = Self {
            nib,
            presets: presets
                .iter()
                .take(MAX_BRUSHES)
                .map(BrushPreset::sanitized)
                .collect(),
            selected: None,
        };
        if selected.is_some() {
            shelf.choose(selected);
        }
        shelf
    }

    /// The nib in hand.
    pub fn nib(&self) -> Nib {
        self.nib
    }

    /// The custom brushes, in the order they were made.
    pub fn presets(&self) -> &[BrushPreset] {
        &self.presets
    }

    /// The custom brush in hand, if the nib is one of them rather than the plain brush.
    pub fn selected(&self) -> Option<usize> {
        self.selected.filter(|i| *i < self.presets.len())
    }

    /// Keeps the nib in hand as a new custom brush and chooses it. Refused past the limit.
    pub fn keep_nib(&mut self) -> Result<(), ShelfFull> {
        if self.presets.len() >= MAX_BRUSHES {
            return Err(ShelfFull);
        }
        let name = (1..)
            .map(|n| format!("Custom brush {n}"))
            .find(|name| self.presets.iter().all(|b| &b.name != name))
            .unwrap_or_else(|| "Custom brush".to_owned());
        self.presets.push(BrushPreset::from_nib(name, self.nib));
        self.selected = Some(self.presets.len() - 1);
        Ok(())
    }

    /// Chooses a custom brush, or the plain round brush for `None` or a place that is not there.
    pub fn choose(&mut self, index: Option<usize>) {
        match index.and_then(|i| self.presets.get(i).map(|b| (i, b.nib()))) {
            Some((i, nib)) => {
                self.selected = Some(i);
                self.nib = nib;
            }
            None => {
                self.selected = None;
                self.nib = Nib::ROUND;
            }
        }
    }

    /// Removes the chosen custom brush; the plain brush is not one and cannot be removed.
    pub fn delete_selected(&mut self) {
        if let Some(i) = self.selected() {
            self.presets.remove(i);
            self.choose(None);
        }
    }

    /// Sets the nib in hand; if a custom brush is chosen, it is edited to match.
    pub fn set_nib(&mut self, nib: Nib) {
        self.nib = nib;
        if let Some(i) = self.selected() {
            let name = std::mem::take(&mut self.presets[i].name);
            self.presets[i] = BrushPreset::from_nib(name, nib);
        }
    }

    /// A shelf like this one with `nib` in hand and no custom brush chosen.
    #[must_use]
    pub fn with_loose_nib(mut self, nib: Nib) -> Self {
        self.selected = None;
        self.nib = nib;
        self
    }
}
