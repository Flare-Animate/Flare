//! The custom brushes: nibs a person has named and kept, as in Animate's Brush list. New makes one
//! from the nib in hand, changing the nib while one is chosen edits it, and Delete removes it.

use serde::{Deserialize, Serialize};

use super::{Nib, NibShape};

/// Most custom brushes kept.
pub const MAX_BRUSHES: usize = 32;
/// Longest brush name kept, in characters.
const MAX_NAME_CHARS: usize = 40;
/// Saved brush shelves are deliberately small and bounded.
pub const MAX_BRUSH_PROFILE_BYTES: usize = 64 * 1024;

/// A named nib, as kept in the preferences.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BrushPreset {
    pub name: String,
    pub square: bool,
    pub angle: f32,
    pub flatness: f32,
    pub pressure_min: f32,
    pub pressure_gamma: f32,
}

impl Default for BrushPreset {
    fn default() -> Self {
        Self {
            name: "Custom brush".to_owned(),
            square: false,
            angle: 0.0,
            flatness: 100.0,
            pressure_min: 0.15,
            pressure_gamma: 1.0,
        }
    }
}

impl BrushPreset {
    pub fn from_nib(name: String, nib: Nib) -> Self {
        Self::from_settings(name, nib, 0.15, 1.0)
    }

    fn from_settings(name: String, nib: Nib, pressure_min: f32, pressure_gamma: f32) -> Self {
        Self {
            name,
            square: nib.shape() == NibShape::Square,
            angle: nib.angle_degrees() as f32,
            flatness: nib.flatness_percent() as f32,
            pressure_min,
            pressure_gamma,
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

    pub fn pressure_min(&self) -> f32 {
        finite_range(self.pressure_min, 0.15, 0.0, 1.0)
    }

    pub fn pressure_gamma(&self) -> f32 {
        finite_range(self.pressure_gamma, 1.0, 0.1, 4.0)
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
        Self::from_settings(name, self.nib(), self.pressure_min(), self.pressure_gamma())
    }
}

/// Ready-made pressure and nib profiles. These are Flare presets shaped around familiar drawing
/// workflows, not imported application settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BuiltinBrushProfile {
    Plain,
    Pencil,
    Ink,
    GreasePencil,
    Calligraphy,
    Vector,
    Animate,
}

impl BuiltinBrushProfile {
    pub const ALL: [Self; 7] = [
        Self::Plain,
        Self::Pencil,
        Self::Ink,
        Self::GreasePencil,
        Self::Calligraphy,
        Self::Vector,
        Self::Animate,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Plain => "Plain",
            Self::Pencil => "Pencil",
            Self::Ink => "Krita-style Ink",
            Self::GreasePencil => "Blender Grease Pencil",
            Self::Calligraphy => "Calligraphy",
            Self::Vector => "OpenToonz-style Vector",
            Self::Animate => "Adobe Animate-style",
        }
    }

    pub fn preset(self) -> BrushPreset {
        let (square, angle, flatness, pressure_min, pressure_gamma) = match self {
            Self::Plain => (false, 0.0, 100.0, 0.15, 1.0),
            Self::Pencil => (false, 0.0, 100.0, 0.04, 1.8),
            Self::Ink => (false, 0.0, 100.0, 0.02, 1.35),
            Self::GreasePencil => (false, 0.0, 100.0, 0.08, 1.15),
            Self::Calligraphy => (true, 20.0, 24.0, 0.08, 1.0),
            Self::Vector => (false, 0.0, 100.0, 0.15, 0.9),
            Self::Animate => (false, 0.0, 100.0, 0.1, 1.0),
        };
        BrushPreset {
            name: self.name().to_owned(),
            square,
            angle,
            flatness,
            pressure_min,
            pressure_gamma,
        }
    }
}

/// A portable Flare brush-profile collection. This is intentionally a documented JSON format;
/// native application bundles have unrelated binary/resource semantics.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BrushProfileFile {
    pub version: u8,
    pub profiles: Vec<BrushPreset>,
}

impl BrushProfileFile {
    pub fn encode(profiles: &[BrushPreset]) -> Result<String, String> {
        if profiles.len() > MAX_BRUSHES {
            return Err(format!(
                "a profile file may contain at most {MAX_BRUSHES} brushes"
            ));
        }
        let file = Self {
            version: 1,
            profiles: profiles.iter().map(BrushPreset::sanitized).collect(),
        };
        let json = serde_json::to_string_pretty(&file).map_err(|error| error.to_string())?;
        if json.len() > MAX_BRUSH_PROFILE_BYTES {
            return Err("the brush profile file is too large".to_owned());
        }
        Ok(json)
    }

