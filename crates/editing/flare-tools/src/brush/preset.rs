//! A brush as it is kept and passed around: a named nib with its pen response, the ready-made
//! profiles, and the file a set of them is saved in.

use serde::{Deserialize, Serialize};

use super::{MAX_TAPER, Nib, NibShape, Taper};

/// Most custom brushes kept.
pub const MAX_BRUSHES: usize = 32;
/// Longest brush name kept, in characters.
const MAX_NAME_CHARS: usize = 40;
/// Saved brush shelves are deliberately small and bounded.
pub const MAX_BRUSH_PROFILE_BYTES: usize = 64 * 1024;

fn default_flow() -> f32 {
    1.0
}

/// A named nib and its pen response, as kept in the preferences.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BrushPreset {
    pub name: String,
    pub square: bool,
    pub angle: f32,
    pub flatness: f32,
    pub pressure_min: f32,
    pub pressure_gamma: f32,
    /// The pressure-to-width curve drawn point by point (`0..=1`, `0..=1` each), as an imported
    /// Krita or Photoshop preset carries it. `None` (or too few points) keeps the
    /// `pressure_gamma` shape.
    pub pressure_points: Option<Vec<(f32, f32)>>,
    /// How much the width answers speed rather than pressure, 0 to 1.
    pub velocity: f32,
    /// How opaque the paint is at the first touch, 0 to 1.
    #[serde(default = "default_flow")]
    pub flow: f32,
    /// How many pen samples are averaged to steady the hand, 0 (off) to
    /// [`flare_input::MAX_STABILIZER_LEVEL`]; fewer are averaged when the pen moves fast.
    pub stabilizer: u8,
    /// How long the start of a stroke tapers in, in stage units (0 for none).
    pub taper_start: f32,
    /// How long the end of a stroke tapers out, in stage units (0 for none).
    pub taper_end: f32,
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
            pressure_points: None,
            velocity: 0.0,
            flow: 1.0,
            stabilizer: 0,
            taper_start: 0.0,
            taper_end: 0.0,
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
            ..Self::default()
        }
    }

    /// The preset with its drawn pressure curve set (`None` restores the gamma shape).
    #[must_use]
    pub fn with_pressure_points(mut self, points: Option<Vec<(f32, f32)>>) -> Self {
        self.pressure_points = points;
        self
    }

    #[must_use]
    pub fn with_velocity(mut self, velocity: f32) -> Self {
        self.velocity = velocity;
        self
    }

    #[must_use]
    pub fn with_flow(mut self, flow: f32) -> Self {
        self.flow = flow;
        self
    }

    #[must_use]
    pub fn with_stabilizer(mut self, level: u8) -> Self {
        self.stabilizer = level;
        self
    }

    #[must_use]
    pub fn with_taper(mut self, start: f32, end: f32) -> Self {
        self.taper_start = start;
        self.taper_end = end;
        self
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

    /// The drawn pressure points, cleaned up, or `None` when the gamma shape is kept.
    pub fn pressure_points(&self) -> Option<Vec<(f32, f32)>> {
        self.pressure_points
            .as_ref()
            .filter(|points| points.len() >= 2)
            .map(|points| {
                flare_input::PressureCurve::from_points(self.pressure_min(), points)
                    .points()
                    .to_vec()
            })
            .filter(|points| !points.is_empty())
    }

    pub fn velocity(&self) -> f32 {
        finite_range(self.velocity, 0.0, 0.0, 1.0)
    }

    pub fn flow(&self) -> f32 {
        finite_range(self.flow, 1.0, 0.0, 1.0)
    }

    pub fn stabilizer(&self) -> u8 {
        self.stabilizer.min(flare_input::MAX_STABILIZER_LEVEL)
    }

    /// The tapers at the two ends, in range.
    pub fn taper(&self) -> Taper {
        let max = MAX_TAPER as f32;
        Taper::new(
            f64::from(finite_range(self.taper_start, 0.0, 0.0, max)),
            f64::from(finite_range(self.taper_end, 0.0, 0.0, max)),
        )
    }

    /// The width response the preset asks for.
    pub fn pressure_curve(&self) -> flare_input::PressureCurve {
        match self.pressure_points() {
            Some(points) => flare_input::PressureCurve::from_points(self.pressure_min(), &points),
            None => flare_input::PressureCurve::new(self.pressure_min(), self.pressure_gamma()),
        }
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
        // The nib turns angle into 0..360 and flatness into 1..100, keeping bad numbers out.
        let nib = self.nib();
        Self {
            name,
            square: nib.shape() == NibShape::Square,
            angle: nib.angle_degrees() as f32,
            flatness: nib.flatness_percent() as f32,
            pressure_min: self.pressure_min(),
            pressure_gamma: self.pressure_gamma(),
            pressure_points: self.pressure_points(),
            velocity: self.velocity(),
            flow: self.flow(),
            stabilizer: self.stabilizer(),
            taper_start: self.taper().start() as f32,
            taper_end: self.taper().end() as f32,
        }
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
    GPen,
    MappingPen,
    TurnipPen,
    BrushPen,
}

impl BuiltinBrushProfile {
    pub const ALL: [Self; 11] = [
        Self::Plain,
        Self::Pencil,
        Self::Ink,
        Self::GreasePencil,
        Self::Calligraphy,
        Self::Vector,
        Self::Animate,
        Self::GPen,
        Self::MappingPen,
        Self::TurnipPen,
        Self::BrushPen,
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
            Self::GPen => "G-pen",
            Self::MappingPen => "Mapping pen",
            Self::TurnipPen => "Turnip pen",
            Self::BrushPen => "Brush pen",
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
            // Manga inking nibs: hard and opaque, thinning to nothing at no pressure, each
            // with its own drawn curve below.
            Self::GPen | Self::MappingPen | Self::TurnipPen | Self::BrushPen => {
                (false, 0.0, 100.0, 0.0, 1.0)
            }
        };
        let base = BrushPreset {
            name: self.name().to_owned(),
            square,
            angle,
            flatness,
            pressure_min,
            pressure_gamma,
            ..BrushPreset::default()
        };
        // What the workflows these presets are shaped around draw like: graphite answers speed a
        // little and does not cover in one pass, and a Grease Pencil stroke thins when the hand
        // is quick.
        //
        // The inking pens steady the hand and taper both ends, as a dipped nib lifts in and out
        // of a line. A G-pen widens evenly with pressure; a mapping pen is fine and needs a firm
        // press to widen; a turnip pen widens early; a brush pen stays a hairline under a light
        // touch, comes in late, and thins when the hand is quick.
        match self {
            Self::Pencil => base.with_velocity(0.2).with_flow(0.9),
            Self::GreasePencil => base.with_velocity(0.35),
            Self::GPen => base.with_stabilizer(6).with_taper(20.0, 20.0),
            Self::MappingPen => base
                .with_pressure_points(Some(vec![
                    (0.0, 0.0),
                    (0.5, 0.245),
                    (0.927, 0.709),
                    (1.0, 1.0),
                ]))
                .with_stabilizer(10)
                .with_taper(20.0, 20.0),
            Self::TurnipPen => base
                .with_pressure_points(Some(vec![
                    (0.0, 0.0),
                    (0.182, 0.4),
                    (0.7, 0.964),
                    (1.0, 1.0),
                ]))
                .with_stabilizer(2)
                .with_taper(20.0, 20.0),
            Self::BrushPen => base
                .with_pressure_points(Some(vec![
                    (0.0, 0.0),
                    (0.145, 0.0),
                    (0.3, 0.073),
                    (1.0, 1.0),
                ]))
                .with_velocity(0.3)
                .with_stabilizer(15),
            Self::Plain | Self::Ink | Self::Calligraphy | Self::Vector | Self::Animate => base,
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

pub(super) fn finite_range(value: f32, fallback: f32, min: f32, max: f32) -> f32 {
    if value.is_finite() {
        value.clamp(min, max)
    } else {
        fallback
    }
}
