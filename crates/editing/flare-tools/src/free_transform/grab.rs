//! What a press with the Free Transform tool takes hold of. The stage asks before the press, to
//! show the pointer for it, and at the press, to start the drag, so the two always agree.

use emath::{Pos2, Rect};

use super::handles::{self, Handle};

/// The part of the selection's frame under a point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Grab {
    /// A handle, which scales.
    Scale(Handle),
    /// An edge handle while the skew modifier (Alt) is held.
    Skew(Handle),
    /// Just outside a corner, which turns the selection about its center.
    Rotate,
    /// Inside the frame, which moves the selection, as in Animate: a thin line need not be hit
    /// on its ink to be dragged.
    Move,
}

/// What a press at `point` takes hold of on the frame `bounds`, with handles `radius` stage units
/// across; `skew` is whether the skew modifier is held. Handles come first, so one sitting
/// inside a small frame still scales. `None` off the frame.
pub fn grab_at(bounds: Rect, point: Pos2, radius: f32, skew: bool) -> Option<Grab> {
    if let Some(handle) = handles::pick(bounds, point, radius) {
        return Some(if skew && handle.is_edge() {
            Grab::Skew(handle)
        } else {
            Grab::Scale(handle)
        });
    }
    if handles::in_rotate_zone(bounds, point, radius) {
        return Some(Grab::Rotate);
    }
    (bounds.is_finite() && point.is_finite() && bounds.contains(point)).then_some(Grab::Move)
}

/// Which way the edge or corner a grab drags runs, for choosing the pointer: a corner resizes
/// along a diagonal, an edge across itself, and a skew along itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GrabAxis {
    /// Left and right.
    Horizontal,
    /// Up and down.
    Vertical,
    /// Top-left to bottom-right.
    Falling,
    /// Bottom-left to top-right.
    Rising,
}

impl Grab {
    /// The direction the pointer drags in, for a scale or a skew.
    pub fn axis(self) -> Option<GrabAxis> {
        match self {
            Grab::Scale(handle) => Some(match handle {
                Handle::TopLeft | Handle::BottomRight => GrabAxis::Falling,
                Handle::TopRight | Handle::BottomLeft => GrabAxis::Rising,
                Handle::Left | Handle::Right => GrabAxis::Horizontal,
                Handle::Top | Handle::Bottom => GrabAxis::Vertical,
            }),
            // A skew slides an edge along itself.
            Grab::Skew(Handle::Top | Handle::Bottom) => Some(GrabAxis::Horizontal),
            Grab::Skew(_) => Some(GrabAxis::Vertical),
            Grab::Rotate | Grab::Move => None,
        }
    }
}
