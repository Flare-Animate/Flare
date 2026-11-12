//! What lies under a point of the stage.

use flare_eval::{DrawnShape, Frame};
use vello_cpu::kurbo::{BezPath, PathEl, Point, Shape as _};

#[allow(unused_imports)]
use crate::image::unpremultiply_in_place;
#[allow(unused_imports)]
use crate::paint::{bez_path, clip_path, color, linear_gradient, stroke_style};
#[allow(unused_imports)]
use crate::{Image, PremultipliedImage, RenderError, Viewport};

/// The least width, in stage units, a stroke is picked by.
const MIN_STROKE_PICK: f64 = 4.0;

/// Indices into `frame.shapes` of every shape whose fill or stroke contains `point` (scene
/// coordinates), topmost first. Fills use the same non-zero rule as rendering, and an open
/// outline is closed for its fill as rendering closes it; a stroke counts where it is drawn
/// (round caps and joins, width scaled by the shape's transform), so a line or an outline-only
/// shape can be picked. Fully transparent fills without a stroke and singular transforms are not
/// hittable. Callers that must skip some shapes (locked layers) walk this list instead of taking
/// only the first.
pub fn hit_test_all(frame: &Frame<'_>, point: glamx::DVec2) -> Vec<usize> {
    let clip_paths: Vec<BezPath> = frame.clips.iter().map(|clip| clip_path(clip)).collect();
    let scene_point = Point::new(point.x, point.y);
    frame
        .shapes
        .iter()
        .enumerate()
        .rev()
        .filter(|(_, drawn)| {
            let Some((local, scale)) = local_point(drawn, &clip_paths, scene_point, point) else {
                return false;
            };
            let path = bez_path(drawn.shape.path());
            (!drawn.shape.fill_is_invisible() && closed(&path).contains(local))
                || on_stroke(drawn, &path, local, scale)
        })
        .map(|(index, _)| index)
        .collect()
}

/// Index into `frame.shapes` of the topmost shape under `point`; see [`hit_test_all`].
pub fn hit_test(frame: &Frame<'_>, point: glamx::DVec2) -> Option<usize> {
    hit_test_all(frame, point).into_iter().next()
}

/// Where a point falls on one drawn shape, for the tools that paint a shape or take its paint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaintHit {
    /// Index into `frame.shapes`.
    pub index: usize,
    /// On the stroke, where it is drawn.
    pub on_stroke: bool,
    /// Inside the fill, and the fill shows there.
    pub on_fill: bool,
    /// Inside an area the shape encloses, painted or not: what the Paint Bucket fills. A filled
    /// shape encloses its fill; an outline with no fill encloses its inside when every part of it
    /// is closed or its ends come within the gap, as a hand-drawn loop's do.
    pub fillable: bool,
}

/// What the paint tools find under `point` (scene coordinates), topmost first: every shape it is
/// on or inside. `gap` is how far apart, in stage units, the ends of an outline may be and still
/// enclose its inside. Shapes that draw nothing (no stroke and an invisible fill) are left out,
/// as are those masked away there and those with a singular transform.
pub fn paint_hits(frame: &Frame<'_>, point: glamx::DVec2, gap: f64) -> Vec<PaintHit> {
    let clip_paths: Vec<BezPath> = frame.clips.iter().map(|clip| clip_path(clip)).collect();
    let scene_point = Point::new(point.x, point.y);
    frame
        .shapes
        .iter()
        .enumerate()
        .rev()
        .filter_map(|(index, drawn)| {
            let invisible = drawn.shape.fill_is_invisible();
            if invisible && drawn.shape.stroke().is_none() {
                return None;
            }
            let (local, scale) = local_point(drawn, &clip_paths, scene_point, point)?;
            let path = bez_path(drawn.shape.path());
            let inside = closed(&path).contains(local);
            let on_fill = inside && !invisible;
            let gap = if gap.is_finite() { gap.max(0.0) } else { 0.0 };
            let hit = PaintHit {
                index,
                on_stroke: on_stroke(drawn, &path, local, scale),
                on_fill,
                fillable: on_fill || (inside && encloses(&path, gap / scale)),
            };
            (hit.on_stroke || hit.fillable).then_some(hit)
        })
        .collect()
}

/// `point` in the shape's own coordinates and how many stage units one of them is, or `None`
/// where a mask hides the shape or its transform flattens it.
fn local_point(
    drawn: &DrawnShape<'_>,
    clip_paths: &[BezPath],
    scene_point: Point,
    point: glamx::DVec2,
) -> Option<(Point, f64)> {
    // Masked artwork can only be picked where it is actually visible.
    if drawn.clips.iter().any(|index| {
        clip_paths
            .get(*index)
            .is_none_or(|p| !p.contains(scene_point))
    }) {
        return None;
    }
    let determinant = drawn.transform.matrix2.determinant();
    if determinant == 0.0 || !determinant.is_finite() {
        return None;
    }
    let local = drawn.transform.inverse().transform_point2(point);
    Some((
        Point::new(local.x, local.y),
        determinant.abs().sqrt().max(1e-6),
    ))
}

fn on_stroke(drawn: &DrawnShape<'_>, path: &BezPath, local: Point, scale: f64) -> bool {
    drawn.shape.stroke().is_some_and(|stroke| {
        // Flatten finely relative to how big a local unit is on screen.
        let tolerance = 0.1 / scale;
        // A hairline pencil stroke is a target of at least a few units, as in Animate.
        let width = stroke.width().max(MIN_STROKE_PICK / scale);
        let style = stroke_style(width, stroke.with_style(flare_document::StrokeStyle::Solid));
        let outline = vello_cpu::kurbo::stroke(
            path.elements().iter().copied(),
            &style,
            &vello_cpu::kurbo::StrokeOpts::default(),
            tolerance,
        );
        outline.contains(local)
    })
}

/// `path` with every open part closed, as rendering closes it to fill it. Testing the open path
/// itself would leave out the edge from its end back to its start, and count the wrong side.
fn closed(path: &BezPath) -> BezPath {
    let mut out = BezPath::new();
    let mut open = false;
    for element in path.elements() {
        match element {
            PathEl::MoveTo(_) => {
                if open {
                    out.close_path();
                }
                open = true;
            }
            PathEl::ClosePath => open = false,
            PathEl::LineTo(_) | PathEl::QuadTo(..) | PathEl::CurveTo(..) => {}
        }
        out.push(*element);
    }
    if open {
        out.close_path();
    }
    out
}

/// Whether every part of `path` is closed, or ends within `gap` (in the path's own units) of
/// where it started.
fn encloses(path: &BezPath, gap: f64) -> bool {
    let mut start: Option<Point> = None;
    let mut last: Option<Point> = None;
    let near = |start: Option<Point>, last: Option<Point>| match (start, last) {
        (Some(start), Some(last)) => start.distance(last) <= gap,
        // A part that is only a starting point draws nothing to leave open.
        _ => true,
    };
    for element in path.elements() {
        match *element {
            PathEl::MoveTo(p) => {
                if !near(start, last) {
                    return false;
                }
                start = Some(p);
                last = None;
            }
            PathEl::LineTo(p) | PathEl::QuadTo(_, p) | PathEl::CurveTo(_, _, p) => last = Some(p),
            PathEl::ClosePath => {
                start = None;
                last = None;
            }
        }
    }
    near(start, last)
}
