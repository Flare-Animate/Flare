//! Edit Multiple Frames, as Animate has it: with a range of frames marked on the timeline, the
//! keyframes in it all show on the stage at once and are picked, selected and transformed
//! together, so a whole animation can be moved, scaled or turned in one go and stay consistent
//! from frame to frame.
//!
//! A span is "in" the range when any of its frames is. The window draws the frames
//! [`frames_shown`] names; the pickers here gather spans from all of them; and
//! [`transform_range`] is the whole operation for a command line or an agent.

use emath::{Pos2, Rect};
use flare_commands::Command;
use flare_core::SceneId;
use flare_document::{Content, Document, FrameSpan, Layer};
use glamx::{DAffine2, DVec2};

use crate::common::*;
use crate::free_transform::transform_many;
use crate::lasso::spans_in_polygon;

/// The most frames a range draws at once. A range of more keyframes than this shows the first
/// ones: each is a whole picture to draw.
pub const MAX_FRAMES_SHOWN: usize = 120;

fn overlaps(span: &FrameSpan, from: u32, to: u32) -> bool {
    span.start() <= to && span.end() > u64::from(from)
}

/// Whether `layers[index]` can be changed: it is shown and neither it nor a folder it is in is
/// locked. Bounded by the number of layers, so a damaged list of parents cannot loop.
fn editable(layers: &[Layer], index: usize) -> bool {
    let layer = &layers[index];
    if !layer.is_visible() || layer.is_locked() {
        return false;
    }
    let mut parent = layer.parent();
    for _ in 0..layers.len() {
        let Some(folder) = parent.and_then(|id| layers.iter().find(|l| l.id() == id)) else {
            return true;
        };
        if folder.is_locked() || !folder.is_visible() {
            return false;
        }
        parent = folder.parent();
    }
    true
}

/// The frames to draw for the range `from..=to`: where each keyframe in it first shows, in
/// order, at most [`MAX_FRAMES_SHOWN`]. A keyframe that began before the range shows at its
/// first frame.
pub fn frames_shown(document: &Document, scene: SceneId, from: u32, to: u32) -> Vec<u32> {
    let Some(timeline) = scene_timeline(document, scene) else {
        return Vec::new();
    };
    let (from, to) = (from.min(to), from.max(to));
    let mut frames: Vec<u32> = timeline
        .layers()
        .iter()
        .filter(|l| l.is_visible() && !l.is_camera() && !l.is_guide() && !l.is_folder())
        .flat_map(|l| l.spans().iter().filter(|s| overlaps(s, from, to)))
        .map(|s| s.start().max(from))
        .collect();
    frames.sort_unstable();
    frames.dedup();
    frames.truncate(MAX_FRAMES_SHOWN);
    frames
}

/// Every element in the range on layers that can be changed: what Select All selects while
/// editing multiple frames. Topmost layer first, then by frame.
pub fn spans_in_range(document: &Document, scene: SceneId, from: u32, to: u32) -> Vec<SpanRef> {
    let Some(timeline) = scene_timeline(document, scene) else {
        return Vec::new();
    };
    let (from, to) = (from.min(to), from.max(to));
    let layers = timeline.layers();
    let mut out = Vec::new();
    for (index, layer) in layers.iter().enumerate() {
        if !editable(layers, index) {
            continue;
        }
        for span in layer.spans().iter().filter(|s| overlaps(s, from, to)) {
            for (element, content) in span.elements().enumerate() {
                if matches!(
                    content,
                    Content::Empty | Content::Camera(_) | Content::Armature(_)
                ) {
                    continue;
                }
                out.push(SpanRef {
                    layer: layer.id(),
                    start: span.start(),
                    element,
                });
            }
        }
    }
    out
}

/// What a lasso or marquee around `polygon` selects while editing multiple frames: the elements
/// it takes in at any of the frames shown, each once, on layers that can be changed.
pub fn spans_in_polygon_range(
    document: &Document,
    scene: SceneId,
    from: u32,
    to: u32,
    polygon: &[Pos2],
) -> Vec<SpanRef> {
    let allowed = spans_in_range(document, scene, from, to);
    let mut out: Vec<SpanRef> = Vec::new();
    for frame in frames_shown(document, scene, from, to) {
        for found in spans_in_polygon(document, scene, frame, polygon) {
            if allowed.contains(&found) && !out.contains(&found) {
                out.push(found);
            }
        }
    }
    out
}

/// How to move what is in a range: scaled and turned about the middle of all of it, then moved.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RangeTransform {
    pub dx: f64,
    pub dy: f64,
    pub scale_x: f64,
    pub scale_y: f64,
    /// Clockwise on the stage, in degrees.
    pub rotate_degrees: f64,
}

impl Default for RangeTransform {
    fn default() -> Self {
        Self {
            dx: 0.0,
            dy: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            rotate_degrees: 0.0,
        }
    }
}

/// The bounds of everything `refs` names, together.
fn union_bounds(document: &Document, scene: SceneId, refs: &[SpanRef]) -> Option<Rect> {
    spans_bounds(document, scene, refs)
        .into_iter()
        .reduce(|a, b| a.union(b))
}

/// Transforms every element in the range on layers that can be changed, as one list of commands
/// (one undo step), about the middle of all of them together: Edit Multiple Frames with Select
/// All and Free Transform. `None` when there is nothing to move or the transform does nothing
/// or cannot be undone (a zero scale).
pub fn transform_range(
    document: &Document,
    scene: SceneId,
    from: u32,
    to: u32,
    how: RangeTransform,
) -> Option<Vec<Command>> {
    let refs = spans_in_range(document, scene, from, to);
    if refs.is_empty() {
        return None;
    }
    // Things with no outline (an empty symbol) have no middle; a move does not need one.
    let middle = union_bounds(document, scene, &refs).map_or(Pos2::ZERO, |b| b.center());
    let pivot = DVec2::new(f64::from(middle.x), f64::from(middle.y));
    let matrix = DAffine2::from_translation(DVec2::new(how.dx, how.dy))
        * DAffine2::from_translation(pivot)
        * DAffine2::from_angle(how.rotate_degrees.to_radians())
        * DAffine2::from_scale(DVec2::new(how.scale_x, how.scale_y))
        * DAffine2::from_translation(-pivot);
    transform_many(document, scene, &refs, matrix)
}

#[cfg(test)]
mod tests;
