//! Reshaping a drawn line by dragging its edge, as Animate's Selection tool does with merged
//! drawing: take hold of any point along an outline (not only its corners) and pull, and that
//! piece of the outline curves after the pull while its two ends stay put. Where the Magnet bends
//! everything near a point, this bends the one edge that was grabbed.
//!
//! Only merged drawing (see `drawing::merge`) is reshaped this way; a drawing object is first
//! selected and edited by its points, as in Animate.

mod bend;
#[cfg(test)]
mod tests;

use flare_commands::Command;
use flare_core::{LayerId, SceneId};
use flare_document::{Content, Document};
use glamx::DVec2;

use crate::common::{SpanRef, find_element, replace_element, scene_timeline};

pub use bend::bend_shape;

/// Why an edge was not reshaped.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ReshapeError {
    #[error("the points of a reshape must be numbers")]
    NotFinite,
    #[error("no merged line or shape has an edge within {tolerance} of ({x}, {y})")]
    NoEdge { x: f64, y: f64, tolerance: f64 },
    #[error("the edge cannot be reshaped (the drawing would be too complex or too large)")]
    Refused,
}

/// The farthest from an edge a grab may be, in stage units, and the one a session starts with.
pub const MAX_TOLERANCE: f64 = 100.0;
pub const DEFAULT_TOLERANCE: f64 = 6.0;

/// An edge taken hold of: the piece of an outline that follows the pull.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EdgeGrab {
    /// The element the edge is part of.
    pub at: SpanRef,
    /// The index in the shape's path of the segment that is the edge (a closing edge is the
    /// `Close`).
    pub segment: usize,
    /// How far along the edge the grab is, 0 to 1.
    pub t: f64,
    /// The point on the edge that was taken, which follows the pull.
    pub point: DVec2,
}

/// The tolerance a caller asked for, kept between 0 and [`MAX_TOLERANCE`]; one that is not a
/// number is the default.
pub fn clamp_tolerance(tolerance: f64) -> f64 {
    if tolerance.is_finite() {
        tolerance.clamp(0.0, MAX_TOLERANCE)
    } else {
        DEFAULT_TOLERANCE
    }
}

/// The nearest merged edge to `at` within `tolerance` among the shapes at `frame`, on `layer`
/// alone or on every unlocked, visible layer (the topmost layer wins a tie).
pub fn edge_at(
    document: &Document,
    scene: SceneId,
    frame: u32,
    layer: Option<LayerId>,
    at: DVec2,
    tolerance: f64,
) -> Option<EdgeGrab> {
    if !at.is_finite() {
        return None;
    }
    let tolerance = clamp_tolerance(tolerance);
    let timeline = scene_timeline(document, scene)?;
    let mut best: Option<(f64, EdgeGrab)> = None;
    for l in timeline
        .layers()
        .iter()
        .filter(|l| !l.is_locked() && l.is_visible() && layer.is_none_or(|id| l.id() == id))
    {
        let Some(span) = l.span_at(frame) else {
            continue;
        };
        for (element, content) in span.elements().enumerate() {
            let Content::Shape(shape) = content else {
                continue;
            };
            if !shape.is_merged() {
                continue;
            }
            if let Some((distance, segment, t, point)) = bend::nearest_edge(shape.path(), at)
                && distance <= tolerance
                && best.as_ref().is_none_or(|(d, _)| distance < *d)
            {
                let grab = EdgeGrab {
                    at: SpanRef {
                        layer: l.id(),
                        start: span.start(),
                        element,
                    },
                    segment,
                    t,
                    point,
                };
                best = Some((distance, grab));
            }
        }
    }
    best.map(|(_, grab)| grab)
}

/// The commands that pull the edge `grab` took hold of to `to`: one batch, one undo step.
pub fn bend(
    document: &Document,
    scene: SceneId,
    grab: &EdgeGrab,
    to: DVec2,
) -> Result<Vec<Command>, ReshapeError> {
    if !(to.is_finite() && grab.point.is_finite()) {
        return Err(ReshapeError::NotFinite);
    }
    let Some(Content::Shape(shape)) = find_element(document, scene, grab.at) else {
        return Err(ReshapeError::Refused);
    };
    let bent =
        bend_shape(shape, grab.segment, grab.t, to - grab.point).ok_or(ReshapeError::Refused)?;
    replace_element(document, scene, grab.at, Content::Shape(bent)).ok_or(ReshapeError::Refused)
}

/// Finds the edge near `from` and pulls it to `to`, for a caller that has no drag to hold the
/// grab across (the command line and the MCP server).
pub fn bend_edge_near(
    document: &Document,
    scene: SceneId,
    frame: u32,
    layer: Option<LayerId>,
    from: DVec2,
    to: DVec2,
    tolerance: f64,
) -> Result<(EdgeGrab, Vec<Command>), ReshapeError> {
    if !(from.is_finite() && to.is_finite()) {
        return Err(ReshapeError::NotFinite);
    }
    let tolerance = clamp_tolerance(tolerance);
    let grab =
        edge_at(document, scene, frame, layer, from, tolerance).ok_or(ReshapeError::NoEdge {
            x: from.x,
            y: from.y,
            tolerance,
        })?;
    let commands = bend(document, scene, &grab, to)?;
    Ok((grab, commands))
}
