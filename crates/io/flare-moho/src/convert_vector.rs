//! A Moho vector layer's mesh as one Flare shape: every curve is a subpath of straight segments
//! (handles are not read, so a smooth curve becomes a polyline through its points). The fill is
//! the mesh's fill colour when the file has one.

use flare_core::{DocumentPoint, Rgba8, ShapeId};
use flare_document::{PathSegment, Shape, Stroke};

use crate::model::Mesh;

/// Flare has no source for the fill colours of a Moho drawing, so closed curves get this grey and
/// open ones only a line, which keeps the drawing visible.
const FILL: Rgba8 = Rgba8::new(170, 170, 170, 255);
const LINE: Rgba8 = Rgba8::new(30, 30, 30, 255);

/// `None` when the mesh has no curve of two points or more.
pub(crate) fn shape(mesh: &Mesh, units_to_px: f64, origin: [f64; 2]) -> Option<Shape> {
    let point = |i: usize| {
        let [x, y] = *mesh.points.get(i)?;
        DocumentPoint::new(origin[0] + x * units_to_px, origin[1] - y * units_to_px).ok()
    };
    let mut path = Vec::new();
    let mut any_closed = false;
    for curve in &mesh.curves {
        let points: Vec<DocumentPoint> = curve.points.iter().filter_map(|i| point(*i)).collect();
        let [first, rest @ ..] = points.as_slice() else {
            continue;
        };
        if rest.is_empty() {
            continue;
        }
        path.push(PathSegment::MoveTo(*first));
        path.extend(rest.iter().map(|p| PathSegment::LineTo(*p)));
        if curve.closed {
            any_closed = true;
            path.push(PathSegment::Close);
        }
    }
    let fill = match mesh.fill {
        Some([r, g, b, a]) => Rgba8::new(byte(r), byte(g), byte(b), byte(a)),
        None if any_closed => FILL,
        None => Rgba8::TRANSPARENT,
    };
    let stroke = Stroke::new(2.0, LINE).ok()?;
    Shape::new(ShapeId::new(), path, fill)
        .ok()
        .map(|s| s.with_stroke(stroke))
}

/// A channel in 0..=1 as a byte; the reader has already clamped it.
fn byte(channel: f64) -> u8 {
    (channel * 255.0).round() as u8
}
