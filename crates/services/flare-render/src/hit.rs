//! What lies under a point of the stage.

use flare_eval::Frame;
use vello_cpu::kurbo::{BezPath, Point, Shape as _};

#[allow(unused_imports)]
use crate::image::unpremultiply_in_place;
#[allow(unused_imports)]
use crate::paint::{bez_path, clip_path, color, linear_gradient, stroke_style};
#[allow(unused_imports)]
use crate::{Image, PremultipliedImage, RenderError, Viewport};

/// Indices into `frame.shapes` of every shape whose fill or stroke contains `point` (scene
/// coordinates), topmost first. Fills use the same non-zero rule as rendering; a stroke counts
/// where it is drawn (round caps and joins, width scaled by the shape's transform), so a line
/// or an outline-only shape can be picked. Fully transparent fills without a stroke and
/// singular transforms are not hittable. Callers that must skip some shapes (locked layers)
/// walk this list instead of taking only the first.
pub fn hit_test_all(frame: &Frame<'_>, point: glamx::DVec2) -> Vec<usize> {
    let clip_paths: Vec<BezPath> = frame.clips.iter().map(|clip| clip_path(clip)).collect();
    let scene_point = Point::new(point.x, point.y);
    frame
        .shapes
        .iter()
        .enumerate()
        .rev()
        .filter(|(_, drawn)| {
            // Masked artwork can only be picked where it is actually visible.
            if drawn.clips.iter().any(|index| {
                clip_paths
                    .get(*index)
                    .is_none_or(|p| !p.contains(scene_point))
            }) {
                return false;
            }
            let determinant = drawn.transform.matrix2.determinant();
            if determinant == 0.0 || !determinant.is_finite() {
                return false;
            }
            let local = drawn.transform.inverse().transform_point2(point);
            let local = Point::new(local.x, local.y);
            let path = bez_path(drawn.shape.path());
            if !drawn.shape.fill_is_invisible() && path.contains(local) {
                return true;
            }
            drawn.shape.stroke().is_some_and(|stroke| {
                // Flatten finely relative to how big a local unit is on screen.
                let tolerance = 0.1 / determinant.abs().sqrt().max(1e-6);
                let style = stroke_style(stroke.width(), stroke);
                let outline = vello_cpu::kurbo::stroke(
                    path.elements().iter().copied(),
                    &style,
                    &vello_cpu::kurbo::StrokeOpts::default(),
                    tolerance,
                );
                outline.contains(local)
            })
        })
        .map(|(index, _)| index)
        .collect()
}

/// Index into `frame.shapes` of the topmost shape under `point`; see [`hit_test_all`].
pub fn hit_test(frame: &Frame<'_>, point: glamx::DVec2) -> Option<usize> {
    hit_test_all(frame, point).into_iter().next()
}
