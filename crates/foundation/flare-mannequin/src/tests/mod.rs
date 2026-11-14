#![allow(clippy::expect_used, clippy::unwrap_used)]

mod drawing;
mod figure;
mod grab;
mod math;
mod posing;
mod request;
mod rings;
mod shading;
mod surface;
mod wire;

use crate::{Placement, View};

/// A flat front view, so lengths in the picture are lengths of the figure.
pub(super) fn front() -> View {
    View::new(0.0, 0.0, 0.0)
}

/// A picture 100 units per figure unit, the figure's middle at (500, 500).
pub(super) fn placement() -> Placement {
    Placement::new([500.0, 500.0], 100.0).expect("placement")
}

pub(super) fn close(a: f64, b: f64, tolerance: f64) -> bool {
    (a - b).abs() <= tolerance
}
