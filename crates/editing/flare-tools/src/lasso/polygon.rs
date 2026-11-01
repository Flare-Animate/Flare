//! The Polygon Lasso tool: click the corners of a loop, close it, and what it encloses is selected.

use emath::Pos2;

/// How far from the first corner, in screen pixels, a click closes the loop.
pub const CLOSE_RADIUS_PX: f32 = 8.0;

/// The most corners one loop takes; clicks past it are ignored.
pub const MAX_CORNERS: usize = 4096;

/// What a click did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Click {
    /// It added a corner.
    Corner,
    /// It landed on the first corner of a loop that already encloses an area: close it.
    Close,
    /// It changed nothing: the repeat of the click before (the second of a double click), or past
    /// the limit.
    Ignored,
}

/// A click at `at`, with `scale` screen pixels to a stage unit, on the `corners` so far.
pub fn click(corners: &mut Vec<Pos2>, at: Pos2, scale: f32) -> Click {
    let radius = CLOSE_RADIUS_PX / scale.max(f32::EPSILON);
    if corners.len() >= 3
        && corners
            .first()
            .is_some_and(|f| (*f - at).length() <= radius)
    {
        return Click::Close;
    }
    let repeated = corners
        .last()
        .is_some_and(|last| (*last - at).length() <= radius);
    if repeated || corners.len() >= MAX_CORNERS {
        return Click::Ignored;
    }
    corners.push(at);
    Click::Corner
}

/// Whether the corners so far can be closed (Enter or a double click). Fewer than three enclose
/// nothing, so they are dropped, and the selection stays as it was.
pub fn can_close(corners: &mut Vec<Pos2>) -> bool {
    if corners.len() < 3 {
        corners.clear();
        return false;
    }
    true
}
