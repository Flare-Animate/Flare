use flare_core::{DocumentPoint, Rgba8, ShapeId};
use thiserror::Error;

/// Upper bound on segments per shape; imports and edits cannot exceed it.
pub const MAX_SEGMENTS: usize = 1_000_000;

/// One drawing command in document space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PathSegment {
    MoveTo(DocumentPoint),
    LineTo(DocumentPoint),
    CubicTo(DocumentPoint, DocumentPoint, DocumentPoint),
    Close,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ShapeError {
    #[error("a shape needs at least one segment")]
    Empty,
    #[error("a shape path must start with MoveTo")]
    MustStartWithMove,
    #[error("a shape cannot have more than {MAX_SEGMENTS} segments")]
    TooManySegments,
    #[error("drawing commands other than MoveTo need a current point")]
    NoCurrentPoint,
    #[error("stroke width must be finite and between 0 (exclusive) and {MAX_STROKE_WIDTH}")]
    InvalidStrokeWidth,
}

/// Upper bound on stroke width in document units.
pub const MAX_STROKE_WIDTH: f64 = 10_000.0;

/// An outline drawn along a shape's path with round caps and joins.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stroke {
    width: f64,
    color: Rgba8,
}

impl Stroke {
    pub fn new(width: f64, color: Rgba8) -> Result<Self, ShapeError> {
        if !(width.is_finite() && width > 0.0 && width <= MAX_STROKE_WIDTH) {
            return Err(ShapeError::InvalidStrokeWidth);
        }
        Ok(Self { width, color })
    }

    pub fn width(&self) -> f64 {
        self.width
    }

    pub fn color(&self) -> Rgba8 {
        self.color
    }
}

/// A two-color linear gradient between two points in the shape's own coordinates. Colors
/// are straight-alpha and interpolated in sRGB; outside the two points the end colors hold.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Gradient {
    from: DocumentPoint,
    to: DocumentPoint,
    start: Rgba8,
    end: Rgba8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum GradientError {
    #[error("a gradient needs two different points")]
    SamePoint,
}

impl Gradient {
    pub fn linear(
        from: DocumentPoint,
        to: DocumentPoint,
        start: Rgba8,
        end: Rgba8,
    ) -> Result<Self, GradientError> {
        if from == to {
            return Err(GradientError::SamePoint);
        }
        Ok(Self {
            from,
            to,
            start,
            end,
        })
    }

    pub fn from(&self) -> DocumentPoint {
        self.from
    }

    pub fn to(&self) -> DocumentPoint {
        self.to
    }

    pub fn start(&self) -> Rgba8 {
        self.start
    }

    pub fn end(&self) -> Rgba8 {
        self.end
    }

    /// Whether both colors are fully transparent, so nothing would be drawn.
    pub fn is_invisible(&self) -> bool {
        self.start.a == 0 && self.end.a == 0
    }
}

/// A filled vector shape. Points are already validated finite by `DocumentPoint`.
#[derive(Debug, Clone, PartialEq)]
pub struct Shape {
    id: ShapeId,
    path: Vec<PathSegment>,
    fill: Rgba8,
    /// When present it is painted instead of the solid `fill`.
    gradient: Option<Gradient>,
    stroke: Option<Stroke>,
}

impl Shape {
    pub fn new(id: ShapeId, path: Vec<PathSegment>, fill: Rgba8) -> Result<Self, ShapeError> {
        if path.is_empty() {
            return Err(ShapeError::Empty);
        }
        if path.len() > MAX_SEGMENTS {
            return Err(ShapeError::TooManySegments);
        }
        if !matches!(path[0], PathSegment::MoveTo(_)) {
            return Err(ShapeError::MustStartWithMove);
        }
        // After Close the current point is gone until the next MoveTo.
        let mut has_current = false;
        for segment in &path {
            match segment {
                PathSegment::MoveTo(_) => has_current = true,
                PathSegment::Close => has_current = false,
                PathSegment::LineTo(_) | PathSegment::CubicTo(..) if !has_current => {
                    return Err(ShapeError::NoCurrentPoint);
                }
                _ => {}
            }
        }
        Ok(Self {
            id,
            path,
            fill,
            gradient: None,
            stroke: None,
        })
    }

    /// Adds an outline. A transparent fill with a stroke gives an outline-only shape.
    #[must_use]
    pub fn with_stroke(mut self, stroke: Stroke) -> Self {
        self.stroke = Some(stroke);
        self
    }

