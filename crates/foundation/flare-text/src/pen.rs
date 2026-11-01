//! Reading a glyph's outline from a font into path segments.

use flare_core::DocumentPoint;
use flare_document::PathSegment;
use skrifa::outline::OutlinePen;

/// Collects a glyph's outline as path segments, moved to where the glyph sits. Font outlines
/// are y-up with the baseline at zero; the stage is y-down, so y is flipped.
pub(crate) struct Pen<'a> {
    pub(crate) out: &'a mut Vec<PathSegment>,
    pub(crate) at: (f32, f32),
    pub(crate) bad: bool,
}

impl Pen<'_> {
    fn point(&mut self, x: f32, y: f32) -> Option<DocumentPoint> {
        let p = DocumentPoint::new(f64::from(self.at.0 + x), f64::from(self.at.1 - y)).ok();
        if p.is_none() {
            self.bad = true;
        }
        p
    }
}

impl OutlinePen for Pen<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        if let Some(p) = self.point(x, y) {
            self.out.push(PathSegment::MoveTo(p));
        }
    }

    fn line_to(&mut self, x: f32, y: f32) {
        if let Some(p) = self.point(x, y) {
            self.out.push(PathSegment::LineTo(p));
        }
    }

    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        // A shape's curves are cubic. The control points of the equal cubic sit two thirds of
        // the way from each end to the quadratic's control point.
        let Some(
            &PathSegment::MoveTo(from)
            | &PathSegment::LineTo(from)
            | &PathSegment::CubicTo(_, _, from),
        ) = self.out.last()
        else {
            return;
        };
        let (Some(c), Some(to)) = (self.point(cx, cy), self.point(x, y)) else {
            return;
        };
        let (f, c, t) = (from.as_vec(), c.as_vec(), to.as_vec());
        let one = f + (c - f) * (2.0 / 3.0);
        let two = t + (c - t) * (2.0 / 3.0);
        if let (Ok(a), Ok(b)) = (
            DocumentPoint::new(one.x, one.y),
            DocumentPoint::new(two.x, two.y),
        ) {
            self.out.push(PathSegment::CubicTo(a, b, to));
        }
    }

    fn curve_to(&mut self, ax: f32, ay: f32, bx: f32, by: f32, x: f32, y: f32) {
        if let (Some(a), Some(b), Some(to)) =
            (self.point(ax, ay), self.point(bx, by), self.point(x, y))
        {
            self.out.push(PathSegment::CubicTo(a, b, to));
        }
    }

    fn close(&mut self) {
        self.out.push(PathSegment::Close);
    }
}
