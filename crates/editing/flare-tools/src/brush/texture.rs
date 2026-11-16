//! A textured brush: the stroke split into strands across its width (bristles), each broken up
//! along its length by grain, so it reads as dry paint, charcoal or chalk instead of one solid
//! ribbon.
//!
//! It stays a vector shape. Each surviving run of a strand is one closed piece of the outline, so
//! the committed shape, the live preview and every export draw the same marks. The breaks come
//! from a noise seeded by where the stroke starts, so the same drag always makes the same
//! texture, and the preview while dragging matches what is kept.

use flare_document::MAX_SEGMENTS;
use glamx::DVec2;

use super::StrokeSides;

/// Most strands a stroke is split into.
pub const MAX_BRISTLES: u8 = 24;

/// Most points one textured stroke puts into its outline, all pieces together. Kept well inside
/// [`MAX_SEGMENTS`], so a long drag with many strands still makes a valid shape.
const MAX_TEXTURE_POINTS: usize = MAX_SEGMENTS / 4;

/// How a brush breaks up its stroke. The default is solid: one strand and no grain.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Texture {
    bristles: u8,
    grain: f32,
}

impl Texture {
    /// One solid ribbon.
    pub const SOLID: Self = Self {
        bristles: 0,
        grain: 0.0,
    };

    /// `bristles` strands (0 and 1 are one) with `grain` from 0 (unbroken) to 1 (mostly gaps);
    /// both are kept in range, and a grain that is not a number is none.
    pub fn new(bristles: u8, grain: f32) -> Self {
        Self {
            bristles: bristles.min(MAX_BRISTLES),
            grain: if grain.is_finite() {
                grain.clamp(0.0, 1.0)
            } else {
                0.0
            },
        }
    }

    pub fn bristles(&self) -> u8 {
        self.bristles
    }

    pub fn grain(&self) -> f32 {
        self.grain
    }

    /// Whether the stroke is one unbroken ribbon.
    pub fn is_solid(&self) -> bool {
        self.bristles <= 1 && self.grain <= 0.0
    }

    /// The pieces `sides` breaks into. `seed` decides the breaks. A solid texture gives `sides`
    /// back whole; a textured one gives open-ended strips (no caps), each a run of one strand.
    pub fn pieces(&self, sides: StrokeSides, seed: u32) -> Vec<StrokeSides> {
        if self.is_solid() {
            return vec![sides];
        }
        let count = sides
            .centre
            .len()
            .min(sides.left.len())
            .min(sides.right.len());
        if count < 2 {
            return Vec::new();
        }
        let strands = usize::from(self.bristles.max(1));
        // Fewer points along each strand when there are many strands on a long line.
        let stride = (count * strands * 2).div_ceil(MAX_TEXTURE_POINTS).max(1);
        let rows: Vec<usize> = (0..count)
            .step_by(stride)
            .chain((!(count - 1).is_multiple_of(stride)).then_some(count - 1))
            .collect();
        let across: Vec<DVec2> = rows
            .iter()
            .map(|&i| (sides.left[i] - sides.right[i]) / 2.0)
            .collect();
        let widest = across.iter().map(|a| a.length()).fold(0.0, f64::max);
        if widest <= 0.0 {
            return Vec::new();
        }
        let mut along = Vec::with_capacity(rows.len());
        let mut total = 0.0;
        for (k, &i) in rows.iter().enumerate() {
            if k > 0 {
                total += (sides.centre[i] - sides.centre[rows[k - 1]]).length();
            }
            along.push(total);
        }
        // A grain is about half the stroke's full width long, within reason.
        let cell = widest.clamp(1.5, 40.0);
        let lane = 1.0 / strands as f64;
        let mut out = Vec::new();
        for strand in 0..strands {
            let salt = seed ^ (strand as u32).wrapping_mul(0x9E37_79B9);
            // Each strand sits in its own lane across the line, nudged and sized a little
            // differently, never reaching into the next lane, so strands do not overlap.
            // The nudge and the half width add up to at most one lane, and the half width is
            // shrunk where the nudge would push it past the side of the stroke.
            let middle = -1.0 + lane * (2 * strand + 1) as f64;
            let (offset, half, edge) = if strands == 1 {
                (0.0, 1.0, 0.0)
            } else {
                let offset = middle + (unit(hash(salt, 1)) - 0.5) * 0.5 * lane;
                let half = lane * (0.5 + 0.25 * unit(hash(salt, 2)));
                // Outer strands touch the paper less, so they break first.
                (offset, half.min(1.0 - offset.abs()), middle.abs())
            };
            let mut run: Option<StrokeSides> = None;
            for (k, &i) in rows.iter().enumerate() {
                let light = 1.0 - across[k].length() / widest;
                let wanted = f64::from(self.grain) * (0.45 + 0.35 * light + 0.3 * edge);
                if half > 0.0 && noise(salt, along[k] / cell) >= wanted {
                    let at = sides.centre[i] + across[k] * offset;
                    let piece = run.get_or_insert_with(empty);
                    piece.centre.push(at);
                    piece.left.push(at + across[k] * half);
                    piece.right.push(at - across[k] * half);
                } else if let Some(piece) = run.take() {
                    keep(&mut out, piece);
                }
            }
            if let Some(piece) = run.take() {
                keep(&mut out, piece);
            }
        }
        out
    }
}

fn empty() -> StrokeSides {
    StrokeSides {
        centre: Vec::new(),
        left: Vec::new(),
        right: Vec::new(),
        end_cap: Vec::new(),
        start_cap: Vec::new(),
    }
}

/// Keeps a run long enough to be a piece of outline.
fn keep(out: &mut Vec<StrokeSides>, piece: StrokeSides) {
    if piece.centre.len() >= 2 {
        out.push(piece);
    }
}

/// A seed for the stroke that starts at `start`: the same start, the same texture.
pub fn seed_at(x: f64, y: f64) -> u32 {
    let fold = |v: f64| {
        let bits = v.to_bits();
        (bits ^ (bits >> 32)) as u32
    };
    hash(fold(x), fold(y))
}

fn hash(a: u32, b: u32) -> u32 {
    let mut h = a.wrapping_mul(0x85EB_CA6B) ^ b.wrapping_mul(0xC2B2_AE35);
    h ^= h >> 16;
    h = h.wrapping_mul(0x7FEB_352D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x846C_A68B);
    h ^ (h >> 16)
}

fn unit(h: u32) -> f64 {
    f64::from(h) / f64::from(u32::MAX)
}

/// Smooth value noise in 0..1 at `t` cells along a strand.
fn noise(salt: u32, t: f64) -> f64 {
    let t = if t.is_finite() { t.max(0.0) } else { 0.0 };
    let cell = t.floor();
    let f = t - cell;
    let index = (cell as u64 & 0xFFFF_FFFF) as u32;
    let a = unit(hash(salt, index));
    let b = unit(hash(salt, index.wrapping_add(1)));
    let s = f * f * (3.0 - 2.0 * f);
    a + (b - a) * s
}
