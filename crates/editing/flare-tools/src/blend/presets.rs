//! The Brush's presets on a raster layer: named sets of size, opacity, hardness, pressure and
//! color mixing, as a painting program's brush list offers a hard round, a pencil, an airbrush
//! and a blender.

use super::BlendSettings;

/// A named set of settings for the Brush on a raster layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RasterPreset {
    /// Hard and opaque, its width following the pen: the brush a new install starts with.
    Round,
    /// A soft edge, for shading.
    SoftRound,
    /// Thin, hard and slightly see-through, for sketching.
    Pencil,
    /// Hard and opaque, thin under a light touch and wide under a firm one, for line art.
    Ink,
    /// Flat, a little see-through and the same width whatever the pressure, for filling in.
    Marker,
    /// Wide, very soft and faint, building up stroke over stroke.
    Airbrush,
    /// Paint that mixes with the colors already there as wet paint does.
    WetPaint,
    /// No paint of its own: smears and blends what is there.
    Blender,
}

/// The settings each preset stands for, kept apart so `BlendSettings::for_kind` can start from
/// the round brush.
pub(super) struct RasterPresetSettings;

impl RasterPresetSettings {
    pub(super) const ROUND: BlendSettings = BlendSettings {
        size: 20.0,
        strength: 1.0,
        hardness: 1.0,
        pressure_size: true,
        stretch: 30,
        mix: 0.0,
    };
}

impl RasterPreset {
    pub const ALL: [RasterPreset; 8] = [
        Self::Round,
        Self::SoftRound,
        Self::Pencil,
        Self::Ink,
        Self::Marker,
        Self::Airbrush,
        Self::WetPaint,
        Self::Blender,
    ];

    /// The word for it on a command line and in a message key.
    pub fn name(self) -> &'static str {
        match self {
            Self::Round => "round",
            Self::SoftRound => "soft-round",
            Self::Pencil => "pencil",
            Self::Ink => "ink",
            Self::Marker => "marker",
            Self::Airbrush => "airbrush",
            Self::WetPaint => "wet-paint",
            Self::Blender => "blender",
        }
    }

    /// Its name in English.
    pub fn label(self) -> &'static str {
        match self {
            Self::Round => "Round",
            Self::SoftRound => "Soft Round",
            Self::Pencil => "Pencil",
            Self::Ink => "Ink",
            Self::Marker => "Marker",
            Self::Airbrush => "Airbrush",
            Self::WetPaint => "Wet Paint",
            Self::Blender => "Blender",
        }
    }

    pub fn parse(word: &str) -> Option<Self> {
        let word = word.trim().replace([' ', '_'], "-").to_ascii_lowercase();
        Self::ALL.into_iter().find(|p| p.name() == word)
    }

    /// The settings it stands for. The color the brush paints with is the fill color, as with
    /// every preset; only how it is laid changes.
    pub fn settings(self) -> BlendSettings {
        let round = RasterPresetSettings::ROUND;
        match self {
            Self::Round => round,
            Self::SoftRound => BlendSettings {
                size: 40.0,
                hardness: 0.3,
                ..round
            },
            Self::Pencil => BlendSettings {
                size: 4.0,
                strength: 0.9,
                pressure_size: false,
                ..round
            },
            Self::Ink => BlendSettings { size: 8.0, ..round },
            Self::Marker => BlendSettings {
                size: 24.0,
                strength: 0.7,
                hardness: 0.9,
                pressure_size: false,
                ..round
            },
            Self::Airbrush => BlendSettings {
                size: 80.0,
                strength: 0.25,
                hardness: 0.0,
                pressure_size: false,
                ..round
            },
            Self::WetPaint => BlendSettings {
                size: 30.0,
                hardness: 0.8,
                mix: 0.5,
                stretch: 30,
                ..round
            },
            Self::Blender => BlendSettings {
                size: 30.0,
                hardness: 0.6,
                mix: 1.0,
                stretch: 60,
                ..round
            },
        }
    }

    /// The preset `settings` are, if they are exactly one.
    pub fn of(settings: &BlendSettings) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.settings() == *settings)
    }
}
