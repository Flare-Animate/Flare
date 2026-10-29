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
}

/// A filled vector shape. Points are already validated finite by `DocumentPoint`.
#[derive(Debug, Clone, PartialEq)]
pub struct Shape {
    id: ShapeId,
    path: Vec<PathSegment>,
    fill: Rgba8,
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
        Ok(Self { id, path, fill })
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
