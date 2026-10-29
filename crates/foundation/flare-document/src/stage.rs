use flare_core::Rgba8;
use thiserror::Error;

/// Largest stage side; matches the renderer's per-side limit.
pub const MAX_STAGE_SIDE: u32 = 16_384;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum StageError {
    #[error("stage sides must be between 1 and {MAX_STAGE_SIDE} pixels, got {0}x{1}")]
    InvalidSize(u32, u32),
}

/// Stage dimensions in document units (1 unit = 1 pixel at 100% zoom) and background.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stage {
    width: u32,
    height: u32,
    background: Rgba8,
}

impl Stage {
    pub fn new(width: u32, height: u32, background: Rgba8) -> Result<Self, StageError> {
        let valid = |side: u32| (1..=MAX_STAGE_SIDE).contains(&side);
        if !valid(width) || !valid(height) {
            return Err(StageError::InvalidSize(width, height));
        }
        Ok(Self {
            width,
            height,
            background,
        })
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn background(&self) -> Rgba8 {
        self.background
    }
}

impl Default for Stage {
    /// 1920x1080 on white.
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            background: Rgba8::WHITE,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_are_bounded() {
        assert!(Stage::new(1, 1, Rgba8::WHITE).is_ok());
        assert!(Stage::new(MAX_STAGE_SIDE, MAX_STAGE_SIDE, Rgba8::WHITE).is_ok());
        assert_eq!(
            Stage::new(0, 10, Rgba8::WHITE),
            Err(StageError::InvalidSize(0, 10))
        );
        assert_eq!(
            Stage::new(10, MAX_STAGE_SIDE + 1, Rgba8::WHITE),
            Err(StageError::InvalidSize(10, MAX_STAGE_SIDE + 1))
        );
    }

    #[test]
    fn default_is_full_hd_white() {
        let stage = Stage::default();
        assert_eq!(
            (stage.width(), stage.height(), stage.background()),
            (1920, 1080, Rgba8::WHITE)
        );
    }
}
