//! One channel of a raster as floats: the luminance the flow estimate follows, and its pyramid.

use super::Raster;

/// A `width x height` grid of floats, rows top to bottom.
#[derive(Debug, Clone)]
pub(super) struct Plane {
    pub(super) width: usize,
    pub(super) height: usize,
    pub(super) data: Vec<f32>,
}

/// Pyramids stop before a level would be smaller than this on its shorter side.
const MIN_LEVEL_SIDE: usize = 8;

impl Plane {
    /// Bilinear sample at a position in pixels, with the position clamped to the plane.
    pub(super) fn sample(&self, x: f32, y: f32) -> f32 {
        let x = x.clamp(0.0, (self.width - 1) as f32);
        let y = y.clamp(0.0, (self.height - 1) as f32);
        let (x0, y0) = (x.floor(), y.floor());
        let (fx, fy) = (x - x0, y - y0);
        let (x0, y0) = (x0 as usize, y0 as usize);
        let x1 = (x0 + 1).min(self.width - 1);
        let y1 = (y0 + 1).min(self.height - 1);
        let at = |xx: usize, yy: usize| self.data[yy * self.width + xx];
        let top = at(x0, y0) + (at(x1, y0) - at(x0, y0)) * fx;
        let bottom = at(x0, y1) + (at(x1, y1) - at(x0, y1)) * fx;
        top + (bottom - top) * fy
    }

    /// The value at a pixel, with its coordinates clamped to the plane.
    pub(super) fn get(&self, x: isize, y: isize) -> f32 {
        let x = x.clamp(0, self.width as isize - 1) as usize;
        let y = y.clamp(0, self.height as isize - 1) as usize;
        self.data[y * self.width + x]
    }

    /// Half the size, each pixel the mean of up to four.
    fn halve(&self) -> Plane {
        let width = self.width.div_ceil(2);
        let height = self.height.div_ceil(2);
        let mut data = Vec::with_capacity(width * height);
        for y in 0..height {
            for x in 0..width {
                let (x0, y0) = (2 * x as isize, 2 * y as isize);
                let sum = self.get(x0, y0)
                    + self.get(x0 + 1, y0)
                    + self.get(x0, y0 + 1)
                    + self.get(x0 + 1, y0 + 1);
                data.push(sum * 0.25);
            }
        }
        Plane {
            width,
            height,
            data,
        }
    }
}

/// The luminance of a raster, premultiplied, so a transparent pixel is black and its edge is an
/// edge of the shape rather than of an invisible colour.
fn luminance(raster: &Raster<'_>) -> Plane {
    let (pixels, _) = raster.rgba.as_chunks::<4>();
    let data = pixels
        .iter()
        .map(|p| {
            let alpha = f32::from(p[3]) / 255.0;
            let luma = 0.299 * f32::from(p[0]) + 0.587 * f32::from(p[1]) + 0.114 * f32::from(p[2]);
            luma * alpha
        })
        .collect();
    Plane {
        width: raster.width as usize,
        height: raster.height as usize,
        data,
    }
}

/// Levels of the pyramid, finest first. At most `levels`, and fewer when the coarsest would fall
/// below `MIN_LEVEL_SIDE`.
pub(super) fn pyramid(raster: &Raster<'_>, levels: u32) -> Vec<Plane> {
    let mut out = vec![luminance(raster)];
    while out.len() < levels as usize {
        let Some(last) = out.last() else { break };
        if last.width.min(last.height) / 2 < MIN_LEVEL_SIDE {
            break;
        }
        let next = last.halve();
        out.push(next);
    }
    out
}

/// Sums of `values` over a window of half-width `radius` around each pixel, with the window
/// clipped at the edges. `scratch` is reused between calls to avoid an allocation per product.
pub(super) fn box_sum(
    values: &mut [f32],
    width: usize,
    height: usize,
    radius: usize,
    scratch: &mut [f32],
) {
    for y in 0..height {
        let row = &values[y * width..(y + 1) * width];
        for x in 0..width {
            let lo = x.saturating_sub(radius);
            let hi = (x + radius).min(width - 1);
            scratch[y * width + x] = row[lo..=hi].iter().sum();
        }
    }
    for y in 0..height {
        let lo = y.saturating_sub(radius);
        let hi = (y + radius).min(height - 1);
        for x in 0..width {
            values[y * width + x] = (lo..=hi).map(|yy| scratch[yy * width + x]).sum();
        }
    }
}
