#![allow(clippy::expect_used)]

mod edits;
mod graph;
mod ops;
mod running;

use crate::{Pixels, Space};

/// A `w` by `h` picture with an opaque square of `color` from `x0, y0` to `x1, y1` (exclusive).
pub(super) fn square(w: u32, h: u32, rect: [u32; 4], color: [u8; 4]) -> Pixels {
    let mut data = vec![0u8; (w * h * 4) as usize];
    for y in rect[1]..rect[3] {
        for x in rect[0]..rect[2] {
            let i = ((y * w + x) * 4) as usize;
            data[i..i + 4].copy_from_slice(&color);
        }
    }
    Pixels::new(w, h, data).expect("pixels")
}

pub(super) fn whole(w: u32, h: u32) -> Space {
    Space::stage(f64::from(w), f64::from(h))
}

/// The sum of every pixel's alpha.
pub(super) fn total_alpha(p: &Pixels) -> u64 {
    p.data()
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| u64::from(c[3]))
        .sum()
}
