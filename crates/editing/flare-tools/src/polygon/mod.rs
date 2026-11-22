//! The Polygon tool: click the corners of an outline, close it, and it becomes one filled shape
//! (with the stroke, when there is one) with straight sides through every corner. The corners
//! are clicked as the Polygon Lasso's are (`crate::lasso::polygon`); this makes the shape the
//! desktop app, the browser, the command line and the MCP server all draw.

use flare_core::{DocumentPoint, Rgba8, ShapeId};
use flare_document::{PathSegment, Shape, Stroke};
use thiserror::Error;

pub use crate::lasso::polygon::MAX_CORNERS;

/// Why corners could not become a shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum PolygonError {
    #[error("a polygon needs at least three corners")]
    TooFewCorners,
    #[error("a polygon has at most {MAX_CORNERS} corners")]
    TooManyCorners,
    #[error("a polygon's corners must be finite numbers")]
    NotFinite,
    #[error("the corners lie on one line and enclose nothing")]
    NoArea,
}

/// A closed shape filled with `fill` through `corners`, in order, with `stroke` around it when
/// given. A corner repeated straight after itself, and a last corner on the first, count once.
pub fn polygon_shape(
    corners: &[(f64, f64)],
    fill: Rgba8,
    stroke: Option<Stroke>,
) -> Result<Shape, PolygonError> {
    if corners.len() > MAX_CORNERS {
        return Err(PolygonError::TooManyCorners);
    }
    let mut kept: Vec<(f64, f64)> = Vec::with_capacity(corners.len());
    for &(x, y) in corners {
        if !(x.is_finite() && y.is_finite()) {
            return Err(PolygonError::NotFinite);
        }
        if kept.last() != Some(&(x, y)) {
            kept.push((x, y));
        }
    }
    if kept.len() > 1 && kept.first() == kept.last() {
        kept.pop();
    }
    if kept.len() < 3 {
        return Err(PolygonError::TooFewCorners);
    }
    if area(&kept).abs() <= f64::EPSILON {
        return Err(PolygonError::NoArea);
    }
    let mut path = Vec::with_capacity(kept.len() + 1);
    for (i, &(x, y)) in kept.iter().enumerate() {
        let point = DocumentPoint::new(x, y).map_err(|_| PolygonError::NotFinite)?;
        path.push(if i == 0 {
            PathSegment::MoveTo(point)
        } else {
            PathSegment::LineTo(point)
        });
    }
    path.push(PathSegment::Close);
    let shape = Shape::new(ShapeId::new(), path, fill).map_err(|_| PolygonError::NotFinite)?;
    Ok(match stroke {
        Some(stroke) => shape.with_stroke(stroke),
        None => shape,
    })
}

/// The signed area of the outline through `corners` (the shoelace formula).
fn area(corners: &[(f64, f64)]) -> f64 {
    let n = corners.len();
    (0..n)
        .map(|i| {
            let (a, b) = (corners[i], corners[(i + 1) % n]);
            a.0 * b.1 - b.0 * a.1
        })
        .sum::<f64>()
        / 2.0
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests;
