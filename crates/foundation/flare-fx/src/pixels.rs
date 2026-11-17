//! The pictures a graph works on, and where the stage lies in them.

use crate::FxError;

/// A picture as premultiplied 8-bit RGBA, row by row: the form the rasterizer draws in.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Pixels {
    width: u32,
    height: u32,
    data: Vec<u8>,
}

impl Pixels {
    /// A picture of `data`, which must hold exactly `width * height` pixels.
    pub fn new(width: u32, height: u32, data: Vec<u8>) -> Result<Self, FxError> {
        let expected = (width as usize)
            .checked_mul(height as usize)
            .and_then(|n| n.checked_mul(4))
            .ok_or(FxError::BadPixels)?;
        if data.len() != expected {
            return Err(FxError::BadPixels);
        }
        Ok(Self {
            width,
            height,
            data,
        })
    }

    /// A transparent picture, or `BadPixels` when `width * height` pixels cannot be held.
    pub fn transparent(width: u32, height: u32) -> Result<Self, FxError> {
        let len = (width as usize)
            .checked_mul(height as usize)
            .and_then(|n| n.checked_mul(4))
            .ok_or(FxError::BadPixels)?;
        Ok(Self {
            width,
            height,
            data: vec![0; len],
        })
    }

    /// A transparent picture the size of this one. It cannot fail: this one's size is held.
    pub fn cleared(&self) -> Self {
        Self {
            width: self.width,
            height: self.height,
            data: vec![0; self.data.len()],
        }
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn into_data(self) -> Vec<u8> {
        self.data
    }

    pub(crate) fn data_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }

    /// The pixel at `x`, `y`, premultiplied; transparent outside the picture.
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        if x >= self.width || y >= self.height {
            return [0; 4];
        }
        let i = (y as usize * self.width as usize + x as usize) * 4;
        [
            self.data[i],
            self.data[i + 1],
            self.data[i + 2],
            self.data[i + 3],
        ]
    }

    /// Each pixel's alpha, row by row.
    pub(crate) fn alpha(&self) -> Vec<u8> {
        self.data.as_chunks::<4>().0.iter().map(|p| p[3]).collect()
    }
}

/// Where the stage lies in a picture: how many pixels one stage unit is, and the pixel where the
/// stage's top-left corner is. Effects measure their lengths on the stage, and snow falls over
/// the stage's rectangle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Space {
    /// Pixels per stage unit.
    pub scale: f64,
    /// The pixel position of the stage's top-left corner (it may be outside the picture).
    pub origin: [f64; 2],
    /// The stage's width and height, in stage units.
    pub stage: [f64; 2],
}

impl Space {
    /// A picture of the whole stage at one pixel per unit.
    pub fn stage(width: f64, height: f64) -> Self {
        Self {
            scale: 1.0,
            origin: [0.0, 0.0],
            stage: [width, height],
        }
    }
}
