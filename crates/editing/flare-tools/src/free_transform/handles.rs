//! Scale handles on the bounds of the selection.
//!
//! Everything here is geometry in stage coordinates: where the eight handles sit, which one a
//! point is on, and what scale a drag of a handle means. The workspace turns that into an edit.

use emath::{Pos2, Rect};
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

    /// Whether this is an edge handle (the only kind that skews).
    pub fn is_edge(self) -> bool {
        let (dx, dy) = self.direction();
        dx == 0 || dy == 0
    }

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

/// How far outside a corner handle, in multiples of its pick radius, the rotate zone reaches.
const ROTATE_ZONE: f32 = 3.0;
/// Largest skew, as a shear factor (about 80 degrees), so a drag cannot flatten the artwork.
const MAX_SHEAR: f64 = 5.67;
/// Rotation snaps to multiples of this many degrees when Shift is held.
const SNAP_DEGREES: f64 = 15.0;

/// Whether `point` is in the rotate zone: just outside a corner of `bounds`, beyond the scale
/// handle's own reach but within a few radii of it.
pub fn in_rotate_zone(bounds: Rect, point: Pos2, radius: f32) -> bool {
    if !bounds.min.is_finite() || !bounds.max.is_finite() || !point.is_finite() || radius <= 0.0 {
        return false;
    }
    if bounds.contains(point) {
        return false;
    }
    [
        Handle::TopLeft,
        Handle::TopRight,
        Handle::BottomRight,
        Handle::BottomLeft,
    ]
    .into_iter()
    .any(|corner| {
        let distance = (corner.position(bounds) - point).length();
        distance > radius && distance <= radius * ROTATE_ZONE
    })
}

/// The angle, in radians and in the stage's y-down orientation (positive is clockwise on
/// screen), that carries the direction from `center` to `from` onto the direction to `to`.
/// `snap` rounds it to a multiple of 15 degrees. `None` when either point sits on the center
/// (no direction) or is not finite.
pub fn rotation_between(center: Pos2, from: Pos2, to: Pos2, snap: bool) -> Option<f64> {
    if ![center, from, to].iter().all(|p| p.is_finite()) {
        return None;
    }
    let (a, b) = (from - center, to - center);
    if a.length() < 1e-3 || b.length() < 1e-3 {
        return None;
    }
    let mut angle = f64::from(b.y).atan2(f64::from(b.x)) - f64::from(a.y).atan2(f64::from(a.x));
    let tau = std::f64::consts::TAU;
    angle = (angle + std::f64::consts::PI).rem_euclid(tau) - std::f64::consts::PI;
    if snap {
        let step = SNAP_DEGREES.to_radians();
        angle = (angle / step).round() * step;
    }
    Some(angle)
}

/// The rotation about `center` by `angle`.
pub fn rotation_matrix(center: Pos2, angle: f64) -> DAffine2 {
    let c = DVec2::new(f64::from(center.x), f64::from(center.y));
    DAffine2::from_translation(c) * DAffine2::from_angle(angle) * DAffine2::from_translation(-c)
}

/// The shear factor for dragging an edge handle sideways: how far the handle's edge moves
/// along the edge, per unit of distance from the opposite edge, which stays put. Only edge
/// handles skew; `None` for corners, a flat box, or non-finite input.
pub fn skew_for(bounds: Rect, handle: Handle, pointer: Pos2) -> Option<f64> {
    if !pointer.is_finite() || !bounds.min.is_finite() || !bounds.max.is_finite() {
        return None;
    }
    let (dx, dy) = handle.direction();
    let from = handle.position(bounds);
    let anchor = handle.anchor(bounds);
    let factor = match (dx, dy) {
        // Top and bottom edges slide along x.
        (0, _) => {
            let span = f64::from(from.y - anchor.y);
            (span.abs() > 1e-6).then(|| f64::from(pointer.x - from.x) / span)?
        }
        // Left and right edges slide along y.
        (_, 0) => {
            let span = f64::from(from.x - anchor.x);
            (span.abs() > 1e-6).then(|| f64::from(pointer.y - from.y) / span)?
        }
        _ => return None,
    };
    Some(factor.clamp(-MAX_SHEAR, MAX_SHEAR))
}

