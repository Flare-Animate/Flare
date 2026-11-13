//! The pixels a Blur or Smudge stroke works on. The stroke never paints over its own work: each
//! pixel it reaches keeps how much of it the stroke has covered so far, capped at the stroke's
//! strength, and its color is rebuilt from the bitmap as it was before the stroke began. So going
//! over the same place again in one stroke does not blur or smear it further; lifting the pen and
//! going again does.
//!
//! What is kept per pixel is stored in square tiles, made only where the stroke goes, so a short
//! stroke on a large bitmap costs what it touches.

use std::collections::HashMap;
use std::sync::Arc;

use super::dab::Dab;

/// The side of a tile, in pixels.
const TILE: u32 = 64;
const TILE_AREA: usize = (TILE * TILE) as usize;

/// A color with its alpha multiplied in, each channel 0 to 1.
pub(super) type Premul = [f32; 4];

/// What the stroke keeps for each pixel of one tile.
struct Tile {
    /// How much of each pixel the stroke has covered, 0 to the strength.
    cover: Vec<f32>,
    /// What each pixel is moving towards, premultiplied, 0 to 65535 a channel: the blurred
    /// bitmap for Blur, the paint the stroke has laid for Smudge.
    target: Vec<[u16; 4]>,
}

pub(super) struct Canvas {
    pub(super) width: u32,
    pub(super) height: u32,
    /// The bitmap as it was when the stroke began: straight RGBA8, shared, never copied.
    backup: Arc<[u8]>,
    /// The bitmap as the stroke has left it, straight RGBA8.
    pub(super) working: Vec<u8>,
    tiles: HashMap<(u32, u32), Tile>,
    /// For Blur, the radius of each of the three box blurs that make the blurred bitmap.
    blur: Option<u32>,
    /// A pixel of `working` changed since this was last cleared.
    pub(super) dirty: bool,
}

impl Canvas {
    /// `blur` is the box radius for a Blur stroke, `None` for Smudge.
    pub(super) fn new(width: u32, height: u32, backup: Arc<[u8]>, blur: Option<u32>) -> Self {
        Self {
            width,
            height,
            working: backup.to_vec(),
            backup,
            tiles: HashMap::new(),
            blur: blur.map(|b| b.max(1)),
            dirty: false,
        }
    }

    /// The pixel at `index` (in pixels, not bytes) as it was before the stroke.
    fn before(&self, index: usize) -> Premul {
        premultiply(&self.backup[index * 4..index * 4 + 4])
    }

    /// One Blur dab: each pixel it covers moves towards the blurred bitmap.
    pub(super) fn blur_dab(&mut self, dab: &Dab, ceiling: f32) {
        self.each_pixel(dab, |canvas, tile, offset, index, coverage| {
            let Some((_, new)) = cover(tile, offset, coverage * dab.strength * ceiling, ceiling)
            else {
                return;
            };
            let before = canvas.before(index);
            let goal = unpack(tile.target[offset]);
            canvas.put(index, lerp(before, goal, new));
        });
    }

    /// One Smudge dab laying `paint` (the color the stroke carries, or `None` before it has
    /// picked any up), and what the bitmap under the dab was before the stroke: the coverage
    /// weighted average, `None` where the dab is off the bitmap.
    pub(super) fn smudge_dab(
        &mut self,
        dab: &Dab,
        ceiling: f32,
        paint: Option<Premul>,
    ) -> Option<Premul> {
        let mut sum = [0.0f32; 4];
        let mut weight = 0.0f32;
        self.each_pixel(dab, |canvas, tile, offset, index, coverage| {
            let before = canvas.before(index);
            for (s, b) in sum.iter_mut().zip(before) {
                *s += b * coverage;
            }
            weight += coverage;
            let Some(paint) = paint else {
                return;
            };
            let Some((old, new)) = cover(tile, offset, coverage * dab.strength * ceiling, ceiling)
            else {
                return;
            };
            // The new paint takes the share of the pixel's paint that this dab added.
            let laid = lerp(unpack(tile.target[offset]), paint, (new - old) / new);
            tile.target[offset] = pack(laid);
            canvas.put(index, lerp(before, laid, new));
        });
        (weight > 0.0).then(|| sum.map(|s| s / weight))
    }

    /// Calls `visit` for each pixel `dab` covers, with its tile, its place in the tile and in
    /// the bitmap, and the coverage.
    fn each_pixel(
        &mut self,
        dab: &Dab,
        mut visit: impl FnMut(&mut Canvas, &mut Tile, usize, usize, f32),
    ) {
        let Some([x0, x1, y0, y1]) = dab.bounds(self.width, self.height) else {
            return;
        };
        for ty in y0 / TILE..=(y1 - 1) / TILE {
            for tx in x0 / TILE..=(x1 - 1) / TILE {
                // The tile is taken out while it is worked on, so the canvas stays free to read.
                let mut tile = match self.tiles.remove(&(tx, ty)) {
                    Some(tile) => tile,
                    None => self.new_tile(tx, ty),
                };
                let (left, top) = (tx * TILE, ty * TILE);
                for py in y0.max(top)..y1.min(top + TILE) {
                    for px in x0.max(left)..x1.min(left + TILE) {
                        let coverage = dab.coverage(px, py);
                        if coverage <= 0.0 {
                            continue;
                        }
                        let offset = ((py - top) * TILE + (px - left)) as usize;
                        let index = (py * self.width + px) as usize;
                        visit(self, &mut tile, offset, index, coverage);
                    }
                }
                self.tiles.insert((tx, ty), tile);
            }
        }
    }

