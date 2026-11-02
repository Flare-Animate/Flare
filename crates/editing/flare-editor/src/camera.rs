//! Setting where the camera looks at a frame, creating the camera layer when the scene has none.
//!
//! The camera holds a keyframe until the next one, so a new keyframe splits the one it lands in
//! (both halves keep the motion of the original), and the first keyframe runs to the end of the
//! scene so the camera does not let go of the picture partway through.

use std::num::NonZeroU32;

use flare_commands::Command;
use flare_core::{LayerId, SceneId};
use flare_document::{Camera, Content, Document, FrameSpan, Layer, Tween};

/// The last frame a camera keyframe may be set on; a SWF has no more frames than this.
pub const MAX_CAMERA_FRAME: u32 = 65_535;

/// The camera looking at `frame`: the layer's own there, or the one that shows the stage as it
/// is. `None` when the scene does not exist.
pub fn camera_now(document: &Document, scene: SceneId, frame: u32) -> Option<Camera> {
    let found = document.scenes().iter().find(|s| s.id() == scene)?;
    Some(
        found
            .timeline()
            .layers()
            .iter()
            .find(|l| l.is_camera())
            .and_then(|l| flare_eval::camera_at(l, frame))
            .unwrap_or_else(|| Camera::centered(&document.stage())),
    )
}

/// The commands that make the camera `camera` from `frame` on. `motion` is the tween and ease the
/// keyframe starts with (to the next keyframe); `None` keeps the motion of the keyframe it
/// replaces or splits, and gives a new one none. `None` when the scene does not exist or `frame`
/// is past [`MAX_CAMERA_FRAME`].
pub fn set_camera_key(
    document: &Document,
    scene: SceneId,
    frame: u32,
    camera: Camera,
    motion: Option<(Tween, i8)>,
) -> Option<Vec<Command>> {
    set_camera_key_named(document, scene, frame, camera, motion, "Camera")
}

/// [`set_camera_key`], naming the camera layer `name` if this makes it (a front end that speaks
/// another language passes the word for it).
pub fn set_camera_key_named(
    document: &Document,
    scene: SceneId,
    frame: u32,
    camera: Camera,
    motion: Option<(Tween, i8)>,
    name: &str,
) -> Option<Vec<Command>> {
    if frame > MAX_CAMERA_FRAME {
        return None;
    }
    let found = document.scenes().iter().find(|s| s.id() == scene)?;
    let timeline = found.timeline();
    // To the end of the movie, and at least to the frame being keyed.
    let end = u32::try_from(timeline.frame_count())
        .unwrap_or(u32::MAX)
        .max(frame.saturating_add(1));
    let key = |start: u32, duration: u32| {
        NonZeroU32::new(duration).map(|d| FrameSpan::new(start, d, Content::Camera(camera)))
    };
    let with_motion = |span: FrameSpan, like: Option<&FrameSpan>| match (motion, like) {
        (Some((tween, ease)), _) => span.with_tween(tween).with_ease(ease),
        (None, Some(like)) => span.with_motion_of(like),
        (None, None) => span,
    };

    let Some(layer) = timeline.layers().iter().find(|l| l.is_camera()) else {
        let span = with_motion(key(frame, end - frame)?, None);
        let id = LayerId::new();
        let layer = Layer::new(id, name, vec![span]).ok()?.with_camera(true);
        // Above everything, where Animate keeps it.
        return Some(vec![
            Command::AddLayer { scene, layer },
            Command::MoveLayer {
                scene,
                layer: id,
                index: 0,
            },
        ]);
    };

    let mut spans: Vec<FrameSpan> = Vec::new();
    let mut placed = false;
    for old in layer.spans() {
        let (start, stop) = (old.start(), u32::try_from(old.end()).unwrap_or(u32::MAX));
        if frame < start || frame >= stop {
            if !placed && frame < start {
                // A gap before this keyframe: the new one runs up to it.
                spans.push(with_motion(key(frame, start - frame)?, None));
                placed = true;
            }
            spans.push(old.clone());
        } else if frame == start {
            spans.push(with_motion(key(start, stop - start)?, Some(old)));
            placed = true;
        } else {
            // Inside a keyframe: it keeps its camera up to here, and the new one takes the rest.
            spans.push(
                FrameSpan::new(
                    start,
                    NonZeroU32::new(frame - start)?,
                    old.content().clone(),
                )
                .with_motion_of(old),
            );
            spans.push(with_motion(key(frame, stop - frame)?, Some(old)));
            placed = true;
        }
    }
    if !placed {
        // After the last keyframe.
        spans.push(with_motion(key(frame, end - frame)?, None));
    }
    let removals = layer.spans().iter().map(|old| Command::RemoveSpan {
        scene,
        layer: layer.id(),
        start: old.start(),
    });
    let inserts = spans.into_iter().map(|span| Command::InsertSpan {
        scene,
        layer: layer.id(),
        span,
    });
    Some(removals.chain(inserts).collect())
}

#[cfg(test)]
mod tests;
