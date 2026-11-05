//! The content of an in-between keyframe: what two keyframes' content is `t` of the way along.
//!
//! `t` is the already-eased fraction (see `span_fraction`). A polyline stroke goes through
//! flare-tween's stroke sequence (resampled and matched point by point), any other shape through
//! the contour morph, an instance through scale, rotation and translation, a camera through its
//! own blend. Anything else, or two contents that do not pair up, gives `None`.

use flare_core::{DocumentPoint, InstanceId, ShapeId, Transform2D};
use flare_document::{Content, Instance, PathSegment, Shape};
use flare_tween::{Easing, Interpolation, MAX_POINTS, Params, SequenceOptions, StrokePoint};

/// The polyline's points, if the path is exactly one open `MoveTo` then `LineTo`s.
fn polyline(path: &[PathSegment]) -> Option<Vec<StrokePoint>> {
    let mut points = Vec::new();
    for (i, seg) in path.iter().enumerate() {
        let (PathSegment::MoveTo(p) | PathSegment::LineTo(p)) = seg else {
            return None;
        };
        if matches!(seg, PathSegment::MoveTo(_)) != (i == 0) {
            return None;
        }
        points.push(StrokePoint {
            x: p.as_vec().x,
            y: p.as_vec().y,
        });
        if points.len() > MAX_POINTS {
            return None;
        }
    }
    (points.len() >= 2).then_some(points)
}

fn shape_between(a: &Shape, b: &Shape, t: f64) -> Option<Shape> {
    let path = match (polyline(a.path()), polyline(b.path())) {
        (Some(from), Some(to)) => {
            let options = SequenceOptions {
                factor: t.clamp(0.0, 1.0),
                interpolation: Interpolation::Linear,
                easing: Easing::Auto,
                params: Params::default(),
                flip: false,
            };
            let points = flare_tween::interpolate_sequence(&from, &to, &options).ok()?;
            let mut path = Vec::with_capacity(points.len());
            for (i, p) in points.iter().enumerate() {
                let at = DocumentPoint::new(p.x, p.y).ok()?;
                path.push(if i == 0 {
                    PathSegment::MoveTo(at)
                } else {
                    PathSegment::LineTo(at)
                });
            }
            path
        }
        _ => return crate::morph(a, b, t).and_then(|m| renewed(&m, m.path().to_vec())),
    };
    renewed(a, path)
}

/// `like`'s paint on `path`, under a new identity.
fn renewed(like: &Shape, path: Vec<PathSegment>) -> Option<Shape> {
    Some(
        Shape::new(ShapeId::new(), path, like.fill())
            .ok()?
            .painted_like(like),
    )
}

pub fn inbetween_content(a: &Content, b: &Content, t: f64) -> Option<Content> {
    if !t.is_finite() {
        return None;
    }
    match (a, b) {
        (Content::Shape(x), Content::Shape(y)) => shape_between(x, y, t).map(Content::Shape),
        (Content::Instance(x), Content::Instance(y)) if x.symbol() == y.symbol() => {
            let m = crate::affine::interpolate(
                x.transform().as_affine(),
                y.transform().as_affine(),
                t,
                None,
            )?;
            let transform = Transform2D::new(m).ok()?;
            Some(Content::Instance(
                Instance::new(InstanceId::new(), x.symbol(), transform).with_playback(x.playback()),
            ))
        }
        (Content::Camera(x), Content::Camera(y)) => Some(Content::Camera(x.lerp(y, t))),
        _ => None,
    }
}
