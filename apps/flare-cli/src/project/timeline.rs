//! Frame and layer operations.

use anyhow::{Context, Result, bail};
use flare_commands::{Command, LayerChange};
use serde_json::{Value, json};
use std::path::Path;
use uuid::Uuid;

use super::*;

/// Timeline frame operations, shared with the desktop UI through `flare_editor::frames`.
#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum FrameOperation {
    InsertFrame,
    RemoveFrame,
    InsertKeyframe,
    InsertBlankKeyframe,
    ClearKeyframe,
}

pub fn frame_op(path: &Path, op: FrameOperation, at: u32, layer: Option<Uuid>) -> Result<Value> {
    use flare_editor::frames;
    let mut session = open(path)?;
    let (scene, layer_id) = target_layer(&session, layer)?;
    let layer = session
        .document()
        .scenes()
        .first()
        .and_then(|s| s.timeline().layers().iter().find(|l| l.id() == layer_id))
        .context("layer disappeared")?;
    let commands = match op {
        FrameOperation::InsertFrame => frames::insert_frame(scene, layer, at),
        FrameOperation::RemoveFrame => frames::remove_frame(scene, layer, at),
        FrameOperation::InsertKeyframe => frames::insert_keyframe(scene, layer, at),
        FrameOperation::InsertBlankKeyframe => frames::insert_blank_keyframe(scene, layer, at),
        FrameOperation::ClearKeyframe => frames::clear_keyframe(scene, layer, at),
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
}

/// Layer management on the first scene. `layer` defaults to the first layer.
pub fn layer_op(
    path: &Path,
    op: LayerOperation,
    layer: Option<Uuid>,
    name: Option<&str>,
) -> Result<Value> {
    let mut session = open(path)?;
    let (scene, layer) = target_layer(&session, layer)?;
    let layers = session
        .document()
        .scenes()
        .first()
        .context("document has no scene")?
        .timeline()
        .layers();
    let position = layers
        .iter()
        .position(|l| l.id() == layer)
        .context("layer disappeared")?;
    let update = |change| Command::UpdateLayer {
        scene,
        layer,
        change,
    };
    let command = match op {
        LayerOperation::Rename => {
            let name = name
                .map(str::trim)
                .filter(|n| !n.is_empty())
                .context("rename needs --name")?;
            update(LayerChange::Name(name.to_owned()))
        }
        LayerOperation::Delete => {
            if layers.len() <= 1 {
                bail!("a scene needs at least one layer");
            }
            Command::RemoveLayer { scene, layer }
        }
        LayerOperation::Hide => update(LayerChange::Visible(false)),
        LayerOperation::Show => update(LayerChange::Visible(true)),
        LayerOperation::Lock => update(LayerChange::Locked(true)),
        LayerOperation::Unlock => update(LayerChange::Locked(false)),
        LayerOperation::Up | LayerOperation::Down => {
            let index = if matches!(op, LayerOperation::Up) {
                position
                    .checked_sub(1)
                    .context("the layer is already at the top")?
            } else {
                position + 1
            };
            if index >= layers.len() {
                bail!("the layer is already at the bottom");
            }
            Command::MoveLayer {
                scene,
                layer,
                index,
            }
        }
    };
    session.apply([command])?;
    commit(&session, path)?;
    Ok(json!({ "applied": format!("{op:?}"), "layer": layer.as_uuid() }))
}
