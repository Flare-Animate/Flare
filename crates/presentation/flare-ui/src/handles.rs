//! Scale handles on the bounds of the selection.
//!
//! Everything here is geometry in stage coordinates: where the eight handles sit, which one a
//! point is on, and what scale a drag of a handle means. The workspace turns that into an edit.

use egui::{Pos2, Rect};
use glamx::{DAffine2, DVec2};

/// Smallest magnitude of a scale factor, so a drag cannot collapse the artwork to nothing.
const MIN_SCALE: f64 = 0.01;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    TopLeft,
    Top,
    TopRight,
    Right,
    BottomRight,
    Bottom,
    BottomLeft,
    Left,
}

impl Handle {
    pub const ALL: [Handle; 8] = [
        Handle::TopLeft,
        Handle::Top,
        Handle::TopRight,
        Handle::Right,
        Handle::BottomRight,
        Handle::Bottom,
        Handle::BottomLeft,
        Handle::Left,
    ];

    /// Which sides this handle moves: (-1, 0, +1) for left/none/right and top/none/bottom.
    fn direction(self) -> (i8, i8) {
        match self {
            Handle::TopLeft => (-1, -1),
            Handle::Top => (0, -1),
            Handle::TopRight => (1, -1),
            Handle::Right => (1, 0),
            Handle::BottomRight => (1, 1),
            Handle::Bottom => (0, 1),
            Handle::BottomLeft => (-1, 1),
            Handle::Left => (-1, 0),
        }
    }

    fn opposite(self) -> Handle {
        let (dx, dy) = self.direction();
        Handle::ALL
            .into_iter()
            .find(|h| h.direction() == (-dx, -dy))
            .unwrap_or(self)
    }

    /// Where the handle sits on `bounds`.
    pub fn position(self, bounds: Rect) -> Pos2 {
        let (dx, dy) = self.direction();
        let x = match dx {
            -1 => bounds.min.x,
            1 => bounds.max.x,
            _ => bounds.center().x,
        };
        let y = match dy {
            -1 => bounds.min.y,
            1 => bounds.max.y,
            _ => bounds.center().y,
        };
        Pos2::new(x, y)
    }

    /// The point that stays put while this handle is dragged: the opposite handle.
    pub fn anchor(self, bounds: Rect) -> Pos2 {
        self.opposite().position(bounds)
    }
}

/// The handle within `radius` of `point`, nearest first. Corners win ties with edges because
/// they are listed first among equals.
pub fn pick(bounds: Rect, point: Pos2, radius: f32) -> Option<Handle> {
    if !bounds.min.is_finite() || !bounds.max.is_finite() || !point.is_finite() {
        return None;
    }
    Handle::ALL
        .into_iter()
        .map(|h| (h, (h.position(bounds) - point).length()))
        .filter(|(_, d)| *d <= radius)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(h, _)| h)
}

fn clamp_scale(s: f64) -> f64 {
    if s.abs() < MIN_SCALE {
        MIN_SCALE.copysign(if s == 0.0 { 1.0 } else { s })
    } else {
        s
    }
}

/// The (x, y) scale that moves `handle` to `pointer` while its opposite handle stays put.
/// Edge handles scale one axis. With `uniform`, corners scale both axes by the larger change.
/// An axis along which the bounds have no extent is left alone. `None` for non-finite input.
pub fn scale_for(bounds: Rect, handle: Handle, pointer: Pos2, uniform: bool) -> Option<(f64, f64)> {
    if !pointer.is_finite() || !bounds.min.is_finite() || !bounds.max.is_finite() {
        return None;
    }
    let (dx, dy) = handle.direction();
    let from = handle.position(bounds);
    let anchor = handle.anchor(bounds);
    let axis = |active: bool, now: f32, was: f32, fixed: f32| -> f64 {
        let span = f64::from(was - fixed);
        if !active || span.abs() < 1e-6 {
            return 1.0;
        }
        clamp_scale(f64::from(now - fixed) / span)
    };
    let mut sx = axis(dx != 0, pointer.x, from.x, anchor.x);
    let mut sy = axis(dy != 0, pointer.y, from.y, anchor.y);
    if uniform && dx != 0 && dy != 0 {
        let s = if (sx - 1.0).abs() >= (sy - 1.0).abs() {
            sx
        } else {
            sy
        };
        sx = s;
        sy = s;
    }
    Some((sx, sy))
}