    fn new_tile(&self, tx: u32, ty: u32) -> Tile {
        let target = match self.blur {
            Some(radius) => self.blurred_tile(tx, ty, radius),
            None => vec![[0; 4]; TILE_AREA],
        };
        Tile {
            cover: vec![0.0; TILE_AREA],
            target,
        }
    }

    /// Tile `(tx, ty)` of the bitmap before the stroke, blurred by three box blurs of `radius`
    /// (close to a Gaussian), in premultiplied color so clear pixels add no dark fringe. The
    /// bitmap's edge pixels are taken as reaching on beyond it.
    fn blurred_tile(&self, tx: u32, ty: u32, radius: u32) -> Vec<[u16; 4]> {
        let margin = 3 * radius as i64;
        let (left, top) = (i64::from(tx * TILE), i64::from(ty * TILE));
        let side = (i64::from(TILE) + 2 * margin) as usize;
        let (w, h) = (i64::from(self.width), i64::from(self.height));
        let mut region = Vec::with_capacity(side * side);
        for row in 0..side as i64 {
            let y = (top - margin + row).clamp(0, h - 1);
            for col in 0..side as i64 {
                let x = (left - margin + col).clamp(0, w - 1);
                region.push(self.before((y * w + x) as usize));
            }
        }
        let mut scratch = vec![[0.0f32; 4]; side];
        for _ in 0..3 {
            for row in 0..side {
                box_pass(&mut region, row * side, 1, side, radius, &mut scratch);
            }
            for col in 0..side {
                box_pass(&mut region, col, side, side, radius, &mut scratch);
            }
        }
        let mut out = Vec::with_capacity(TILE_AREA);
        let m = margin as usize;
        for row in 0..TILE as usize {
            for col in 0..TILE as usize {
                out.push(pack(region[(row + m) * side + col + m]));
            }
        }
        out
    }

    /// Writes `color` to pixel `index` of the working bitmap.
    fn put(&mut self, index: usize, color: Premul) {
        let pixel = straight(color);
        let at = &mut self.working[index * 4..index * 4 + 4];
        if at != pixel {
            at.copy_from_slice(&pixel);
            self.dirty = true;
        }
    }
}

/// Adds `amount` of coverage to the pixel at `offset`, up to `ceiling`; gives the coverage before
/// and after, or `None` when the pixel was already at the ceiling or nothing was added.
fn cover(tile: &mut Tile, offset: usize, amount: f32, ceiling: f32) -> Option<(f32, f32)> {
    let old = tile.cover[offset];
    if old >= ceiling || amount <= 0.0 {
        return None;
    }
    let new = (old + (1.0 - old) * amount).min(ceiling);
    tile.cover[offset] = new;
    (new > old).then_some((old, new))
}

/// One box blur of `radius` along `count` values of `values` from `start`, `stride` apart, with
/// the ends repeated past the edge.
fn box_pass(
    values: &mut [Premul],
    start: usize,
    stride: usize,
    count: usize,
    radius: u32,
    scratch: &mut [Premul],
) {
    let r = radius as i64;
    let last = count as i64 - 1;
    let at = |i: i64| start + i.clamp(0, last) as usize * stride;
    let mut sum = [0.0f32; 4];
    for i in -r..=r {
        add(&mut sum, values[at(i)], 1.0);
    }
    let scale = 1.0 / (2 * r + 1) as f32;
    for (i, slot) in scratch.iter_mut().enumerate().take(count) {
        *slot = sum.map(|s| s * scale);
        let i = i as i64;
        add(&mut sum, values[at(i + r + 1)], 1.0);
        add(&mut sum, values[at(i - r)], -1.0);
    }
    for (i, value) in scratch.iter().enumerate().take(count) {
        values[start + i * stride] = *value;
    }
}

fn add(sum: &mut Premul, value: Premul, sign: f32) {
    for (s, v) in sum.iter_mut().zip(value) {
        *s += v * sign;
    }
}

pub(super) fn lerp(from: Premul, to: Premul, t: f32) -> Premul {
    let mut out = from;
    for (o, (f, t2)) in out.iter_mut().zip(from.into_iter().zip(to)) {
        *o = f + (t2 - f) * t;
    }
    out
}

fn premultiply(pixel: &[u8]) -> Premul {
    let a = f32::from(pixel[3]) / 255.0;
    [
        f32::from(pixel[0]) / 255.0 * a,
        f32::from(pixel[1]) / 255.0 * a,
        f32::from(pixel[2]) / 255.0 * a,
        a,
    ]
}

/// Back to straight RGBA8; a pixel with no alpha left is clear black, as a cleared pixel is.
fn straight(color: Premul) -> [u8; 4] {
    let a = color[3].clamp(0.0, 1.0);
    let alpha = (a * 255.0).round() as u8;
    if alpha == 0 {
        return [0; 4];
    }
    let channel = |c: f32| ((c / a).clamp(0.0, 1.0) * 255.0).round() as u8;
    [
        channel(color[0]),
        channel(color[1]),
        channel(color[2]),
        alpha,
    ]
}

fn pack(color: Premul) -> [u16; 4] {
    color.map(|c| (c.clamp(0.0, 1.0) * 65535.0).round() as u16)
}

fn unpack(color: [u16; 4]) -> Premul {
    color.map(|c| f32::from(c) / 65535.0)
}
