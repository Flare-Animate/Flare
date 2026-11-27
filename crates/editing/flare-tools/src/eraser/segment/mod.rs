//! The Vector Eraser's Segment type, after OpenToonz's: where lines cross, it takes only the
//! stretch of a line the tip touched, out to the nearest crossing on either side, rather than the
//! whole line. What crosses a shape is everything else drawn on the same layer at that frame (a
//! line counts as wide as it is drawn) and, for a line, the line itself where it loops over
//! itself.
//!
//! It erases lines only, in two ways. A line drawn with a stroke (the Pencil, the Line, the Pen,
//! a shape's outline) is split along its middle at the points where it meets the edge of what
//! crosses it, and the pieces the tip touched go; a shape's fill under its outline stays. A fill
//! no wider than a line ([`MAX_LINE_WIDTH`]: a Brush stroke, a Pencil line drawn with a pen) is
//! sliced along the edges of what crosses it, and the parts the tip touched go. Either way, a
//! line nothing crosses goes whole, as the Whole Line type takes it. A wider fill is an area,
//! such as the Paint Bucket pours under line art, and is left for the Eraser.
//!
//! Only lines cross, for the same reason: every other shape's line, and every other fill that is
//! a line and about as wide as the one erased ([`line_like`]). An area, or a fill much wider than
//! the line, lies against it with its edge along it (the Paint Bucket's fills are laid up to the
//! lines that close them) and would slice it down its length.

use flare_core::{Rgba8, ShapeId};
use flare_document::{Content, FrameSpan, Shape};
use flare_input::InputSample;
use glamx::DVec2;
use i_overlay::core::fill_rule::FillRule;
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::float::single::SingleFloatOverlay;
use i_overlay::float::slice::FloatSlice;

use super::{Cut, eraser_outline};
use crate::common::cut::{Contour, boxes_meet, fill_piece, subpaths};

mod line;
use line::{Line, line_parts, path_of_lines};

/// The most points of one line checked against each other for where it crosses itself; a longer
/// line is split only where other drawing crosses it, so the check stays bounded.
pub const MAX_SELF_CROSSING_POINTS: usize = 4_096;

/// Lengths along a line closer than this are the same place.
pub(super) const SAME: f64 = 1e-7;

/// The widest a fill with no line may be and still be a line the Segment type erases and lets
/// cross, in stage units: as wide as a broad Brush stroke, narrower than most areas filled.
pub const MAX_LINE_WIDTH: f64 = 64.0;

/// How much wider than the shape being erased another fill may be and still count as a line
/// crossing it, as a share of the shape's own width, and in stage units over that.
const LINE_LIKE_SHARE: f64 = 2.0;
const LINE_LIKE_MORE: f64 = 8.0;

