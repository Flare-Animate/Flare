//! The Line tool: where a dragged line ends when its direction is constrained.

use emath::{Pos2, Vec2};

#[cfg(test)]
mod tests;

/// The eight directions a constrained line may take, a step of 45 degrees apart, starting along
/// +x. A table and not a sine and cosine, so a horizontal line ends exactly on its start's y, a
/// vertical one on its x, and a diagonal moves by the same amount on both axes.
const DIRECTIONS: [Vec2; 8] = {
    let d = std::f32::consts::FRAC_1_SQRT_2;
    [
        Vec2::new(1.0, 0.0),
        Vec2::new(d, d),
        Vec2::new(0.0, 1.0),
        Vec2::new(-d, d),
        Vec2::new(-1.0, 0.0),
        Vec2::new(-d, -d),
        Vec2::new(0.0, -1.0),
        Vec2::new(d, -d),
    ]
};

/// Where a line from `from` ends when the pointer is at `to` and the line is held to the nearest
/// of the eight directions 45 degrees apart: the point of that direction closest to the pointer,
/// so the end follows the pointer as nearly as the direction allows. A pointer still on `from`,
/// or a point that is not finite, gives `to` back unchanged.
pub fn constrain(from: Pos2, to: Pos2) -> Pos2 {
    let drag = to - from;
    if !from.is_finite() || !to.is_finite() || drag == Vec2::ZERO {
        return to;
    }
    let step = std::f32::consts::FRAC_PI_4;
    // `rem_euclid` keeps the index in 0..8 for the negative half of the circle; `get` and not an
    // index, so a rounding surprise cannot panic.
    let index = (drag.angle() / step).round().rem_euclid(8.0) as usize;
    let Some(direction) = DIRECTIONS.get(index) else {
        return to;
    };
    from + *direction * drag.dot(*direction)
}