    /// Carries `other`'s stroke and gradient over to this shape (a rebuilt or copied one).
    #[must_use]
    pub fn painted_like(mut self, other: &Shape) -> Self {
        self.stroke = other.stroke;
        self.gradient = other.gradient;
        self
    }

    /// Paints the fill with `gradient`; the solid fill color is kept as the fallback.
    #[must_use]
    pub fn with_gradient(mut self, gradient: Gradient) -> Self {
        self.gradient = Some(gradient);
        self
    }

    /// Axis-aligned rectangle, the simplest closed shape.
    pub fn rect(id: ShapeId, x: f64, y: f64, w: f64, h: f64, fill: Rgba8) -> Option<Self> {
        let point = |x, y| DocumentPoint::new(x, y).ok();
        let path = vec![
            PathSegment::MoveTo(point(x, y)?),
            PathSegment::LineTo(point(x + w, y)?),
            PathSegment::LineTo(point(x + w, y + h)?),
            PathSegment::LineTo(point(x, y + h)?),
            PathSegment::Close,
        ];
        Self::new(id, path, fill).ok()
    }

    /// Ellipse centered at (`cx`, `cy`) with radii `rx` and `ry`, as four cubic arcs.
    pub fn ellipse(id: ShapeId, cx: f64, cy: f64, rx: f64, ry: f64, fill: Rgba8) -> Option<Self> {
        // Control-point distance that makes four cubics approximate a circle closely.
        const K: f64 = 0.552_284_749_830_793_4;
        let p = |x, y| DocumentPoint::new(x, y).ok();
        let (kx, ky) = (K * rx, K * ry);
        let path = vec![
            PathSegment::MoveTo(p(cx + rx, cy)?),
            PathSegment::CubicTo(p(cx + rx, cy + ky)?, p(cx + kx, cy + ry)?, p(cx, cy + ry)?),
            PathSegment::CubicTo(p(cx - kx, cy + ry)?, p(cx - rx, cy + ky)?, p(cx - rx, cy)?),
            PathSegment::CubicTo(p(cx - rx, cy - ky)?, p(cx - kx, cy - ry)?, p(cx, cy - ry)?),
            PathSegment::CubicTo(p(cx + kx, cy - ry)?, p(cx + rx, cy - ky)?, p(cx + rx, cy)?),
            PathSegment::Close,
        ];
        Self::new(id, path, fill).ok()
    }

    /// An open two-point line. It has no fill, so the stroke is what you see.
    pub fn line(id: ShapeId, from: (f64, f64), to: (f64, f64), stroke: Stroke) -> Option<Self> {
        let p = |(x, y): (f64, f64)| DocumentPoint::new(x, y).ok();
        let path = vec![PathSegment::MoveTo(p(from)?), PathSegment::LineTo(p(to)?)];
        Some(
            Self::new(id, path, Rgba8::TRANSPARENT)
                .ok()?
                .with_stroke(stroke),
        )
    }

    pub fn id(&self) -> ShapeId {
        self.id
    }

    pub fn path(&self) -> &[PathSegment] {
        &self.path
    }

    pub fn fill(&self) -> Rgba8 {
        self.fill
    }

    pub fn stroke(&self) -> Option<Stroke> {
        self.stroke
    }

    pub fn gradient(&self) -> Option<Gradient> {
        self.gradient
    }

