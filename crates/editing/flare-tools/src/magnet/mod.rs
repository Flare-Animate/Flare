//! The Magnet: drag near a line and the line follows, the points nearest where the drag started
//! most and those at the edge of its circle not at all, so a hand-drawn line with hundreds of
//! points bends in one gesture instead of point by point. Where a line has too few points to
//! bend smoothly (a straight line has two), points are added along it first.
//!
//! Behaviour after OpenToonz's Magnet tool (`original/flare/sources/tnztools/magnettool.cpp`,
//! BSD-3): written fresh, as a pull from one point to another rather than OpenToonz's step by
//! step drag, so what is shown while dragging is exactly what is let go.

mod path;
#[cfg(test)]
mod tests;

pub use path::{falloff, pull_shape, reaches};

use flare_commands::Command;
use flare_core::{LayerId, SceneId};
use flare_document::{Content, Document};
use glamx::DVec2;

use crate::common::{SpanRef, each_element, find_element, scene_timeline};

/// The smallest and largest circle a pull reaches over, in stage units, and the one a new
/// session starts with.
pub const MIN_MAGNET_RADIUS: f64 = 2.0;
pub const MAX_MAGNET_RADIUS: f64 = 1000.0;
pub const DEFAULT_MAGNET_RADIUS: f64 = 40.0;

/// Why a pull did nothing.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum MagnetError {
    #[error("the points of a pull must be numbers")]
    NotFinite,
    #[error("no line or shape comes within {radius} of ({x}, {y})")]
    NothingInReach { x: f64, y: f64, radius: f64 },
}

/// A pull: from where the drag started to where it is, over a circle `radius` around the start.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pull {
    pub from: DVec2,
    pub to: DVec2,
    pub radius: f64,
}

impl Pull {
    /// A pull, its radius kept between [`MIN_MAGNET_RADIUS`] and [`MAX_MAGNET_RADIUS`] (one that
    /// is not a number is the default). Points that are not numbers are refused.
    pub fn new(from: DVec2, to: DVec2, radius: f64) -> Result<Self, MagnetError> {
        if !(from.is_finite() && to.is_finite()) {
            return Err(MagnetError::NotFinite);
        }
        let radius = if radius.is_finite() {
            radius.clamp(MIN_MAGNET_RADIUS, MAX_MAGNET_RADIUS)
        } else {
            DEFAULT_MAGNET_RADIUS
        };
        Ok(Self { from, to, radius })
    }

    fn nothing(&self) -> MagnetError {
        MagnetError::NothingInReach {
            x: self.from.x,
            y: self.from.y,
            radius: self.radius,
        }
    }
}

/// The shapes at `frame` whose outline comes within the pull's circle: on `layer` alone, or on
/// every unlocked layer. Groups and symbols are not reached into.
pub fn in_reach(
    document: &Document,
    scene: SceneId,
    frame: u32,
    layer: Option<LayerId>,
    pull: &Pull,
) -> Vec<SpanRef> {
    let Some(timeline) = scene_timeline(document, scene) else {
        return Vec::new();
    };
    timeline
        .layers()
        .iter()
        .filter(|l| !l.is_locked() && layer.is_none_or(|id| l.id() == id))
        .filter_map(|l| Some((l.id(), l.span_at(frame)?)))
        .flat_map(|(id, span)| {
            span.elements()
                .enumerate()
                .filter(|(_, content)| matches!(content, Content::Shape(s) if reaches(s, pull)))
                .map(|(element, _)| SpanRef {
                    layer: id,
                    start: span.start(),
                    element,
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

/// The commands that pull the shapes at `targets`, as one batch: one undo step. An element
/// that is not a shape, or that the pull does not reach, is left as it is.
pub fn pull(
    document: &Document,
    scene: SceneId,
    targets: &[SpanRef],
    pull: &Pull,
) -> Result<Vec<Command>, MagnetError> {
    each_element(document, targets, |doc, at| {
        let Some(Content::Shape(shape)) = find_element(doc, scene, at) else {
            return None;
        };
        let pulled = pull_shape(shape, pull)?;
        crate::common::replace_element(doc, scene, at, Content::Shape(pulled))
    })
    .ok_or_else(|| pull.nothing())
}
