//! The shape a finished pen path makes.

use flare_core::{Rgba8, ShapeId};
use flare_document::{Cap, Join, PathSegment, Shape, ShapeError, Stroke};

/// How a finished pen path is painted: the fill and the outline settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PenStyle {
    pub fill: Rgba8,
    /// Outline a closed path too. An open path has no area, so it is always drawn as a line.
    pub outline: bool,
    pub stroke_width: f64,
    pub stroke_color: Rgba8,
    pub cap: Cap,
    pub join: Join,
}

/// The shape `path` makes under `style`: a closed path is filled (and outlined when asked), an open
/// one has no fill and is drawn with the stroke, like a line.
pub fn path_shape(
    id: ShapeId,
    path: Vec<PathSegment>,
    closed: bool,
    style: &PenStyle,
) -> Result<Shape, ShapeError> {
    let fill = if closed {
        style.fill
    } else {
        Rgba8::TRANSPARENT
    };
    let shape = Shape::new(id, path, fill)?;
    if closed && !style.outline {
        return Ok(shape);
    }
    let stroke = Stroke::new(style.stroke_width, style.stroke_color)?
        .with_cap(style.cap)
        .with_join(style.join);
    Ok(shape.with_stroke(stroke))
}
