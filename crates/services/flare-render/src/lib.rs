//! CPU rasterization of an evaluated frame with vello_cpu.
//!
//! Document units map 1:1 to pixels with the origin at the top-left. Output is straight
//! (non-premultiplied) 8-bit sRGB RGBA. This crate owns only drawing; timeline evaluation
//! lives in `flare-eval` and the file formats elsewhere.
#![forbid(unsafe_code)]

mod hit;
mod image;
mod image_formats;
mod ink;
mod limits;
mod outline;
mod paint;
mod pattern;
mod render;
mod viewport;

pub use hit::{PaintHit, hit_test, hit_test_all, paint_hits};
pub use image::{Image, PremultipliedImage, StillFormat};
pub use image_formats::MAX_ICO_SIDE;
pub use ink::{ink_bounds, ink_mask};
pub use limits::{MAX_DIMENSION, MAX_PIXELS, RenderError};
pub use outline::{render_onion_outlines_viewport, render_outline_mode_viewport};
pub use render::{
    render, render_onion, render_onion_viewport, render_viewport, render_viewport_premultiplied,
};
pub use viewport::Viewport;

#[cfg(test)]
#[allow(unused_imports)]
use flare_core::Rgba8;
#[cfg(test)]
#[allow(unused_imports)]
use render::{MAX_RENDER_THREADS, PARALLEL_MIN_SHAPES, available_cores, rasterize, render_threads};

#[cfg(test)]
mod tests;
