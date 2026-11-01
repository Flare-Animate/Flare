//! The brushes of the rasterizer: colors, gradients, paths and line styles for what a frame holds.

use flare_core::Rgba8;
use flare_document::PathSegment;
use vello_cpu::color::{AlphaColor, Srgb};
use vello_cpu::kurbo::{Affine, BezPath, Cap, Join, Stroke};
use vello_cpu::peniko::Gradient;

#[allow(unused_imports)]
use crate::image::unpremultiply_in_place;
#[allow(unused_imports)]
use crate::{Image, PremultipliedImage, RenderError, Viewport};

pub(crate) fn color(c: Rgba8) -> AlphaColor<Srgb> {
    AlphaColor::from_rgba8(c.r, c.g, c.b, c.a)
}

pub(crate) fn linear_gradient(gradient: &flare_document::Gradient) -> Gradient {
    let (from, to) = (gradient.from().as_vec(), gradient.to().as_vec());
    Gradient::new_linear((from.x, from.y), (to.x, to.y))
        .with_stops([color(gradient.start()), color(gradient.end())])
}

/// The region of mask `clip`, in scene coordinates: the union of its shapes (a shape drawn
/// with the opposite winding over another one cuts a hole, as with any single filled path).
pub(crate) fn clip_path(clip: &[flare_eval::ClipShape<'_>]) -> BezPath {
    let mut path = BezPath::new();
    for member in clip {
        let [a, b, c, d, e, f] = member.transform.to_cols_array();
        let mut one = bez_path(member.shape.path());
        one.apply_affine(Affine::new([a, b, c, d, e, f]));
        path.extend(one.elements().iter().copied());
    }
    path
}

pub(crate) fn bez_path(segments: &[PathSegment]) -> BezPath {
    let mut path = BezPath::new();
    for segment in segments {
        match segment {
            PathSegment::MoveTo(p) => path.move_to((p.as_vec().x, p.as_vec().y)),
            PathSegment::LineTo(p) => path.line_to((p.as_vec().x, p.as_vec().y)),
            PathSegment::CubicTo(a, b, c) => path.curve_to(
                (a.as_vec().x, a.as_vec().y),
                (b.as_vec().x, b.as_vec().y),
                (c.as_vec().x, c.as_vec().y),
            ),
            PathSegment::Close => path.close_path(),
        }
    }
    path
}

/// The line style `stroke` asks for, `width` wide.
pub(crate) fn stroke_style(width: f64, stroke: flare_document::Stroke) -> Stroke {
    use flare_document::{Cap as DocCap, Join as DocJoin};
    let caps = match stroke.cap() {
        DocCap::Butt => Cap::Butt,
        DocCap::Round => Cap::Round,
        DocCap::Square => Cap::Square,
    };
    let join = match stroke.join() {
        DocJoin::Round => Join::Round,
        DocJoin::Bevel => Join::Bevel,
        DocJoin::Miter => Join::Miter,
    };
    // A miter that would stick out more than four widths is cut off, as other renderers do.
    Stroke::new(width)
        .with_caps(caps)
        .with_join(join)
        .with_miter_limit(4.0)
}
