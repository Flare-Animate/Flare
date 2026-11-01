//! The Lasso tool: the loop drawn by dragging, and the selection it makes.

use emath::Pos2;

use crate::common::SpanRef;

/// Most points one loop takes, so a long drag cannot grow it without end.
pub const MAX_POINTS: usize = 8192;

/// A point closer than this to the last one, in screen pixels, adds nothing the eye can see.
const MIN_STEP_PX: f32 = 2.0;

/// Adds `at` to the loop being dragged if it is far enough from the last point at this `scale`
/// (screen pixels per stage unit) and the loop has room. Returns whether it was added.
pub fn extend(points: &mut Vec<Pos2>, at: Pos2, scale: f32) -> bool {
    let step = MIN_STEP_PX / scale.max(f32::EPSILON);
    let far = points
        .last()
        .is_none_or(|last| (*last - at).length() >= step);
    if far && points.len() < MAX_POINTS {
        points.push(at);
        return true;
    }
    false
}

/// The selection after a loop: what it `found`, added to the `current` selection when `additive`
/// (Shift) and replacing it otherwise, without a span twice, the first one first.
pub fn selection_after(current: &[SpanRef], found: Vec<SpanRef>, additive: bool) -> Vec<SpanRef> {
    let mut all = if additive {
        current.to_vec()
    } else {
        Vec::new()
    };
    for span in found {
        if !all.contains(&span) {
            all.push(span);
        }
    }
    all
}
