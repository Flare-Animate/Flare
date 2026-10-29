//! Tools that work on layers and frames.

use flare_commands::{Command, LayerChange};
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
            let layers = state
                .session
                .document()
                .scenes()
                .first()
                .map(|s| s.timeline().layers())
                .unwrap_or_default();
            let position = layers
                .iter()
                .position(|l| l.id() == layer)
                .ok_or("layer disappeared")?;
            let count = layers.len();
            let update = |change| Command::UpdateLayer {
                scene,
                layer,
                change,
            };
            let command = match args.operation {
                LayerOperation::Rename => {
                    let name = args.name.as_deref().ok_or("name is required")?.trim();
                    if name.is_empty() {
                        return Err("a layer needs a name".to_owned());
                    }
                    update(LayerChange::Name(name.to_owned()))
                }
                LayerOperation::Delete if count <= 1 => {
                    return Err("a scene needs at least one layer".to_owned());
                }
                LayerOperation::Delete => Command::RemoveLayer { scene, layer },
                LayerOperation::Hide => update(LayerChange::Visible(false)),
                LayerOperation::Show => update(LayerChange::Visible(true)),
                LayerOperation::Lock => update(LayerChange::Locked(true)),
                LayerOperation::Unlock => update(LayerChange::Locked(false)),
                LayerOperation::MoveUp => {
                    let index = position
                        .checked_sub(1)
                        .ok_or("the layer is already at the top")?;
                    Command::MoveLayer {
                        scene,
                        layer,
                        index,
                    }
                }
                LayerOperation::MoveDown if position + 1 >= count => {
                    return Err("the layer is already at the bottom".to_owned());
                }
                LayerOperation::MoveDown => Command::MoveLayer {
                    scene,
                    layer,
                    index: position + 1,
                },
            };
            apply(state, vec![command])
        })
    }
}