    /// Whether the fill paints nothing (a transparent solid fill and no visible gradient).
    pub fn fill_is_invisible(&self) -> bool {
        match self.gradient {
            Some(gradient) => gradient.is_invisible(),
            None => self.fill.a == 0,
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> DocumentPoint {
        DocumentPoint::new(x, y).expect("finite")
    }

    fn shape(path: Vec<PathSegment>) -> Result<Shape, ShapeError> {
        Shape::new(ShapeId::new(), path, Rgba8::WHITE)
    }

    #[test]
    fn well_formed_paths_are_accepted() {
        assert!(Shape::rect(ShapeId::new(), 0.0, 0.0, 10.0, 5.0, Rgba8::WHITE).is_some());
        assert!(Shape::rect(ShapeId::new(), f64::NAN, 0.0, 1.0, 1.0, Rgba8::WHITE).is_none());
    }

    #[test]
    fn ellipses_and_lines_are_well_formed() {
        let ellipse =
            Shape::ellipse(ShapeId::new(), 10.0, 20.0, 5.0, 3.0, Rgba8::WHITE).expect("ellipse");
        assert_eq!(ellipse.path().len(), 6);
        assert!(matches!(ellipse.path()[5], PathSegment::Close));
        // The four anchor points sit on the axes at the given radii.
        let anchors: Vec<(f64, f64)> = ellipse
            .path()
            .iter()
            .filter_map(|s| match s {
                PathSegment::MoveTo(p) => Some((p.as_vec().x, p.as_vec().y)),
                PathSegment::CubicTo(_, _, p) => Some((p.as_vec().x, p.as_vec().y)),
                _ => None,
            })
            .collect();
        assert_eq!(
            anchors,
            vec![
                (15.0, 20.0),
                (10.0, 23.0),
                (5.0, 20.0),
                (10.0, 17.0),
                (15.0, 20.0)
            ]
        );
        assert!(Shape::ellipse(ShapeId::new(), f64::NAN, 0.0, 1.0, 1.0, Rgba8::WHITE).is_none());

        let stroke = Stroke::new(2.0, Rgba8::WHITE).expect("stroke");
        let line = Shape::line(ShapeId::new(), (0.0, 0.0), (4.0, 3.0), stroke).expect("line");
        assert_eq!(line.path().len(), 2);
        assert_eq!(line.fill(), Rgba8::TRANSPARENT);
        assert_eq!(line.stroke(), Some(stroke));
        assert!(Shape::line(ShapeId::new(), (0.0, 0.0), (f64::INFINITY, 0.0), stroke).is_none());
    }

    #[test]
    fn strokes_validate_their_width() {
        assert!(Stroke::new(2.5, Rgba8::WHITE).is_ok());
        for bad in [0.0, -1.0, f64::NAN, f64::INFINITY, MAX_STROKE_WIDTH + 1.0] {
            assert_eq!(
                Stroke::new(bad, Rgba8::WHITE),
                Err(ShapeError::InvalidStrokeWidth)
            );
        }
        let stroke = Stroke::new(3.0, Rgba8::opaque(1, 2, 3)).expect("stroke");
        let shape = Shape::rect(ShapeId::new(), 0.0, 0.0, 1.0, 1.0, Rgba8::WHITE)
            .expect("rect")
            .with_stroke(stroke);
        assert_eq!(shape.stroke(), Some(stroke));
    }

    #[test]
    fn malformed_paths_are_rejected() {
        assert_eq!(shape(vec![]), Err(ShapeError::Empty));
        assert_eq!(
            shape(vec![PathSegment::Close]),
            Err(ShapeError::MustStartWithMove)
        );
        let line_after_close = vec![
            PathSegment::MoveTo(p(0.0, 0.0)),
            PathSegment::Close,
            PathSegment::LineTo(p(1.0, 1.0)),
        ];
        assert_eq!(shape(line_after_close), Err(ShapeError::NoCurrentPoint));
        let too_long = vec![PathSegment::MoveTo(p(0.0, 0.0)); MAX_SEGMENTS + 1];
        assert_eq!(shape(too_long), Err(ShapeError::TooManySegments));
    }

    #[test]
    fn gradients_need_two_different_points_and_report_visibility() {
        assert_eq!(
            Gradient::linear(
                p(1.0, 1.0),
                p(1.0, 1.0),
                Rgba8::WHITE,
                Rgba8::opaque(0, 0, 0)
            ),
            Err(GradientError::SamePoint)
        );
        let clear = Gradient::linear(
            p(0.0, 0.0),
            p(1.0, 0.0),
            Rgba8::TRANSPARENT,
            Rgba8::TRANSPARENT,
        )
        .expect("gradient");
        assert!(clear.is_invisible());
        let faded = Gradient::linear(p(0.0, 0.0), p(1.0, 0.0), Rgba8::TRANSPARENT, Rgba8::WHITE)
            .expect("gradient");
        let shape =
            Shape::rect(ShapeId::new(), 0.0, 0.0, 4.0, 4.0, Rgba8::TRANSPARENT).expect("rect");
        assert!(shape.fill_is_invisible());
        let shape = shape.with_gradient(faded);
        assert!(
            !shape.fill_is_invisible(),
            "a visible gradient paints even over a clear fill"
        );
        assert_eq!(shape.gradient(), Some(faded));
        assert!(shape.with_gradient(clear).fill_is_invisible());
    }
}
