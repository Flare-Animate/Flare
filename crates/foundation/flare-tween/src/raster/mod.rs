//! Classical raster inbetweening: two RGBA bitmaps in, the pictures between them out.
//!
//! The method is optical flow in two directions (each picture's pixels are followed into the
//! other by a coarse-to-fine Lucas-Kanade estimate), a consistency and brightness check that
//! marks the pixels one picture shows and the other does not, then a forward warp of each
//! picture to the in-between time, blended with premultiplied colour. The pictures never change
//! themselves: a frame at 0 is the first picture and a frame at 1 is the second, exactly.
//!
//! Nothing here is learned. Pixel art with hard edges comes out soft, because the warp is
//! bilinear; snapping to the pixel grid is a separate feature.

use thiserror::Error;

mod blend;
mod flow;
mod plane;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_bounds;

use blend::{Blend, visibility};
use flow::Flow;

/// Longest side of a raster, in pixels.
pub const MAX_SIDE: u32 = 4096;
/// Most pixels in one raster (4 Mpx, about 16 MiB of RGBA).
pub const MAX_PIXELS: u64 = 1 << 22;
/// Most pyramid levels a flow estimate may use.
pub const MAX_LEVELS: u32 = 6;
/// Most Gauss-Newton steps a level may take.
pub const MAX_ITERATIONS: u32 = 16;
/// Largest half-width of the window a flow vector is solved over.
pub const MAX_WINDOW: u32 = 8;

#[derive(Debug, Error, Clone, PartialEq)]
pub enum RasterError {
    #[error("a raster needs at least one pixel and at most {MAX_SIDE} on a side")]
    Size,
    #[error("a raster is limited to {MAX_PIXELS} pixels")]
    TooManyPixels,
    #[error("a {width}x{height} raster needs {expected} bytes of RGBA, got {got}")]
    Length {
        width: u32,
        height: u32,
        expected: usize,
        got: usize,
    },
    #[error("the two rasters differ in size ({a_width}x{a_height} and {b_width}x{b_height})")]
    Mismatch {
        a_width: u32,
        a_height: u32,
        b_width: u32,
        b_height: u32,
    },
    #[error("the time must be between 0 and 1, not {0}")]
    Time(f64),
    #[error("{name} must be between {min} and {max}, not {value}")]
    Param {
        name: &'static str,
        min: u32,
        max: u32,
        value: u32,
    },
}

/// A borrowed RGBA raster, straight alpha, rows top to bottom.
#[derive(Debug, Clone, Copy)]
pub struct Raster<'a> {
    width: u32,
    height: u32,
    rgba: &'a [u8],
}

impl<'a> Raster<'a> {
    /// # Errors
    /// `Size` for a zero or oversized side, `TooManyPixels`, or `Length` when `rgba` is not
    /// exactly `width * height * 4` bytes.
    pub fn new(width: u32, height: u32, rgba: &'a [u8]) -> Result<Self, RasterError> {
        if width == 0 || height == 0 || width > MAX_SIDE || height > MAX_SIDE {
            return Err(RasterError::Size);
        }
        let pixels = u64::from(width) * u64::from(height);
        if pixels > MAX_PIXELS {
            return Err(RasterError::TooManyPixels);
        }
        let expected = (pixels * 4) as usize;
        if rgba.len() != expected {
            return Err(RasterError::Length {
                width,
                height,
                expected,
                got: rgba.len(),
            });
        }
        Ok(Self {
            width,
            height,
            rgba,
        })
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn rgba(&self) -> &'a [u8] {
        self.rgba
    }
}

/// How hard the flow estimate works. Each value is bounded, so a request cannot ask for an
/// unbounded amount of work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlowParams {
    /// Pyramid levels, the coarsest at a 2^(levels-1) reduction. Large motion needs more.
    pub levels: u32,
    /// Gauss-Newton steps per level.
    pub iterations: u32,
    /// Half-width of the window each flow vector is solved over (the window is 2w+1 square).
    pub window: u32,
}

impl Default for FlowParams {
    fn default() -> Self {
        Self {
            levels: 4,
            iterations: 4,
            window: 3,
        }
    }
}

impl FlowParams {
    fn validate(self) -> Result<(), RasterError> {
        check("levels", self.levels, 1, MAX_LEVELS)?;
        check("iterations", self.iterations, 1, MAX_ITERATIONS)?;
        check("window", self.window, 1, MAX_WINDOW)
    }
}

fn check(name: &'static str, value: u32, min: u32, max: u32) -> Result<(), RasterError> {
    if (min..=max).contains(&value) {
        Ok(())
    } else {
        Err(RasterError::Param {
            name,
            min,
            max,
            value,
        })
    }
}

/// The estimated motion between two rasters, ready to make any number of in-betweens. The flow
/// is worked out once, so making ten frames costs the warps, not ten estimates.
#[derive(Debug)]
pub struct Inbetweener<'a> {
    a: Raster<'a>,
    b: Raster<'a>,
    forward: Flow,
    backward: Flow,
    visible_a: Vec<bool>,
    visible_b: Vec<bool>,
}

impl<'a> Inbetweener<'a> {
    /// Estimates the flow from `a` to `b` and back, and which pixels each picture shows.
    ///
    /// # Errors
    /// `Mismatch` when the sizes differ, or `Param` for a flow setting out of range.
    pub fn new(a: Raster<'a>, b: Raster<'a>, params: FlowParams) -> Result<Self, RasterError> {
        if a.width != b.width || a.height != b.height {
            return Err(RasterError::Mismatch {
                a_width: a.width,
                a_height: a.height,
                b_width: b.width,
                b_height: b.height,
            });
        }
        params.validate()?;
        let pa = plane::pyramid(&a, params.levels);
        let pb = plane::pyramid(&b, params.levels);
        let forward = flow::estimate(&pa, &pb, params);
        let backward = flow::estimate(&pb, &pa, params);
        let (visible_a, visible_b) = visibility(a, b, &forward, &backward);
        Ok(Self {
            a,
            b,
            forward,
            backward,
            visible_a,
            visible_b,
        })
    }

    pub fn width(&self) -> u32 {
        self.a.width
    }

    pub fn height(&self) -> u32 {
        self.a.height
    }

    /// The picture at time `t`: 0 is the first raster, 1 the second, and in between a blend of
    /// both pictures moved along the flow. Fully transparent pixels come out as zeros.
    ///
    /// # Errors
    /// `Time` when `t` is not a number between 0 and 1.
    pub fn frame(&self, t: f64) -> Result<Vec<u8>, RasterError> {
        if !(0.0..=1.0).contains(&t) {
            return Err(RasterError::Time(t));
        }
        let t = t as f32;
        let blend = Blend {
            a: self.a,
            b: self.b,
            forward: &self.forward,
            backward: &self.backward,
            visible_a: &self.visible_a,
            visible_b: &self.visible_b,
        };
        Ok(blend.frame(t))
    }
}

/// Flow vectors and visibility, read by the tests in this module.
#[cfg(test)]
impl Inbetweener<'_> {
    pub(crate) fn forward_flow(&self) -> &[[f32; 2]] {
        &self.forward
    }

    pub(crate) fn visible_a(&self) -> &[bool] {
        &self.visible_a
    }
}
