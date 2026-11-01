//! Tools that work on layers and frames.

use flare_commands::layers;
use flare_editor::frames;
use rmcp::{handler::server::wrapper::Parameters, tool, tool_router};

use crate::ops::{apply, first_scene, target_layer};
use crate::params::{FrameOp, FrameOperation, LayerOp, LayerOperation};
use crate::server::{FlareServer, Outcome};

#[tool_router(router = timeline_tools, vis = "pub(super)")]
impl FlareServer {
    #[tool(description = "Insert or remove frames and keyframes on a layer (needs write access).")]
    fn frame_operation(&self, Parameters(args): Parameters<FrameOp>) -> Outcome {
        self.write(args.expected_revision, |state| {
            let scene = first_scene(state)?;
            let layer_id = target_layer(state, args.layer.as_deref())?;
            let layer = state
                .session
                .document()
                .scenes()
                .first()
                .and_then(|s| s.timeline().layers().iter().find(|l| l.id() == layer_id))
                .ok_or("layer disappeared")?;
            let at = args.at;
            let commands = match args.operation {
                FrameOperation::RemoveFrames => {
                    let to = args.to.ok_or("remove_frames needs `to`, the last frame")?;
                    frames::remove_frames(scene, layer, at, to)
                }
                FrameOperation::ReverseFrames => {
                    let to = args.to.ok_or("reverse_frames needs `to`, the last frame")?;
                    Some(
                        frames::reverse_frames(scene, layer, at, to)
                            .map_err(|refusal| format!("cannot reverse those frames: {refusal}"))?,
                    )
                }
                FrameOperation::InsertFrame => frames::insert_frame(scene, layer, at),
                FrameOperation::RemoveFrame => frames::remove_frame(scene, layer, at),
                FrameOperation::InsertKeyframe => frames::insert_keyframe(scene, layer, at),
                FrameOperation::InsertBlankKeyframe => {
                    frames::insert_blank_keyframe(scene, layer, at)
                }
                FrameOperation::ClearKeyframe => frames::clear_keyframe(scene, layer, at),
            }
            .ok_or_else(|| {
                format!(
                    "{:?} does not apply at frame {at} of this layer",
                    args.operation
                )
            })?;
            apply(state, commands)
        })
    }

    #[tool(
        description = "Rename, show/hide, lock/unlock, reorder or delete a layer (needs write access)."
    )]
    fn layer_operation(&self, Parameters(args): Parameters<LayerOp>) -> Outcome {
        self.write(args.expected_revision, |state| {
            let scene = first_scene(state)?;
            let layer = target_layer(state, args.layer.as_deref())?;
            let document = state.session.document();
            let commands = match args.operation {
                LayerOperation::Rename => {
                    let name = args.name.as_deref().ok_or("name is required")?;
                    layers::rename(scene, layer, name).map_err(|e| e.to_string())?
                }
                LayerOperation::Delete => {
                    layers::delete(document, scene, layer).map_err(|e| e.to_string())?
                }
                LayerOperation::Hide => layers::set_visible(scene, layer, false),
                LayerOperation::Show => layers::set_visible(scene, layer, true),
                LayerOperation::Lock => layers::set_locked(scene, layer, true),
                LayerOperation::Unlock => layers::set_locked(scene, layer, false),
                LayerOperation::MoveUp => {
                    layers::move_by(document, scene, layer, -1).map_err(|e| e.to_string())?
                }
                LayerOperation::MoveDown => {
                    layers::move_by(document, scene, layer, 1).map_err(|e| e.to_string())?
                }
            };
            apply(state, commands)
        })
    }
}