/// The shear that moves the dragged edge by factor `k` while the opposite edge stays put.
pub fn skew_matrix(bounds: Rect, handle: Handle, k: f64) -> DAffine2 {
    let a = handle.anchor(bounds);
    let anchor = DVec2::new(f64::from(a.x), f64::from(a.y));
    let (dx, _) = handle.direction();
    // x' = x + k (y - anchor.y) for top/bottom; y' = y + k (x - anchor.x) for left/right.
    let shear = if dx == 0 {
        DAffine2::from_cols_array(&[1.0, 0.0, k, 1.0, 0.0, 0.0])
    } else {
        DAffine2::from_cols_array(&[1.0, k, 0.0, 1.0, 0.0, 0.0])
    };
    DAffine2::from_translation(anchor) * shear * DAffine2::from_translation(-anchor)
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

    #[test]
    fn the_rotate_zone_is_a_ring_around_each_corner_outside_the_box() {
        let b = bounds(); // (10,20)-(110,70)
        assert!(
            in_rotate_zone(b, Pos2::new(120.0, 80.0), 6.0),
            "diagonally outside a corner"
        );
        assert!(in_rotate_zone(b, Pos2::new(0.0, 12.0), 6.0));
        assert!(
            !in_rotate_zone(b, Pos2::new(111.0, 71.0), 6.0),
            "that is the scale handle"
        );
        assert!(
            !in_rotate_zone(b, Pos2::new(60.0, 45.0), 6.0),
            "inside the box"
        );
        assert!(
            !in_rotate_zone(b, Pos2::new(300.0, 300.0), 6.0),
            "too far away"
        );
        assert!(!in_rotate_zone(b, Pos2::new(f32::NAN, 0.0), 6.0));
        assert!(!in_rotate_zone(b, Pos2::new(120.0, 80.0), 0.0));
    }

    #[test]
    fn rotation_is_the_angle_between_the_two_directions_and_snaps_on_request() {
        let c = Pos2::new(0.0, 0.0);
        let quarter =
            rotation_between(c, Pos2::new(10.0, 0.0), Pos2::new(0.0, 10.0), false).expect("angle");
        assert!(
            (quarter - std::f64::consts::FRAC_PI_2).abs() < 1e-9,
            "clockwise on screen"
        );
        let back =
            rotation_between(c, Pos2::new(0.0, 10.0), Pos2::new(10.0, 0.0), false).expect("angle");
        assert!((back + std::f64::consts::FRAC_PI_2).abs() < 1e-9);
        // Across the +-180 degree seam the short way round is taken.
        let seam = rotation_between(c, Pos2::new(-10.0, -1.0), Pos2::new(-10.0, 1.0), false)
            .expect("angle");
        assert!(seam.abs() < 0.3, "{seam}");
        let near =
            rotation_between(c, Pos2::new(10.0, 0.0), Pos2::new(10.0, 3.0), true).expect("angle");
        assert!(
            (near - 15.0_f64.to_radians()).abs() < 1e-9,
            "16.7 degrees snaps to 15: {near}"
        );
    }

    #[test]
    fn rotation_needs_two_real_directions() {
        let c = Pos2::new(5.0, 5.0);
        assert!(rotation_between(c, c, Pos2::new(9.0, 5.0), false).is_none());
        assert!(rotation_between(c, Pos2::new(9.0, 5.0), c, false).is_none());
        assert!(
            rotation_between(c, Pos2::new(f32::NAN, 1.0), Pos2::new(9.0, 5.0), false).is_none()
        );
    }

    #[test]
    fn rotating_about_the_center_keeps_it_fixed() {
        let m = rotation_matrix(Pos2::new(60.0, 45.0), std::f64::consts::FRAC_PI_2);
        let moved = m.transform_point2(DVec2::new(60.0, 45.0));
        assert!((moved - DVec2::new(60.0, 45.0)).length() < 1e-9);
        let corner = m.transform_point2(DVec2::new(110.0, 45.0));
        assert!(
            (corner - DVec2::new(60.0, 95.0)).length() < 1e-9,
            "{corner:?}"
        );
    }

    #[test]
    fn skewing_an_edge_slides_it_along_itself_and_holds_the_opposite_edge() {
        let b = bounds(); // 100 wide, 50 tall
        // Drag the top edge's handle (60, 20) 25 to the right.
        let k = skew_for(b, Handle::Top, Pos2::new(85.0, 20.0)).expect("shear");
        assert!((k - (25.0 / -50.0)).abs() < 1e-9, "{k}");
        let m = skew_matrix(b, Handle::Top, k);
        let top = m.transform_point2(DVec2::new(60.0, 20.0));
        assert!(
            (top - DVec2::new(85.0, 20.0)).length() < 1e-9,
            "the handle follows the pointer"
        );
        let bottom = m.transform_point2(DVec2::new(60.0, 70.0));
        assert!(
            (bottom - DVec2::new(60.0, 70.0)).length() < 1e-9,
            "the opposite edge is fixed"
        );
        // The right edge slides along y.
        let k = skew_for(b, Handle::Right, Pos2::new(110.0, 95.0)).expect("shear");
        let m = skew_matrix(b, Handle::Right, k);
        let right = m.transform_point2(DVec2::new(110.0, 45.0));
        assert!(
            (right - DVec2::new(110.0, 95.0)).length() < 1e-9,
            "{right:?}"
        );
    }

    #[test]
    fn corners_and_flat_boxes_do_not_skew_and_the_shear_is_bounded() {
        let b = bounds();
        assert!(skew_for(b, Handle::TopLeft, Pos2::new(0.0, 0.0)).is_none());
        let flat = Rect::from_min_max(Pos2::new(0.0, 5.0), Pos2::new(50.0, 5.0));
        assert!(skew_for(flat, Handle::Top, Pos2::new(9.0, 5.0)).is_none());
        let huge = skew_for(b, Handle::Top, Pos2::new(1e9, 20.0)).expect("shear");
        assert_eq!(huge.abs(), MAX_SHEAR);
        assert!(skew_for(b, Handle::Top, Pos2::new(f32::NAN, 0.0)).is_none());
    }
}
