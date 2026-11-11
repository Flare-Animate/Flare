//! Cutting what an eraser passed over out of a shape: its fill loses the area, its line loses the
//! stretches inside it. A shape's fill and its line are cut apart, so what is left of a filled,
//! outlined shape is a fill and a line, as erasing does in Animate: the cut edge of a fill gets no
//! outline of its own.

use flare_core::{DocumentPoint, Rgba8, ShapeId};
use flare_document::{MAX_SEGMENTS, PathSegment, Shape};
use glamx::DVec2;
use i_overlay::core::fill_rule::FillRule;
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::float::clip::FloatClip;
use i_overlay::float::single::SingleFloatOverlay;
use i_overlay::string::clip::ClipRule;

/// Straight pieces a curve of a shape is flattened into.
const CURVE_STEPS: usize = 16;

type Contour = Vec<[f64; 2]>;

/// What erasing did to one shape.
#[derive(Clone, Debug, PartialEq)]
pub enum Cut {
    /// The eraser missed it.
    Untouched,
    /// What is left of it, in drawing order (the fill, then the line); empty when nothing is.
    Left(Vec<Shape>),
}

/// The points of each subpath of `path`, curves flattened, and whether the subpath was closed.
fn subpaths(path: &[PathSegment]) -> Vec<(Contour, bool)> {
    let mut out: Vec<(Contour, bool)> = Vec::new();
    let mut current: Contour = Vec::new();
    let mut at = DVec2::ZERO;
    let flush = |current: &mut Contour, out: &mut Vec<(Contour, bool)>, closed: bool| {
        if current.len() >= 2 {
            out.push((std::mem::take(current), closed));
        } else {
            current.clear();
        }
    };
    for segment in path {
        match *segment {
            PathSegment::MoveTo(p) => {
                flush(&mut current, &mut out, false);
                at = p.as_vec();
                current.push([at.x, at.y]);
            }
            PathSegment::LineTo(p) => {
                at = p.as_vec();
                current.push([at.x, at.y]);
            }
            PathSegment::CubicTo(c1, c2, p) => {
                let (from, c1, c2, to) = (at, c1.as_vec(), c2.as_vec(), p.as_vec());
                for step in 1..=CURVE_STEPS {
                    let t = step as f64 / CURVE_STEPS as f64;
                    let u = 1.0 - t;
                    let q = from * (u * u * u)
                        + c1 * (3.0 * t * u * u)
                        + c2 * (3.0 * t * t * u)
                        + to * (t * t * t);
                    current.push([q.x, q.y]);
                }
                at = to;
            }
            PathSegment::Close => {
                let start = current.first().copied();
                flush(&mut current, &mut out, true);
                if let Some(start) = start {
                    at = DVec2::new(start[0], start[1]);
                    current.push(start);
                }
            }
        }
    }
    flush(&mut current, &mut out, false);
    out
}

/// A path of straight segments through `contours`, each closed or left open.
fn path_of(contours: &[Contour], closed: bool) -> Option<Vec<PathSegment>> {
    let point = |p: &[f64; 2]| DocumentPoint::new(p[0], p[1]).ok();
    let mut path = Vec::new();
    for contour in contours.iter().filter(|c| c.len() >= 2) {
        path.push(PathSegment::MoveTo(point(&contour[0])?));
        for p in &contour[1..] {
            path.push(PathSegment::LineTo(point(p)?));
        }
        if closed {
            path.push(PathSegment::Close);
        }
    }
    (!path.is_empty() && path.len() <= MAX_SEGMENTS).then_some(path)
}

/// Whether the boxes round `a` and `b` overlap.
fn boxes_meet(a: &[Contour], b: &[Contour]) -> bool {
    let bounds = |contours: &[Contour]| {
        contours.iter().flatten().fold(
            ([f64::MAX, f64::MAX], [f64::MIN, f64::MIN]),
            |(lo, hi), p| {
                (
                    [lo[0].min(p[0]), lo[1].min(p[1])],
                    [hi[0].max(p[0]), hi[1].max(p[1])],
                )
            },
        )
    };
    let ((alo, ahi), (blo, bhi)) = (bounds(a), bounds(b));
    alo[0] <= bhi[0] && blo[0] <= ahi[0] && alo[1] <= bhi[1] && blo[1] <= ahi[1]
}

