//! The pen tool's path under construction.
//!
//! Click adds a corner point; press and drag adds a smooth point whose dragged handle
//! shapes the curve going forward (the incoming handle mirrors it). Clicking back on the first
//! point of a path with at least three points closes it. This type holds only the in-progress
//! geometry in document coordinates; the workspace decides when and how to commit it.

use emath::Pos2;
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
