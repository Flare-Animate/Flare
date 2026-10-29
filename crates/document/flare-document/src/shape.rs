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

/// A filled vector shape. Points are already validated finite by `DocumentPoint`.
#[derive(Debug, Clone, PartialEq)]
pub struct Shape {
    id: ShapeId,
    path: Vec<PathSegment>,
    fill: Rgba8,
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
            stroke: None,
        })
    }

    /// Adds an outline. A transparent fill with a stroke gives an outline-only shape.
    #[must_use]
    pub fn with_stroke(mut self, stroke: Stroke) -> Self {
        self.stroke = Some(stroke);
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
}
