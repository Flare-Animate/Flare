use std::sync::Arc;

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
    /// Shared, so copying a shape (every edit and every undo snapshot does) copies no points.
    path: Arc<[PathSegment]>,
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
            path: path.into(),
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

    /// This shape with a different solid fill; a gradient is dropped, since the new color is
    /// what should show.
    #[must_use]
    pub fn with_fill(mut self, fill: Rgba8) -> Self {
        self.fill = fill;
        self.gradient = None;
        self
    }

    /// This shape with its outline replaced, or removed with `None`.
    #[must_use]
    pub fn restroked(mut self, stroke: Option<Stroke>) -> Self {
        self.stroke = stroke;
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

    /// A rectangle with rounded corners of radius `r` (clamped to half the shorter side), as
    /// four straight edges and four cubic corners.
    pub fn rounded_rect(
        id: ShapeId,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        r: f64,
        fill: Rgba8,
    ) -> Option<Self> {
        let r = r.clamp(0.0, w.min(h) / 2.0);
        if r <= 0.0 {
            return Self::rect(id, x, y, w, h, fill);
        }
        // Control-point distance that makes a cubic follow a quarter circle closely.
        const K: f64 = 0.552_284_749_830_793_4;
        let k = r * (1.0 - K);
        let p = |x, y| DocumentPoint::new(x, y).ok();
        let (x2, y2) = (x + w, y + h);
        let path = vec![
            PathSegment::MoveTo(p(x + r, y)?),
            PathSegment::LineTo(p(x2 - r, y)?),
            PathSegment::CubicTo(p(x2 - k, y)?, p(x2, y + k)?, p(x2, y + r)?),
            PathSegment::LineTo(p(x2, y2 - r)?),
            PathSegment::CubicTo(p(x2, y2 - k)?, p(x2 - k, y2)?, p(x2 - r, y2)?),
            PathSegment::LineTo(p(x + r, y2)?),
            PathSegment::CubicTo(p(x + k, y2)?, p(x, y2 - k)?, p(x, y2 - r)?),
            PathSegment::LineTo(p(x, y + r)?),
            PathSegment::CubicTo(p(x, y + k)?, p(x + k, y)?, p(x + r, y)?),
            PathSegment::Close,
        ];
        Self::new(id, path, fill).ok()
    }

    /// A regular polygon with `sides` corners (3 to 64), or a star with `sides` points when
    /// `inner` is `Some(ratio)`, the inner radius as a fraction of `radius`. The first corner is
    /// at `rotation` radians clockwise from straight up.
    pub fn polygon(
        id: ShapeId,
        center: (f64, f64),
        radius: f64,
        sides: u32,
        rotation: f64,
        inner: Option<f64>,
        fill: Rgba8,
    ) -> Option<Self> {
        if !(3..=64).contains(&sides) || !radius.is_finite() || radius <= 0.0 {
            return None;
        }
        let inner = inner.map(|ratio| ratio.clamp(0.05, 0.95) * radius);
        let corners = if inner.is_some() { sides * 2 } else { sides };
        let step = std::f64::consts::TAU / f64::from(corners);
        let mut path = Vec::with_capacity(corners as usize + 2);
        for i in 0..corners {
            let r = match inner {
                Some(inner) if i % 2 == 1 => inner,
                _ => radius,
            };
            let angle = rotation + step * f64::from(i);
            // Angle zero points up; the stage's y axis points down.
            let point =
                DocumentPoint::new(center.0 + r * angle.sin(), center.1 - r * angle.cos()).ok()?;
            path.push(if i == 0 {
                PathSegment::MoveTo(point)
            } else {
                PathSegment::LineTo(point)
            });
        }
        path.push(PathSegment::Close);
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

    /// The fill as one color, for formats without gradients: the solid fill, or the average of
    /// a gradient's two colors.
    pub fn flat_fill(&self) -> Rgba8 {
        match self.gradient {
            Some(g) => {
                let mean = |a: u8, b: u8| (u16::from(a) + u16::from(b)).div_ceil(2) as u8;
                let (s, e) = (g.start, g.end);
                Rgba8::new(
                    mean(s.r, e.r),
                    mean(s.g, e.g),
                    mean(s.b, e.b),
                    mean(s.a, e.a),
                )
            }
            None => self.fill,
        }
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
mod tests;
