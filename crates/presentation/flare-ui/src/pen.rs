//! The pen tool's path under construction.
//!
//! Click adds a corner point; press and drag adds a smooth point whose dragged handle
//! shapes the curve going forward (the incoming handle mirrors it). Clicking back on the first
//! point of a path with at least three points closes it. This type holds only the in-progress
//! geometry in document coordinates; the workspace decides when and how to commit it.

use egui::Pos2;
use flare_core::DocumentPoint;
use flare_document::PathSegment;

/// Most points one path may have, so a stuck key or a script cannot grow it forever.
pub const MAX_POINTS: usize = 10_000;
/// How close, in document units, a click must be to the first point to close the path.
pub const CLOSE_RADIUS: f32 = 8.0;
/// Trailing points closer than this to the one before are duplicates (a double-click).
const DUPLICATE_RADIUS: f32 = 1.5;
/// A drag shorter than this leaves a corner point rather than a tiny handle.
const MIN_HANDLE: f32 = 1.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PenPoint {
    pub anchor: Pos2,
    /// The handle leading to the next point. The incoming handle is its mirror image.
    pub out: Option<Pos2>,
}

impl PenPoint {
    fn incoming(&self) -> Pos2 {
        match self.out {
            Some(out) => self.anchor + (self.anchor - out),
            None => self.anchor,
        }
    }