/// The matrix that applies the scale about the handle's anchor.
pub fn matrix(bounds: Rect, handle: Handle, scale: (f64, f64)) -> DAffine2 {
    let a = handle.anchor(bounds);
    let anchor = DVec2::new(f64::from(a.x), f64::from(a.y));
    DAffine2::from_translation(anchor)
        * DAffine2::from_scale(DVec2::new(scale.0, scale.1))
        * DAffine2::from_translation(-anchor)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    fn bounds() -> Rect {
        Rect::from_min_max(Pos2::new(10.0, 20.0), Pos2::new(110.0, 70.0))
    }

    #[test]
    fn handles_sit_on_corners_and_edge_midpoints_and_their_anchors_are_opposite() {
        assert_eq!(Handle::TopLeft.position(bounds()), Pos2::new(10.0, 20.0));
        assert_eq!(Handle::Right.position(bounds()), Pos2::new(110.0, 45.0));
        assert_eq!(Handle::Top.position(bounds()), Pos2::new(60.0, 20.0));
        for h in Handle::ALL {
            assert_ne!(h.anchor(bounds()), h.position(bounds()));
            assert_eq!(h.opposite().opposite(), h);
        }
        assert_eq!(Handle::BottomRight.anchor(bounds()), Pos2::new(10.0, 20.0));
        assert_eq!(Handle::Left.anchor(bounds()), Pos2::new(110.0, 45.0));
    }

    #[test]
    fn pick_finds_the_nearest_handle_within_the_radius() {
        assert_eq!(
            pick(bounds(), Pos2::new(111.0, 69.0), 4.0),
            Some(Handle::BottomRight)
        );
        assert_eq!(
            pick(bounds(), Pos2::new(60.0, 21.0), 4.0),
            Some(Handle::Top)
        );
        assert_eq!(
            pick(bounds(), Pos2::new(60.0, 45.0), 4.0),
            None,
            "the middle is not a handle"
        );
        assert_eq!(pick(bounds(), Pos2::new(f32::NAN, 0.0), 4.0), None);
    }

    #[test]
    fn dragging_a_corner_scales_about_the_opposite_corner() {
        // Bottom-right from (110, 70) to (210, 120): width 100 -> 200, height 50 -> 100.
        let s = scale_for(
            bounds(),
            Handle::BottomRight,
            Pos2::new(210.0, 120.0),
            false,
        )
        .expect("scale");
        assert_eq!(s, (2.0, 2.0));
        let m = matrix(bounds(), Handle::BottomRight, s);
        assert_eq!(
            m.transform_point2(DVec2::new(10.0, 20.0)),
            DVec2::new(10.0, 20.0),
            "anchor fixed"
        );
        assert_eq!(
            m.transform_point2(DVec2::new(110.0, 70.0)),
            DVec2::new(210.0, 120.0)
        );
    }

    #[test]
    fn edges_scale_one_axis_and_uniform_follows_the_larger_change() {
        let s = scale_for(bounds(), Handle::Right, Pos2::new(60.0, 999.0), false).expect("scale");
        assert_eq!(s, (0.5, 1.0), "only x changes, whatever y is");
        let s =
            scale_for(bounds(), Handle::BottomRight, Pos2::new(210.0, 80.0), true).expect("scale");
        assert_eq!(s, (2.0, 2.0), "x changed more, so both follow x");
        let s = scale_for(bounds(), Handle::TopLeft, Pos2::new(10.0, -30.0), true).expect("scale");
        assert_eq!(s, (2.0, 2.0), "from the top-left, y changed more");
    }

    #[test]
    fn dragging_past_the_anchor_flips_and_never_collapses() {
        let s = scale_for(bounds(), Handle::Right, Pos2::new(-90.0, 0.0), false).expect("scale");
        assert_eq!(s.0, -1.0, "past the left edge mirrors it");
        let s = scale_for(bounds(), Handle::Right, Pos2::new(10.0, 0.0), false).expect("scale");
        assert_eq!(s.0, 0.01, "exactly on the anchor stays a sliver");
        assert!(
            scale_for(
                bounds(),
                Handle::Right,
                Pos2::new(f32::INFINITY, 0.0),
                false
            )
            .is_none()
        );
    }

    #[test]
    fn a_flat_axis_is_left_alone() {
        let flat = Rect::from_min_max(Pos2::new(0.0, 5.0), Pos2::new(50.0, 5.0));
        let s = scale_for(flat, Handle::BottomRight, Pos2::new(100.0, 40.0), false).expect("scale");
        assert_eq!(s, (2.0, 1.0));
    }
}
