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

    fn bounds_of(shape: &Shape) -> (f64, f64, f64, f64) {
        let mut b = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for s in shape.path() {
            let points: Vec<&DocumentPoint> = match s {
                PathSegment::MoveTo(p) | PathSegment::LineTo(p) => vec![p],
                PathSegment::CubicTo(a, c, p) => vec![a, c, p],
                PathSegment::Close => vec![],
            };
            for p in points {
                let v = p.as_vec();
                b = (b.0.min(v.x), b.1.min(v.y), b.2.max(v.x), b.3.max(v.y));
            }
        }
        b
    }

    #[test]
    fn rounded_rectangles_keep_their_bounds_clamp_the_radius_and_square_off_at_zero() {
        let id = ShapeId::new();
        let r =
            Shape::rounded_rect(id, 10.0, 20.0, 100.0, 50.0, 8.0, Rgba8::WHITE).expect("rounded");
        assert_eq!(bounds_of(&r), (10.0, 20.0, 110.0, 70.0));
        assert_eq!(
            r.path()
                .iter()
                .filter(|s| matches!(s, PathSegment::CubicTo(..)))
                .count(),
            4
        );
        // A radius bigger than half the short side is a pill, not a bulge outside the box.
        let pill =
            Shape::rounded_rect(id, 0.0, 0.0, 100.0, 20.0, 999.0, Rgba8::WHITE).expect("pill");
        assert_eq!(bounds_of(&pill), (0.0, 0.0, 100.0, 20.0));
        let square =
            Shape::rounded_rect(id, 0.0, 0.0, 10.0, 10.0, 0.0, Rgba8::WHITE).expect("square");
        assert_eq!(square.path().len(), 5, "zero radius is the plain rectangle");
        assert!(Shape::rounded_rect(id, f64::NAN, 0.0, 10.0, 10.0, 2.0, Rgba8::WHITE).is_none());
    }

    #[test]
    fn polygons_and_stars_have_the_right_corners_on_the_right_circle() {
        let id = ShapeId::new();
        let tri =
            Shape::polygon(id, (50.0, 50.0), 10.0, 3, 0.0, None, Rgba8::WHITE).expect("triangle");
        let corners: Vec<(f64, f64)> = tri
            .path()
            .iter()
            .filter_map(|s| match s {
                PathSegment::MoveTo(p) | PathSegment::LineTo(p) => {
                    Some((p.as_vec().x, p.as_vec().y))
                }
                _ => None,
            })
            .collect();
        assert_eq!(corners.len(), 3);
        assert!(
            (corners[0].0 - 50.0).abs() < 1e-9 && (corners[0].1 - 40.0).abs() < 1e-9,
            "first corner straight up"
        );
        for (x, y) in &corners {
            let d = ((x - 50.0).powi(2) + (y - 50.0).powi(2)).sqrt();
            assert!((d - 10.0).abs() < 1e-9, "on the circle: {d}");
        }
        let star =
            Shape::polygon(id, (0.0, 0.0), 10.0, 5, 0.0, Some(0.5), Rgba8::WHITE).expect("star");
        let radii: Vec<f64> = star
            .path()
            .iter()
            .filter_map(|s| match s {
                PathSegment::MoveTo(p) | PathSegment::LineTo(p) => Some(p.as_vec().length()),
                _ => None,
            })
            .collect();
        assert_eq!(radii.len(), 10, "five points, five notches");
        for (i, r) in radii.iter().enumerate() {
            let want = if i % 2 == 0 { 10.0 } else { 5.0 };
            assert!((r - want).abs() < 1e-9, "{i}: {r}");
        }
        for (sides, radius) in [(2, 5.0), (65, 5.0), (5, 0.0), (5, -1.0), (5, f64::NAN)] {
            assert!(
                Shape::polygon(id, (0.0, 0.0), radius, sides, 0.0, None, Rgba8::WHITE).is_none()
            );
        }
    }
}
