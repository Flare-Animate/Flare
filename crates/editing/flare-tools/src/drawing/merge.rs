//! Merge Drawing, Animate's default way of drawing: shapes drawn on a layer in a keyframe become
//! one drawing that later shapes cut and join, instead of drawing objects of their own.
//!
//! What a merged shape does to the merged drawing under it, as Adobe's "Drawing modes and graphic
//! objects" describes it:
//!
//! - its fill and its line become two elements, which are picked and moved apart;
//! - its fill cuts away the part of every merged fill and line underneath that it covers;
//! - a merged fill of the same colour that it overlaps joins it, so the two are one fill;
//! - a line cuts nothing.
//!
//! Drawing objects, groups, symbols and bitmaps in the keyframe are never touched, and the merged
//! drawing stays beneath them, as Animate keeps its shape under the objects on a layer. What is cut
//! or joined is made of straight pieces, as what the Eraser leaves is.

use flare_core::{Rgba8, ShapeId};
use flare_document::{Content, Layer, Shape};
use glamx::DVec2;
use i_overlay::core::fill_rule::FillRule;
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::float::single::SingleFloatOverlay;

use crate::common::cut::{Contour, Cut, boxes_meet, cut, fill_piece, subpaths};

/// A keyframe's elements once merged shapes have been drawn into it.
#[derive(Clone, Debug, PartialEq)]
pub struct Merged {
    /// The merged drawing first, bottom to top, then the other elements in their order.
    pub elements: Vec<Content>,
    /// Where in `elements` the pieces of the shapes just drawn ended up, bottom first.
    pub placed: Vec<usize>,
}

/// `elements` (a keyframe's, bottom first) with `fresh` drawn above them as merged drawing, in
/// order, by the rules in the module's description. A shape made from text is a drawing object
/// whatever it is asked to be, as Animate's text is, and joins the objects above.
pub fn merge_into(elements: Vec<Content>, fresh: Vec<Shape>) -> Merged {
    // (the piece, whether it comes from a shape drawn now)
    let mut pieces: Vec<(Shape, bool)> = Vec::new();
    let mut objects: Vec<(Content, bool)> = Vec::new();
    for element in elements {
        match element {
            Content::Empty => {}
            Content::Shape(shape) if shape.is_merged() => pieces.push((shape, false)),
            other => objects.push((other, false)),
        }
    }
    for shape in fresh {
        if shape.text().is_some() {
            objects.push((Content::Shape(shape.merged(false)), true));
            continue;
        }
        let shape = shape.merged(true);
        let (fill, line) = split(&shape);
        if let Some(fill) = fill {
            pieces = cover(pieces, fill);
        }
        if let Some(line) = line {
            pieces.push((line, true));
        }
    }
    let mut placed = Vec::new();
    let mut out = Vec::with_capacity(pieces.len() + objects.len());
    for (index, (content, new)) in pieces
        .into_iter()
        .map(|(shape, new)| (Content::Shape(shape), new))
        .chain(objects)
        .enumerate()
    {
        if new {
            placed.push(index);
        }
        out.push(content);
    }
    Merged {
        elements: out,
        placed,
    }
}

/// `shape`'s fill alone and its line alone, either absent when it has none. The fill keeps the
/// shape's identity, or the line does when there is no fill.
fn split(shape: &Shape) -> (Option<Shape>, Option<Shape>) {
    if shape.fill_is_invisible() {
        return (None, shape.stroke().is_some().then(|| shape.clone()));
    }
    let line = shape.stroke().and_then(|stroke| {
        Shape::new(ShapeId::new(), shape.path().to_vec(), Rgba8::TRANSPARENT)
            .ok()
            .map(|line| line.with_stroke(stroke).merged(true))
    });
    (Some(shape.clone().restroked(None)), line)
}

/// The closed areas a shape's fill paints.
fn areas(shape: &Shape) -> Vec<Contour> {
    subpaths(shape.path())
        .into_iter()
        .map(|(contour, _)| contour)
        .filter(|contour| contour.len() >= 3)
        .collect()
}

/// Whether `piece` is a fill that `fill` joins when they overlap: a plain fill of the very same
/// solid colour. A gradient, or a piece with a line, stays a piece of its own.
fn joins(piece: &Shape, fill: &Shape) -> bool {
    piece.stroke().is_none()
        && piece.text().is_none()
        && piece.gradient().is_none()
        && fill.gradient().is_none()
        && !piece.fill_is_invisible()
        && piece.fill() == fill.fill()
}

fn overlap(a: &[Contour], b: &[Contour]) -> bool {
    boxes_meet(a, b)
        && !a
            .to_vec()
            .overlay(&b.to_vec(), OverlayRule::Intersect, FillRule::NonZero)
            .is_empty()
}

/// `pieces` with the new `fill` drawn over them: the same colour joined into it, everything else
/// cut where it covers it, and the fill itself on top.
fn cover(pieces: Vec<(Shape, bool)>, fill: Shape) -> Vec<(Shape, bool)> {
    let area = areas(&fill);
    if area.is_empty() {
        let mut pieces = pieces;
        pieces.push((fill, true));
        return pieces;
    }
    let mut joined = area.clone();
    let mut absorbed = vec![false; pieces.len()];
    for (index, (piece, _)) in pieces.iter().enumerate() {
        if joins(piece, &fill) {
            let theirs = areas(piece);
            if overlap(&theirs, &joined) {
                joined = joined
                    .overlay(&theirs, OverlayRule::Union, FillRule::NonZero)
                    .into_iter()
                    .flatten()
                    .collect();
                absorbed[index] = true;
            }
        }
    }
    // A join too big for one shape is not made: the fill cuts those pieces like any other.
    let top = if absorbed.contains(&true) {
        match fill_piece(&fill, &joined) {
            Some(whole) => whole,
            None => {
                absorbed.fill(false);
                fill.clone()
            }
        }
    } else {
        fill.clone()
    };
    let cutter: Vec<Vec<DVec2>> = area
        .iter()
        .map(|contour| contour.iter().map(|p| DVec2::new(p[0], p[1])).collect())
        .collect();
    let mut out = Vec::with_capacity(pieces.len() + 1);
    for ((piece, new), gone) in pieces.into_iter().zip(absorbed) {
        if gone {
            continue;
        }
        match cut(&piece, &cutter) {
            Cut::Untouched => out.push((piece, new)),
            Cut::Left(rest) => out.extend(rest.into_iter().map(|left| (left, new))),
        }
    }
    out.push((top, true));
    out
}

/// How many of `layer`'s elements, over all its keyframes, are merged drawing rather than
/// drawing objects, as the command line and the MCP server report it.
pub fn merged_count(layer: &Layer) -> usize {
    layer
        .spans()
        .iter()
        .flat_map(|span| span.elements())
        .filter(|element| matches!(element, Content::Shape(shape) if shape.is_merged()))
        .count()
}

#[cfg(test)]
#[path = "merge_rules_tests.rs"]
mod tests;