/// What the Segment type does to `shape`, the element `index` of `span`, under the closed
/// outlines `eraser`.
pub(super) fn erase_segments(
    span: &FrameSpan,
    index: usize,
    shape: &Shape,
    eraser: &[Contour],
) -> Cut {
    if eraser.is_empty() || shape.text().is_some() {
        return Cut::Untouched;
    }
    let parts = subpaths(shape.path());
    let all: Vec<Contour> = parts.iter().map(|(c, _)| c.clone()).collect();
    if !boxes_meet(&all, eraser) {
        return Cut::Untouched;
    }
    let near: Vec<&Shape> = span
        .elements()
        .enumerate()
        .filter(|(i, _)| *i != index)
        .filter_map(|(_, content)| match content {
            Content::Shape(other) => Some(other),
            _ => None,
        })
        .collect();
    let crossing = regions(&near, &all, shape);
    let mut touched = false;
    let mut left: Vec<Option<Shape>> = Vec::new();
    let areas: Vec<Contour> = all.iter().filter(|c| c.len() >= 3).cloned().collect();
    // An outlined shape's fill is not a line, and stays as it is under its outline.
    let fill_is_line = shape.stroke().is_none() && width_of(&areas) <= MAX_LINE_WIDTH;
    if !shape.fill_is_invisible() && !fill_is_line {
        left.push(Some(shape.clone().restroked(None)));
    } else if !shape.fill_is_invisible() {
        match fill_parts(&areas, &crossing, eraser) {
            None => left.push(Some(shape.clone().restroked(None))),
            Some(rest) => {
                touched = true;
                if !rest.is_empty() {
                    left.push(fill_piece(shape, &rest));
                }
            }
        }
    }
    if let Some(stroke) = shape.stroke() {
        let lines: Vec<Line> = parts
            .iter()
            .map(|(c, closed)| {
                let mut points: Vec<DVec2> = c.iter().map(|p| DVec2::new(p[0], p[1])).collect();
                if *closed && points.first() != points.last() {
                    points.extend(points.first().copied());
                }
                Line {
                    points,
                    closed: *closed,
                }
            })
            .collect();
        let line_of = |lines: &[Line]| {
            path_of_lines(lines)
                .and_then(|path| Shape::new(ShapeId::new(), path, Rgba8::TRANSPARENT).ok())
                .map(|s| s.with_stroke(stroke).merged(shape.is_merged()))
        };
        match line_parts(&lines, &crossing, eraser) {
            None => left.push(
                Shape::new(ShapeId::new(), shape.path().to_vec(), Rgba8::TRANSPARENT)
                    .ok()
                    .map(|s| s.with_stroke(stroke).merged(shape.is_merged())),
            ),
            Some(kept) => {
                touched = true;
                if !kept.is_empty() {
                    left.push(line_of(&kept));
                }
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

/// About how wide the fill `contours` is: twice its area over its outline's length, which for a
/// ribbon is its width.
fn width_of(contours: &[Contour]) -> f64 {
    let (mut area, mut around) = (0.0, 0.0);
    for c in contours.iter().filter(|c| c.len() >= 3) {
        let n = c.len();
        let mut signed = 0.0;
        for i in 0..n {
            let (a, b) = (c[i], c[(i + 1) % n]);
            signed += a[0] * b[1] - b[0] * a[1];
            around += ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
        }
        area += signed / 2.0;
    }
    if around > 0.0 {
        2.0 * area.abs() / around
    } else {
        0.0
    }
}

/// Whether a fill `width` across is thin enough beside a shape `own` across to cross it as a
/// line does, rather than being an area laid against it.
pub(super) fn line_like(width: f64, own: f64) -> bool {
    width <= own * LINE_LIKE_SHARE + LINE_LIKE_MORE
}

/// The outlines of what crosses `shape` (whose subpaths are `own`) among the shapes `near`: their
/// lines as wide as they are drawn, and their fills that are thin enough to be lines.
fn regions(near: &[&Shape], own: &[Contour], shape: &Shape) -> Vec<Contour> {
    let reach = shape.stroke().map_or(0.0, |s| s.width() / 2.0);
    let own_width = match shape.stroke() {
        Some(stroke) if shape.fill_is_invisible() => stroke.width(),
        _ => width_of(own),
    };
    let grown = |contours: &[Contour], by: f64| -> Vec<Contour> {
        contours
            .iter()
            .map(|c| {
                c.iter()
                    .flat_map(|p| [[p[0] - by, p[1] - by], [p[0] + by, p[1] + by]])
                    .collect()
            })
            .collect()
    };
    let own = grown(own, reach);
    let mut out = Vec::new();
    for other in near {
        let parts = subpaths(other.path());
        let contours: Vec<Contour> = parts.iter().map(|(c, _)| c.clone()).collect();
        let half = other.stroke().map_or(0.0, |s| s.width() / 2.0);
        if !boxes_meet(&grown(&contours, half), &own) {
            continue;
        }
        let width = width_of(&contours);
        if !other.fill_is_invisible()
            && other.stroke().is_none()
            && width <= MAX_LINE_WIDTH
            && line_like(width, own_width)
        {
            out.extend(contours.iter().filter(|c| c.len() >= 3).cloned());
        }
        if let Some(stroke) = other.stroke() {
            let width = stroke.width().max(1.0);
            for (contour, closed) in &parts {
                let mut samples: Vec<InputSample> = contour
                    .iter()
                    .filter_map(|p| InputSample::mouse(p[0], p[1]))
                    .collect();
                if *closed && let Some(first) = samples.first().copied() {
                    samples.push(first);
                }
                let outline = eraser_outline(&samples, width, false);
                if outline.len() >= 3 {
                    out.push(outline.iter().map(|p| [p.x, p.y]).collect());
                }
            }
        }
    }
    out
}

/// What is left of the fill `areas` once it is sliced along the edges of `crossing` and the
/// parts `eraser` touches are taken; `None` when the eraser takes nothing of it.
fn fill_parts(areas: &[Contour], crossing: &[Contour], eraser: &[Contour]) -> Option<Vec<Contour>> {
    let eraser: Vec<Contour> = eraser.to_vec();
    if areas.is_empty()
        || areas
            .overlay(&eraser, OverlayRule::Intersect, FillRule::NonZero)
            .is_empty()
    {
        return None;
    }
    let edges: Vec<Contour> = crossing
        .iter()
        .map(|c| {
            let mut closed = c.clone();
            closed.extend(c.first().copied());
            closed
        })
        .collect();
    // With nothing crossing it, the fill is one part (or several, if it was drawn apart).
    let parts = if edges.is_empty() {
        areas.overlay(
            &Vec::<Contour>::new(),
            OverlayRule::Union,
            FillRule::NonZero,
        )
    } else {
        areas.slice_by(&edges, FillRule::NonZero)
    };
    let taken: Vec<Contour> = parts
        .into_iter()
        .filter(|part| {
            !part
                .overlay(&eraser, OverlayRule::Intersect, FillRule::NonZero)
                .is_empty()
        })
        .flatten()
        .collect();
    if taken.is_empty() {
        return None;
    }
    Some(
        areas
            .overlay(&taken, OverlayRule::Difference, FillRule::NonZero)
            .into_iter()
            .flatten()
            .collect(),
    )
}
