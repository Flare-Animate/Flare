//! The custom brushes: nibs a person has named and kept, as in Animate's Brush list. New makes one
//! from the nib in hand, changing the nib while one is chosen edits it, and Delete removes it.

use super::preset::{BrushPreset, BuiltinBrushProfile, MAX_BRUSHES, finite_range};
use super::{BrushStroke, Nib, NibShape, Taper};

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
    /// The drawn pressure curve in hand, when the brush asks for one.
    pressure_points: Option<Vec<(f32, f32)>>,
    velocity: f32,
    flow: f32,
    stabilizer: u8,
    taper: Taper,
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
            pressure_points: None,
            velocity: 0.0,
            flow: 1.0,
            stabilizer: 0,
            taper: Taper::NONE,
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
            pressure_points: None,
            velocity: 0.0,
            flow: 1.0,
            stabilizer: 0,
            taper: Taper::NONE,
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
            pressure_points: current.pressure_points(),
            velocity: current.velocity(),
            flow: current.flow(),
            stabilizer: current.stabilizer(),
            taper: current.taper(),
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

    /// The drawn pressure curve in hand, when the brush asks for one.
    pub fn pressure_points(&self) -> Option<&[(f32, f32)]> {
        self.pressure_points.as_deref()
    }

    /// How much the width in hand answers speed.
    pub fn velocity(&self) -> f32 {
        self.velocity
    }

    /// How opaque the paint in hand is.
    pub fn flow(&self) -> f32 {
        self.flow
    }

    /// How many pen samples the brush in hand averages to steady the hand (0 is off).
    pub fn stabilizer(&self) -> u8 {
        self.stabilizer
    }

    /// The tapers of the brush in hand.
    pub fn taper(&self) -> Taper {
        self.taper
    }

    /// The shape a stroke `width` wide takes with the brush in hand.
    pub fn stroke(&self, width: f64) -> BrushStroke {
        BrushStroke {
            width,
            curve: self.pressure_curve(),
            nib: self.nib,
            velocity: self.velocity,
            taper: self.taper,
        }
    }

    pub fn pressure_curve(&self) -> flare_input::PressureCurve {
        match self.pressure_points.as_deref() {
            Some(points) => flare_input::PressureCurve::from_points(self.pressure_min, points),
            None => flare_input::PressureCurve::new(self.pressure_min, self.pressure_gamma),
        }
    }

    pub fn choose_builtin(&mut self, profile: BuiltinBrushProfile) {
        self.take_holding(BuiltinBrushProfile::Plain.preset());
        self.selected = None;
        self.builtin = Some(profile);
        self.take_holding(profile.preset());
    }

    /// What the shelf holds for `preset`'s sake.
    fn take_holding(&mut self, preset: BrushPreset) {
        self.nib = preset.nib();
        self.pressure_min = preset.pressure_min();
        self.pressure_gamma = preset.pressure_gamma();
        self.pressure_points = preset.pressure_points();
        self.velocity = preset.velocity();
        self.flow = preset.flow();
        self.stabilizer = preset.stabilizer();
        self.taper = preset.taper();
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
        let preset = BrushPreset::default()
            .with_velocity(self.velocity)
            .with_flow(self.flow)
            .with_pressure_points(self.pressure_points.clone())
            .with_stabilizer(self.stabilizer)
            .with_taper(self.taper.start() as f32, self.taper.end() as f32);
        let preset = BrushPreset {
            name,
            square: self.nib.shape() == NibShape::Square,
            angle: self.nib.angle_degrees() as f32,
            flatness: self.nib.flatness_percent() as f32,
            pressure_min: self.pressure_min,
            pressure_gamma: self.pressure_gamma,
            ..preset
        }
        .sanitized();
        self.presets.push(preset);
        self.selected = Some(self.presets.len() - 1);
        self.builtin = None;
        Ok(())
    }

    /// Keeps `presets` as new custom brushes after what is on the shelf, names and all, and
    /// chooses the first of them that landed. Refuses the whole batch with [`ShelfFull`] when
    /// the shelf does not have the room: an import changes nothing a refusal undoes.
    pub fn keep_presets(&mut self, presets: Vec<BrushPreset>) -> Result<(), ShelfFull> {
        if self.presets.len() + presets.len() > MAX_BRUSHES {
            return Err(ShelfFull);
        }
        for preset in &presets {
            let mut named = preset.sanitized();
            // The same name twice on one shelf would tie two brushes to one entry, so an
            // imported brush that runs onto a kept one gains a suffix.
            if self.presets.iter().any(|kept| kept.name == named.name) {
                let n = self.presets.len() + 1;
                named.name = format!("{} {n}", named.name);
            }
            self.presets.push(named);
        }
        if !presets.is_empty() {
            let i = self.presets.len() - presets.len();
            self.selected = Some(i);
            self.builtin = None;
            self.take_holding(self.presets[i].clone());
        }
        Ok(())
    }

    /// Chooses a custom brush, or the plain round brush for `None` or a place that is not there.
    pub fn choose(&mut self, index: Option<usize>) {
        if let Some(i) = index.filter(|i| self.presets.get(*i).is_some()) {
            self.selected = Some(i);
            self.builtin = None;
            // Unwrap-free: `get` above already proved the place is there.
            if let Some(preset) = self.presets.get(i).cloned() {
                self.take_holding(preset);
            }
        } else {
            self.selected = None;
            self.choose_builtin(BuiltinBrushProfile::Plain);
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
            self.presets[i] = BrushPreset {
                name,
                square: nib.shape() == NibShape::Square,
                angle: nib.angle_degrees() as f32,
                flatness: nib.flatness_percent() as f32,
                ..self.presets[i].clone()
            }
            .sanitized();
        }
    }

    pub fn set_pressure_curve(&mut self, min: f32, gamma: f32) {
        let min = finite_range(min, 0.15, 0.0, 1.0);
        let gamma = finite_range(gamma, 1.0, 0.1, 4.0);
        if self.pressure_min == min
            && self.pressure_gamma == gamma
            && self.pressure_points.is_none()
        {
            return;
        }
        self.pressure_min = min;
        self.pressure_gamma = gamma;
        self.pressure_points = None;
        self.builtin = None;
        if let Some(i) = self.selected() {
            self.presets[i].pressure_min = self.pressure_min;
            self.presets[i].pressure_gamma = self.pressure_gamma;
            self.presets[i].pressure_points = None;
        }
    }

    /// How much the width answers speed, 0 to 1. Editing any response puts the brush off
    /// the built-in list, as its setting is no longer quite the list's.
    pub fn set_velocity(&mut self, velocity: f32) {
        let velocity = finite_range(velocity, 0.0, 0.0, 1.0);
        if self.velocity == velocity {
            return;
        }
        self.velocity = velocity;
        self.builtin = None;
        if let Some(i) = self.selected() {
            self.presets[i].velocity = self.velocity;
        }
    }

    /// How opaque the paint is at the first touch, 0 to 1.
    pub fn set_flow(&mut self, flow: f32) {
        let flow = finite_range(flow, 1.0, 0.0, 1.0);
        if self.flow == flow {
            return;
        }
        self.flow = flow;
        self.builtin = None;
        if let Some(i) = self.selected() {
            self.presets[i].flow = self.flow;
        }
    }

    /// How many pen samples are averaged to steady the hand, 0 (off) to
    /// [`flare_input::MAX_STABILIZER_LEVEL`].
    pub fn set_stabilizer(&mut self, level: u8) {
        let level = level.min(flare_input::MAX_STABILIZER_LEVEL);
        if self.stabilizer == level {
            return;
        }
        self.stabilizer = level;
        self.builtin = None;
        if let Some(i) = self.selected() {
            self.presets[i].stabilizer = level;
        }
    }

    /// How long the start and the end of a stroke taper, in stage units.
    pub fn set_taper(&mut self, start: f64, end: f64) {
        let taper = Taper::new(start, end);
        if self.taper == taper {
            return;
        }
        self.taper = taper;
        self.builtin = None;
        if let Some(i) = self.selected() {
            self.presets[i].taper_start = taper.start() as f32;
            self.presets[i].taper_end = taper.end() as f32;
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
