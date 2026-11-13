//! Frame and layer operations.

use anyhow::{Context, Result};
use flare_commands::layers;
use serde_json::{Value, json};
use std::path::Path;
use uuid::Uuid;

use super::*;

/// Timeline frame operations, shared with the desktop UI through `flare_editor::frames`.
#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum FrameOperation {
    InsertFrame,
    RemoveFrame,
    /// Take the frames from `--at` to `--to` out, moving what follows left (Cut Frames, without
    /// the clipboard).
    RemoveFrames,
    /// Reverse the keyframes from `--at` to `--to`; they must lie wholly inside it.
    ReverseFrames,
    InsertKeyframe,
    InsertBlankKeyframe,
    ClearKeyframe,
    /// Tween the instance in the span at the frame toward the next span's.
    MotionTween,
    /// Turn the shape in the span at the frame into the next span's shape.
    ShapeTween,
    /// Take the tween off the span at the frame.
    RemoveTween,
    /// Set the easing (`--ease`, -100 to 100) of the span's tween.
    SetEase,
    /// Move the keyframe holding `--at` so it starts at `--to`, on `--onto` (default: the same
    /// layer), replacing the frames it lands on and leaving its old frames empty.
    MoveKeyframe,
}

pub fn frame_op(
    path: &Path,
    op: FrameOperation,
    at: u32,
    to: Option<u32>,
    layer: Option<Uuid>,
    ease: Option<i8>,
    onto: Option<Uuid>,
) -> Result<Value> {
    use flare_editor::frames;
    use flare_tools::ops::{self, SpanRef};
    let mut session = open(path)?;
    let (scene, layer_id) = target_layer(&session, layer)?;
    if let FrameOperation::MoveKeyframe = op {
        let to = to.context("move-keyframe needs --to, the frame the keyframe goes to")?;
        let onto_id = match onto {
            Some(id) => target_layer(&session, Some(id))?.1,
            None => layer_id,
        };
        let layers = session
            .document()
            .scenes()
            .first()
            .map(|s| s.timeline().layers())
            .unwrap_or_default();
        let find = |id| layers.iter().find(|l| l.id() == id);
        let from = find(layer_id).context("layer disappeared")?;
        let start = from
            .span_at(at)
            .map(flare_document::FrameSpan::start)
            .with_context(|| format!("there is no keyframe at frame {at} of this layer"))?;
        let commands = frames::drop_span(scene, from, start, find(onto_id).context("layer disappeared")?, to)
            .context("the keyframe cannot go there: it is there already, the layer is a folder, or it would run past the last frame")?;
        session.apply(commands)?;
        commit(&session, path)?;
        return Ok(json!({ "applied": format!("{op:?}"), "frame": to }));
    }
    if let FrameOperation::MotionTween
    | FrameOperation::ShapeTween
    | FrameOperation::RemoveTween
    | FrameOperation::SetEase = op
    {
        let document = session.document();
        let start = document
            .scenes()
            .first()
            .and_then(|s| s.timeline().layers().iter().find(|l| l.id() == layer_id))
            .and_then(|l| l.span_at(at))
            .map(flare_document::FrameSpan::start)
            .with_context(|| format!("there is no span at frame {at} of this layer"))?;
        let span = SpanRef {
            layer: layer_id,
            start,
            element: 0,
        };
        let commands = match op {
            FrameOperation::MotionTween => {
                ops::set_tween(document, scene, span, flare_document::Tween::Linear)
            }
            FrameOperation::ShapeTween => {
                ops::set_tween(document, scene, span, flare_document::Tween::Shape)
            }
            FrameOperation::RemoveTween => {
                ops::set_tween(document, scene, span, flare_document::Tween::None)
            }
            _ => {
                let ease = ease.context("set-ease needs --ease, from -100 to 100")?;
                ops::set_ease(document, scene, span, ease)
            }
        }
        .with_context(|| {
            format!("{op:?} does not apply: a motion tween needs an instance, a shape tween a shape, and nothing may already be set that way")
        })?;
        session.apply(commands)?;
        commit(&session, path)?;
        return Ok(json!({ "applied": format!("{op:?}"), "frame": at }));
    }
    let layer = session
        .document()
        .scenes()
        .first()
        .and_then(|s| s.timeline().layers().iter().find(|l| l.id() == layer_id))
        .context("layer disappeared")?;
    let commands = match op {
        FrameOperation::RemoveFrames => {
            let to = to.context("remove-frames needs --to, the last frame to take out")?;
            frames::remove_frames(scene, layer, at, to)
        }
        FrameOperation::ReverseFrames => {
            let to = to.context("reverse-frames needs --to, the last frame of the range")?;
            Some(
                frames::reverse_frames(scene, layer, at, to)
                    .map_err(|refusal| anyhow::anyhow!("cannot reverse those frames: {refusal}"))?,
            )
        }
        FrameOperation::InsertFrame => frames::insert_frame(scene, layer, at),
        FrameOperation::RemoveFrame => frames::remove_frame(scene, layer, at),
        FrameOperation::InsertKeyframe => frames::insert_keyframe(scene, layer, at),
        FrameOperation::InsertBlankKeyframe => frames::insert_blank_keyframe(scene, layer, at),
        FrameOperation::ClearKeyframe => frames::clear_keyframe(scene, layer, at),
        _ => None,
    }
    .with_context(|| format!("{op:?} does not apply at frame {at} of this layer"))?;
    session.apply(commands)?;
    commit(&session, path)?;
    Ok(json!({ "applied": format!("{op:?}"), "frame": at }))
}

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum LayerOperation {
    Rename,
    Delete,
    Hide,
    Show,
    Lock,
    Unlock,
    Up,
    Down,
    /// Move the layer (a folder with everything in it) to just above `--before`, or to the
    /// bottom without it, into the folder `--folder` or out of any folder without it.
    Move,
}

/// Layer management on the first scene. `layer` defaults to the first layer.
pub fn layer_op(
    path: &Path,
    op: LayerOperation,
    layer: Option<Uuid>,
    name: Option<&str>,
    place: LayerPlace,
) -> Result<Value> {
    let mut session = open(path)?;
    let (scene, layer) = target_layer(&session, layer)?;
    let named = |id: Option<Uuid>| -> Result<Option<flare_core::LayerId>> {
        id.map(|id| target_layer(&session, Some(id)).map(|(_, l)| l))
            .transpose()
    };
    let (before, folder) = (named(place.before)?, named(place.folder)?);
    let document = session.document();
    let commands = match op {
        LayerOperation::Rename => {
            layers::rename(scene, layer, name.context("rename needs --name")?)?
        }
        LayerOperation::Delete => layers::delete(document, scene, layer)?,
        LayerOperation::Hide => layers::set_visible(scene, layer, false),
        LayerOperation::Show => layers::set_visible(scene, layer, true),
        LayerOperation::Lock => layers::set_locked(scene, layer, true),
        LayerOperation::Unlock => layers::set_locked(scene, layer, false),
        LayerOperation::Up => layers::move_by(document, scene, layer, -1)?,
        LayerOperation::Down => layers::move_by(document, scene, layer, 1)?,
        LayerOperation::Move => {
            let commands = layers::move_to(document, scene, layer, before, folder)?;
            if commands.is_empty() {
                anyhow::bail!("the layer is already there");
            }
            commands
        }
    };
    session.apply(commands)?;
    commit(&session, path)?;
    Ok(json!({ "applied": format!("{op:?}"), "layer": layer.as_uuid() }))
}
