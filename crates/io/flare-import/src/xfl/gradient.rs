//! Gradient fills of XFL drawn shapes. A shape's fill is a flat color in the document model unless
//! it is a two-color linear gradient, so a gradient is read as that when it can be, and as the
//! average of its first and last stop when it cannot (a radial gradient, or one this file places
//! somewhere that does not make sense for its shape).
//!
//! The gradient's `Matrix` is taken to be in the shape's own space, like the edges: twips, and
//! mapping a gradient square 32768 units wide (-16384 to 16384, along the matrix's x axis) onto
//! the shape, as a SWF gradient does. That is checked against the shape rather than trusted: a
//! gradient whose square is not about the size of what it fills, or is nowhere near it, is not
//! placed.

use super::color_from;
use crate::xml::Node;
use flare_core::{DocumentPoint, Rgba8};
use flare_document::Gradient;

/// Half the width of the square a gradient matrix maps, in gradient units.
const HALF_SQUARE: f64 = 16384.0;
/// A twip is a twentieth of a pixel.
const TWIPS: f64 = 20.0;

/// A linear gradient as the file gives it, before it meets its shape.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct LinearFill {
    first: Rgba8,
    last: Rgba8,
    /// Where along the square the first and last stops sit, 0 to 1.
    from: f64,
    to: f64,
    /// `a b c d tx ty` of the matrix.
    matrix: [f64; 6],
}

/// What a gradient fill says.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Read {
    /// The average of the first and last stop, for when it is drawn as one color.
    pub(super) flat: Rgba8,
    /// The gradient itself, when it is a linear one with two stops or more.
    pub(super) linear: Option<LinearFill>,
    pub(super) radial: bool,
    /// More stops than the two the document can hold.
    pub(super) extra_stops: bool,
}

/// Reads the gradient inside a `FillStyle`, or `None` if there is none.
pub(super) fn read(style: &Node) -> Option<Read> {
    let (node, radial) = style
        .child("LinearGradient")
        .map(|n| (n, false))
        .or_else(|| style.child("RadialGradient").map(|n| (n, true)))?;
    let stops: Vec<(Rgba8, f64)> = node
        .children_named("GradientEntry")
        .filter_map(|entry| {
            let color = color_from(entry.attr("color")?, entry.attr("alpha"))?;
            let ratio = entry
                .attr("ratio")
                .and_then(|r| r.parse::<f64>().ok())
                .filter(|r| r.is_finite())
                .map_or(0.0, |r| r.clamp(0.0, 1.0));
            Some((color, ratio))
        })
        .collect();
    let (&(first, from), &(last, to)) = (stops.first()?, stops.last()?);
    let number = |key: &str, default: f64| {
        node.child("matrix")
            .and_then(|m| m.child("Matrix"))
            .and_then(|m| m.attr(key))
            .and_then(|v| v.parse::<f64>().ok())
            .filter(|v| v.is_finite())
            .unwrap_or(default)
    };
    let matrix = [
        number("a", 1.0),
        number("b", 0.0),
        number("c", 0.0),
        number("d", 1.0),
        number("tx", 0.0),
        number("ty", 0.0),
    ];
    Some(Read {
        flat: average(first, last),
        linear: (!radial && stops.len() >= 2).then_some(LinearFill {
            first,
            last,
            from,
            to,
            matrix,
        }),
        radial,
        extra_stops: stops.len() > 2,
    })
}

/// The color halfway between two.
fn average(a: Rgba8, b: Rgba8) -> Rgba8 {
    let mid = |x: u8, y: u8| ((u16::from(x) + u16::from(y)) / 2) as u8;
    Rgba8::new(mid(a.r, b.r), mid(a.g, b.g), mid(a.b, b.b), mid(a.a, b.a))
}

/// The gradient laid on a shape whose bounds, in pixels, are `min_x, min_y, max_x, max_y`; or
/// `None` when the matrix does not put a gradient square of about the shape's size on it.
pub(super) fn place(fill: &LinearFill, bounds: (f64, f64, f64, f64)) -> Option<Gradient> {
    let (min_x, min_y, max_x, max_y) = bounds;
    let [a, b, _, _, tx, ty] = fill.matrix;
    let diagonal = (max_x - min_x).hypot(max_y - min_y);
    if diagonal.is_nan() || diagonal <= 0.0 || (fill.to - fill.from).abs() < 1e-6 {
        return None;
    }
    // The square's length along the gradient, in pixels, against the shape it is for.
    let square = a.hypot(b) * 2.0 * HALF_SQUARE / TWIPS;
    if !(0.2 * diagonal..=5.0 * diagonal).contains(&square) {
        return None;
    }
    // Its middle must be on the shape or close to it.
    let (centre_x, centre_y) = (tx / TWIPS, ty / TWIPS);
    let slack = 0.75 * diagonal;
    if !(min_x - slack..=max_x + slack).contains(&centre_x)
        || !(min_y - slack..=max_y + slack).contains(&centre_y)
    {
        return None;
    }
    let at = |t: f64| {
        let x = -HALF_SQUARE + 2.0 * HALF_SQUARE * t;
        DocumentPoint::new((a * x + tx) / TWIPS, (b * x + ty) / TWIPS).ok()
    };
    Gradient::linear(at(fill.from)?, at(fill.to)?, fill.first, fill.last).ok()
}

#[cfg(test)]
mod tests;
