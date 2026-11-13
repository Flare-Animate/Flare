//! What helps frames be counted at a glance, as Animate's timeline has it: every fifth frame is
//! shaded, and the ruler numbers the first frame and then every fifth (or tenth, twentieth and so
//! on when the frames are too narrow for that many numbers).

/// The least room, in points, between the starts of two frame numbers on the ruler: five digits
/// of its small type and a little space.
const MIN_LABEL_GAP: f32 = 34.0;

/// Whether frame `index` (counting from 0) is the 5th, 10th, 15th and so on, counting from 1:
/// the frames the timeline shades.
pub fn is_fifth(index: u32) -> bool {
    index % 5 == 4
}

/// How many frames apart the ruler's numbers are when a frame is `per_frame` points wide: the
/// least of 5, 10, 20, 50, 100, 200, 500, ... that keeps the numbers from running together.
pub fn ruler_step(per_frame: f32) -> u32 {
    if !(per_frame.is_finite() && per_frame > 0.0) {
        return 5;
    }
    let mut step: u32 = 5;
    let mut rung = 0;
    while (step as f32) * per_frame < MIN_LABEL_GAP {
        // 5, 10, 20, 50, 100, 200, ...: times 2, then 2, then 2.5.
        let next = match rung % 3 {
            0 | 1 => step.checked_mul(2),
            _ => step.checked_mul(5).map(|s| s / 2),
        };
        match next {
            Some(next) => step = next,
            None => break,
        }
        rung += 1;
    }
    step
}

/// Whether the ruler numbers frame `index` (counting from 0) when its numbers are `step` apart:
/// the first frame, and every frame whose number, counting from 1, is a multiple of `step`.
pub fn is_numbered(index: u32, step: u32) -> bool {
    index == 0 || (step > 0 && index % step == step - 1)
}

#[cfg(test)]
#[path = "counting_tests.rs"]
mod tests;