/// `shape` with the inside of the closed outlines `eraser` (one for each place the eraser went)
/// taken out of it. A shape made from
/// text is left alone, as Animate asks for text to be broken apart before it is erased; so is a
/// shape whose cut would not be a valid path (too many pieces for one shape), rather than losing
/// it.
pub fn cut(shape: &Shape, eraser: &[Vec<DVec2>]) -> Cut {
    let eraser: Vec<Contour> = eraser
        .iter()
        .filter(|outline| outline.len() >= 3)
        .map(|outline| outline.iter().map(|p| [p.x, p.y]).collect())
        .collect();
    if eraser.is_empty() || shape.text().is_some() {
        return Cut::Untouched;
    }
    let parts = subpaths(shape.path());
    let all: Vec<Contour> = parts.iter().map(|(c, _)| c.clone()).collect();
    if !boxes_meet(&all, &eraser) {
        return Cut::Untouched;
    }
    let mut touched = false;
    let mut left = Vec::new();

    let filled = !shape.fill_is_invisible();
    if filled {
        // Every subpath of a fill is closed when it is painted, so every one is an area here.
        let areas: Vec<Contour> = all.iter().filter(|c| c.len() >= 3).cloned().collect();
        let overlap = areas.overlay(&eraser, OverlayRule::Intersect, FillRule::NonZero);
        if overlap.is_empty() {
            left.push(Some(shape.clone().restroked(None)));
        } else {
            touched = true;
            let rest: Vec<Contour> = areas
                .overlay(&eraser, OverlayRule::Difference, FillRule::NonZero)
                .into_iter()
                .flatten()
                .collect();
            if !rest.is_empty() {
                left.push(fill_piece(shape, &rest));
            }
        }
    }
    if let Some(stroke) = shape.stroke() {
        // A closed subpath's line runs back to where it began.
        let lines: Vec<Contour> = parts
            .iter()
            .map(|(c, closed)| {
                let mut line = c.clone();
                if *closed && c.first() != c.last() {
                    line.extend(c.first().copied());
                }
                line
            })
            .collect();
        let inside = lines.clip_by(
            &eraser,
            FillRule::NonZero,
            ClipRule {
                invert: false,
                boundary_included: false,
            },
        );
        if inside.is_empty() {
            let line = Shape::new(ShapeId::new(), shape.path().to_vec(), Rgba8::TRANSPARENT)
                .ok()
                .map(|s| s.with_stroke(stroke));
            left.push(line);
        } else {
            touched = true;
            let outside = lines.clip_by(
                &eraser,
                FillRule::NonZero,
                ClipRule {
                    invert: true,
                    boundary_included: false,
                },
            );
            if !outside.is_empty() {
                let line = path_of(&outside, false)
                    .and_then(|path| Shape::new(ShapeId::new(), path, Rgba8::TRANSPARENT).ok())
                    .map(|s| s.with_stroke(stroke));
                left.push(line);
            }
        }
    }
    if !touched {
        return Cut::Untouched;
    }
    // A piece that could not be made would be lost: keep the whole shape instead.
    match left.into_iter().collect::<Option<Vec<Shape>>>() {
        Some(pieces) => Cut::Left(pieces),
        None => Cut::Untouched,
    }
}

/// The fill of `shape` over `rest`, its colour or gradient kept, with no line.
fn fill_piece(shape: &Shape, rest: &[Contour]) -> Option<Shape> {
    let path = path_of(rest, true)?;
    let piece = Shape::new(ShapeId::new(), path, shape.fill()).ok()?;
    Some(match shape.gradient() {
        Some(gradient) => piece.with_gradient(gradient),
        None => piece,
    })
}