    fn outgoing(&self) -> Pos2 {
        self.out.unwrap_or(self.anchor)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Press {
    /// A point was added (and may now be dragged to shape a handle).
    Added,
    /// The press landed on the first point: close the path.
    Close,
    /// Nothing happened (not finite, or the path is full).
    Ignored,
}

#[derive(Clone, Debug, Default)]
pub struct PenPath {
    points: Vec<PenPoint>,
    dragging: bool,
}

fn finite(p: Pos2) -> bool {
    p.x.is_finite() && p.y.is_finite()
}

fn point(p: Pos2) -> Option<DocumentPoint> {
    DocumentPoint::new(f64::from(p.x), f64::from(p.y)).ok()
}

impl PenPath {
    pub fn points(&self) -> &[PenPoint] {
        &self.points
    }

    pub fn is_active(&self) -> bool {
        !self.points.is_empty()
    }

    pub fn clear(&mut self) {
        self.points.clear();
        self.dragging = false;
    }

    pub fn press(&mut self, at: Pos2) -> Press {
        if !finite(at) {
            return Press::Ignored;
        }
        if self.points.len() >= 3 && (at - self.points[0].anchor).length() <= CLOSE_RADIUS {
            self.dragging = false;
            return Press::Close;
        }
        if self.points.len() >= MAX_POINTS {
            return Press::Ignored;
        }
        self.points.push(PenPoint {
            anchor: at,
            out: None,
        });
        self.dragging = true;
        Press::Added
    }

    /// While the button is held after a press, the drag position becomes the handle.
    pub fn drag(&mut self, to: Pos2) {
        if !self.dragging || !finite(to) {
            return;
        }
        if let Some(last) = self.points.last_mut() {
            last.out = ((to - last.anchor).length() >= MIN_HANDLE).then_some(to);
        }
    }

    pub fn release(&mut self) {
        self.dragging = false;
    }

    /// Removes the most recent point (Backspace).
    pub fn remove_last(&mut self) {
        self.points.pop();
        self.dragging = false;
    }

    /// Drops trailing duplicates left by a double-click.
    pub fn trim_duplicates(&mut self) {
        while self.points.len() >= 2 {
            let (a, b) = (
                self.points[self.points.len() - 2],
                self.points[self.points.len() - 1],
            );
            if (a.anchor - b.anchor).length() < DUPLICATE_RADIUS {
                self.points.pop();
            } else {
                break;
            }
        }
        self.dragging = false;
    }

    /// The path as drawing commands, or `None` when there are too few points (two for an open
    /// path, three for a closed one) or a coordinate is not finite.
    pub fn segments(&self, closed: bool) -> Option<Vec<PathSegment>> {
        let needed = if closed { 3 } else { 2 };
        if self.points.len() < needed {
            return None;
        }
        let mut out = vec![PathSegment::MoveTo(point(self.points[0].anchor)?)];
        let link = |from: &PenPoint, to: &PenPoint| -> Option<PathSegment> {
            if from.out.is_none() && to.out.is_none() {
                return Some(PathSegment::LineTo(point(to.anchor)?));
            }
            Some(PathSegment::CubicTo(
                point(from.outgoing())?,
                point(to.incoming())?,
                point(to.anchor)?,
            ))
        };
        for pair in self.points.windows(2) {
            out.push(link(&pair[0], &pair[1])?);
        }
        if closed {
            let (last, first) = (self.points[self.points.len() - 1], self.points[0]);
            // A straight closing edge is implied by Close; only a curved one needs a segment.
            if last.out.is_some() || first.out.is_some() {
                out.push(link(&last, &first)?);
            }
            out.push(PathSegment::Close);
        }
        Some(out)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    fn p(x: f32, y: f32) -> Pos2 {
        Pos2::new(x, y)
    }

    fn click(path: &mut PenPath, at: Pos2) -> Press {
        let outcome = path.press(at);
        path.release();
        outcome
    }

    fn xy(point: &DocumentPoint) -> (f64, f64) {
        (point.as_vec().x, point.as_vec().y)
    }

    #[test]
    fn clicks_make_a_polyline() {
        let mut path = PenPath::default();
        for at in [p(0.0, 0.0), p(50.0, 0.0), p(50.0, 40.0)] {
            assert_eq!(click(&mut path, at), Press::Added);
        }
        let segments = path.segments(false).expect("segments");
        assert_eq!(segments.len(), 3);
        assert!(matches!(segments[0], PathSegment::MoveTo(_)));
        assert!(
            segments[1..]
                .iter()
                .all(|s| matches!(s, PathSegment::LineTo(_)))
        );
    }

    #[test]
    fn dragging_a_point_sets_its_handle_and_the_incoming_side_mirrors_it() {
        let mut path = PenPath::default();
        assert_eq!(path.press(p(100.0, 100.0)), Press::Added);
        path.drag(p(140.0, 100.0));
        path.release();
        assert_eq!(path.points()[0].out, Some(p(140.0, 100.0)));
        // Second point is dragged too, so its incoming handle is the mirror of its outgoing one.
        path.press(p(300.0, 100.0));
        path.drag(p(340.0, 120.0));
        path.release();
        let segments = path.segments(false).expect("segments");
        let PathSegment::CubicTo(c1, c2, to) = &segments[1] else {
            panic!("a curve expected")
        };
        assert_eq!(xy(c1), (140.0, 100.0), "the first point's dragged handle");
        assert_eq!(
            xy(c2),
            (260.0, 80.0),
            "mirror of (340, 120) through (300, 100)"
        );
        assert_eq!(xy(to), (300.0, 100.0));
    }

    #[test]
    fn a_curve_needs_only_one_dragged_end() {
        let mut path = PenPath::default();
        path.press(p(0.0, 0.0));
        path.drag(p(0.0, 30.0));
        path.release();
        click(&mut path, p(100.0, 0.0));
        let segments = path.segments(false).expect("segments");
        let PathSegment::CubicTo(c1, c2, _) = &segments[1] else {
            panic!("a curve expected")
        };
        assert_eq!(xy(c1), (0.0, 30.0));
        assert_eq!(
            xy(c2),
            (100.0, 0.0),
            "no handle on the corner end: it sits on its anchor"
        );
    }

    #[test]
    fn tiny_drags_stay_corner_points() {
        let mut path = PenPath::default();
        path.press(p(10.0, 10.0));
        path.drag(p(10.4, 10.3));
        path.release();
        assert_eq!(path.points()[0].out, None);
        // Dragging without a press does nothing.
        path.drag(p(99.0, 99.0));
        assert_eq!(path.points()[0].out, None);
    }

    #[test]
    fn clicking_the_first_point_closes_only_with_three_or_more_points() {
        let mut path = PenPath::default();
        click(&mut path, p(0.0, 0.0));
        click(&mut path, p(60.0, 0.0));
        assert_eq!(
            path.press(p(2.0, 2.0)),
            Press::Added,
            "two points cannot close; this is a third point"
        );
        path.release();
        path.clear();
        for at in [p(0.0, 0.0), p(60.0, 0.0), p(60.0, 60.0)] {
            click(&mut path, at);
        }
        assert_eq!(path.press(p(3.0, 3.0)), Press::Close);
        assert_eq!(path.points().len(), 3, "closing adds no point");
        assert_eq!(
            path.press(p(30.0, 30.0)),
            Press::Added,
            "far from the start it is just another point"
        );
    }

    #[test]
    fn closed_paths_end_with_close_and_curved_closing_edges_are_explicit() {
        let mut path = PenPath::default();
        for at in [p(0.0, 0.0), p(60.0, 0.0), p(60.0, 60.0)] {
            click(&mut path, at);
        }
        let straight = path.segments(true).expect("closed");
        assert!(matches!(straight.last(), Some(PathSegment::Close)));
        assert_eq!(
            straight.len(),
            4,
            "a straight closing edge is implied by Close"
        );

        path.clear();
        path.press(p(0.0, 0.0));
        path.drag(p(0.0, -30.0));
        path.release();
        click(&mut path, p(60.0, 0.0));
        click(&mut path, p(60.0, 60.0));
        let curved = path.segments(true).expect("closed");
        assert!(
            matches!(curved[curved.len() - 2], PathSegment::CubicTo(..)),
            "the closing edge is a curve"
        );
        assert!(matches!(curved.last(), Some(PathSegment::Close)));
    }

    #[test]
    fn too_few_points_give_no_path() {
        let mut path = PenPath::default();
        assert!(path.segments(false).is_none());
        click(&mut path, p(0.0, 0.0));
        assert!(path.segments(false).is_none(), "one point is not a path");
        click(&mut path, p(5.0, 0.0));
        assert!(path.segments(false).is_some());
        assert!(path.segments(true).is_none(), "two points cannot be closed");
    }

    #[test]
    fn backspace_removes_points_and_a_double_click_duplicate_is_trimmed() {
        let mut path = PenPath::default();
        for at in [p(0.0, 0.0), p(40.0, 0.0), p(40.0, 40.0), p(40.5, 40.2)] {
            click(&mut path, at);
        }
        path.trim_duplicates();
        assert_eq!(
            path.points().len(),
            3,
            "the near-duplicate from the second click went"
        );
        path.remove_last();
        assert_eq!(path.points().len(), 2);
        path.remove_last();
        path.remove_last();
        path.remove_last(); // removing from empty is harmless
        assert!(!path.is_active());
    }

    #[test]
    fn bad_input_is_ignored_and_the_path_is_bounded() {
        let mut path = PenPath::default();
        assert_eq!(path.press(p(f32::NAN, 0.0)), Press::Ignored);
        assert_eq!(path.press(p(0.0, f32::INFINITY)), Press::Ignored);
        path.press(p(1.0, 1.0));
        path.drag(p(f32::NAN, 5.0));
        assert_eq!(
            path.points()[0].out,
            None,
            "a non-finite handle is not stored"
        );
        path.clear();
        for i in 0..MAX_POINTS {
            assert_eq!(click(&mut path, p(i as f32 * 10.0, 0.0)), Press::Added);
        }
        assert_eq!(
            path.press(p(-500.0, 500.0)),
            Press::Ignored,
            "the point limit holds"
        );
        assert_eq!(path.points().len(), MAX_POINTS);
    }
}
