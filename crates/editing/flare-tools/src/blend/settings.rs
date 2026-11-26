//! What a stroke on a bitmap is and how it is set: the tools that lay dabs on pixels (Blur,
//! Smudge, the Brush on a raster layer and the Eraser there), their settings, and why a stroke is
//! refused.

use thiserror::Error;

use crate::drawing::DrawRefusal;

use super::presets::RasterPresetSettings;

/// The largest size, in stage units.
pub const MAX_SIZE: f32 = 1000.0;
/// The most Color stretch.
pub const MAX_STRETCH: u8 = 100;

/// Which of the tools a stroke is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendKind {
    /// Softens what it passes over.
    Blur,
    /// Pushes colors along the drag, as a finger through wet paint.
    Smudge,
    /// Paints a color into the pixels, mixing in as much of the color already there as its
    /// Color mix says: the Brush on a raster layer.
    RasterBrush,
    /// Rubs pixels out, leaving them clear: the Eraser on a raster layer.
    Erase,
}

impl BlendKind {
    pub const ALL: [BlendKind; 4] = [Self::Blur, Self::Smudge, Self::RasterBrush, Self::Erase];

    /// The word for it on a command line and in a message key.
    pub fn name(self) -> &'static str {
        match self {
            Self::Blur => "blur",
            Self::Smudge => "smudge",
            Self::RasterBrush => "raster-brush",
            Self::Erase => "erase",
        }
    }

    /// Its name in English, as an undo step and an error message are headed.
    pub fn label(self) -> &'static str {
        match self {
            Self::Blur => "Blur",
            Self::Smudge => "Smudge",
            Self::RasterBrush => "Raster Brush",
            Self::Erase => "Eraser",
        }
    }

    pub fn parse(word: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|k| k.name().eq_ignore_ascii_case(word.trim()))
    }

    /// Whether the tool picks up colors and carries them, so Color stretch means something.
    pub fn carries_color(self) -> bool {
        matches!(self, Self::Smudge | Self::RasterBrush)
    }

    /// Whether each dab lays at full strength, a pixel keeping the most any dab gave it (painting
    /// and rubbing out), rather than building up dab by dab (Blur and Smudge).
    pub(super) fn lays(self) -> bool {
        matches!(self, Self::RasterBrush | Self::Erase)
    }
}

/// A Blur, Smudge or Raster Brush tool's settings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BlendSettings {
    /// The width of a dab at full pressure, in stage units, 1 to [`MAX_SIZE`].
    pub size: f32,
    /// How far a stroke goes at most, 0 to 1: at 1 the middle of a Blur stroke is the blurred
    /// picture, the middle of a Smudge stroke is the carried color, and the middle of a Raster
    /// Brush stroke is its paint. For the Raster Brush this is its opacity.
    pub strength: f32,
    /// How sharp a dab's edge is, 0 (soft) to 1.
    pub hardness: f32,
    /// Pen pressure sets the size of each dab; off, every dab is full size.
    pub pressure_size: bool,
    /// Smudge and Raster Brush: how far a picked-up color is carried, 0 (hardly at all, which
    /// mixes in place) to [`MAX_STRETCH`].
    pub stretch: u8,
    /// Raster Brush only: how much of the color under the brush mixes into the paint, 0 (none:
    /// the paint lays as it is) to 1 (all: the brush smears what is there). Clear pixels have
    /// no color to give, so on an empty bitmap the paint lays as it is whatever the mix.
    pub mix: f32,
}

impl BlendSettings {
    /// Where each tool starts: Blur and Smudge from Clip Studio's own Blur and Finger tip; the
    /// Raster Brush half way, so a stroke over another color blends with it at once.
    pub fn for_kind(kind: BlendKind) -> Self {
        match kind {
            BlendKind::Blur => Self {
                size: 80.0,
                strength: 1.0,
                hardness: 0.0,
                pressure_size: true,
                stretch: 0,
                mix: 0.0,
            },
            BlendKind::Smudge => Self {
                size: 30.0,
                strength: 1.0,
                hardness: 0.5,
                pressure_size: true,
                stretch: 70,
                mix: 0.0,
            },
            // A hard round brush that lays its color as it is, as a painting program's default
            // brush does: a soft edge or a mix are a preset away (see `RasterPreset`).
            BlendKind::RasterBrush => RasterPresetSettings::ROUND,
            BlendKind::Erase => Self {
                size: 20.0,
                strength: 1.0,
                hardness: 1.0,
                pressure_size: true,
                stretch: 0,
                mix: 0.0,
            },
        }
    }

    pub fn validate(&self) -> Result<(), BlendError> {
        if !(1.0..=MAX_SIZE).contains(&self.size) {
            return Err(BlendError::Size);
        }
        if !(0.0..=1.0).contains(&self.strength) {
            return Err(BlendError::Strength);
        }
        if !(0.0..=1.0).contains(&self.hardness) {
            return Err(BlendError::Hardness);
        }
        if self.stretch > MAX_STRETCH {
            return Err(BlendError::Stretch);
        }
        if !(0.0..=1.0).contains(&self.mix) {
            return Err(BlendError::Mix);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum BlendError {
    #[error("Blur and Smudge work on bitmaps; there is no bitmap where the stroke starts")]
    NoBitmap,
    #[error("there is no painting on this layer where the stroke starts to rub out")]
    NothingToErase,
    #[error("the element is not a bitmap any more")]
    NotBitmap,
    #[error("the bitmap changed while the stroke was being made")]
    Stale,
    #[error("the bitmap is squashed flat, so a stroke cannot reach its pixels")]
    Flat,
    #[error("give at least one point")]
    NoPoints,
    #[error("at most {MAX_POINTS} points at a time", MAX_POINTS = super::MAX_POINTS)]
    TooManyPoints,
    #[error("the size is 1 to {MAX_SIZE}")]
    Size,
    #[error("the strength is 0 to 100%")]
    Strength,
    #[error("the hardness is 0 to 100%")]
    Hardness,
    #[error("the color stretch is 0 to {MAX_STRETCH}")]
    Stretch,
    #[error("the color mix is 0 to 100%")]
    Mix,
    #[error("the stroke did not change any pixel")]
    NothingChanged,
    #[error("the bitmap could not be rebuilt: {0}")]
    Bitmap(String),
    #[error("the Raster Brush needs a color and a layer to paint on")]
    NeedsPaint,
    #[error("{0}")]
    Refused(DrawRefusal),
    #[error(
        "the stage is {0}x{1}, larger than a bitmap may be, so there is nothing to paint on; \
         paint on a picture that is already there"
    )]
    StageTooLarge(u32, u32),
}