    pub fn decode(json: &str) -> Result<Vec<BrushPreset>, String> {
        if json.len() > MAX_BRUSH_PROFILE_BYTES {
            return Err("the brush profile file is too large".to_owned());
        }
        let file: Self = serde_json::from_str(json).map_err(|error| error.to_string())?;
        if file.version != 1 {
            return Err(format!(
                "unsupported brush profile version {}",
                file.version
            ));
        }
        if file.profiles.len() > MAX_BRUSHES {
            return Err(format!(
                "a profile file may contain at most {MAX_BRUSHES} brushes"
            ));
        }
        Ok(file.profiles.iter().map(BrushPreset::sanitized).collect())
    }
}

fn finite_range(value: f32, fallback: f32, min: f32, max: f32) -> f32 {
    if value.is_finite() {
        value.clamp(min, max)
    } else {
        fallback
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
    builtin: Option<BuiltinBrushProfile>,
    pressure_min: f32,
    pressure_gamma: f32,
}

impl Default for BrushShelf {
    fn default() -> Self {
        Self {
            nib: Nib::ROUND,
            presets: Vec::new(),
            selected: None,
            builtin: Some(BuiltinBrushProfile::Plain),
            pressure_min: 0.15,
            pressure_gamma: 1.0,
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
            builtin: Some(BuiltinBrushProfile::Plain),
            pressure_min: 0.15,
            pressure_gamma: 1.0,
        };
        if selected.is_some() {
            shelf.choose(selected);
        }
        shelf
    }

    pub fn restore_profile(
        preset: &BrushPreset,
        presets: &[BrushPreset],
        selected: Option<usize>,
    ) -> Self {
        let current = preset.sanitized();
        let mut shelf = Self {
            nib: current.nib(),
            presets: presets
                .iter()
                .take(MAX_BRUSHES)
                .map(BrushPreset::sanitized)
                .collect(),
            selected: None,
            builtin: None,
            pressure_min: current.pressure_min(),
            pressure_gamma: current.pressure_gamma(),
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

    pub fn builtin(&self) -> Option<BuiltinBrushProfile> {
        self.builtin
    }

    pub fn pressure_min(&self) -> f32 {
        self.pressure_min
    }

    pub fn pressure_gamma(&self) -> f32 {
        self.pressure_gamma
    }

    pub fn pressure_curve(&self) -> flare_input::PressureCurve {
        flare_input::PressureCurve::new(self.pressure_min, self.pressure_gamma)
    }

    pub fn choose_builtin(&mut self, profile: BuiltinBrushProfile) {
        let preset = profile.preset();
        self.selected = None;
        self.builtin = Some(profile);
        self.nib = preset.nib();
        self.pressure_min = preset.pressure_min();
        self.pressure_gamma = preset.pressure_gamma();
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
        self.presets.push(BrushPreset::from_settings(
            name,
            self.nib,
            self.pressure_min,
            self.pressure_gamma,
        ));
        self.selected = Some(self.presets.len() - 1);
        self.builtin = None;
        Ok(())
    }

    /// Chooses a custom brush, or the plain round brush for `None` or a place that is not there.
    pub fn choose(&mut self, index: Option<usize>) {
        match index.and_then(|i| self.presets.get(i).map(|b| (i, b.nib()))) {
            Some((i, nib)) => {
                self.selected = Some(i);
                self.builtin = None;
                self.nib = nib;
                self.pressure_min = self.presets[i].pressure_min();
                self.pressure_gamma = self.presets[i].pressure_gamma();
            }
            None => {
                self.selected = None;
                self.choose_builtin(BuiltinBrushProfile::Plain);
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
        self.builtin = None;
        if let Some(i) = self.selected() {
            let name = std::mem::take(&mut self.presets[i].name);
            self.presets[i] =
                BrushPreset::from_settings(name, nib, self.pressure_min, self.pressure_gamma);
        }
    }

    pub fn set_pressure_curve(&mut self, min: f32, gamma: f32) {
        let min = finite_range(min, 0.15, 0.0, 1.0);
        let gamma = finite_range(gamma, 1.0, 0.1, 4.0);
        if self.pressure_min == min && self.pressure_gamma == gamma {
            return;
        }
        self.pressure_min = min;
        self.pressure_gamma = gamma;
        self.builtin = None;
        if let Some(i) = self.selected() {
            self.presets[i].pressure_min = self.pressure_min;
            self.presets[i].pressure_gamma = self.pressure_gamma;
        }
    }

    /// A shelf like this one with `nib` in hand and no custom brush chosen.
    #[must_use]
    pub fn with_loose_nib(mut self, nib: Nib) -> Self {
        self.selected = None;
        self.nib = nib;
        self.builtin = None;
        self
    }
}
