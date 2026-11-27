//! The vector erasers: take lines by the piece or whole, as a drag with the desktop app's Eraser
//! does when a vector eraser is picked in its list.

use flare_core::LayerId;
use flare_tools::eraser::vector::{self, VectorEraseError, VectorEraserType};
use glamx::DVec2;
use rmcp::{handler::server::wrapper::Parameters, tool, tool_router};
use serde_json::json;

use crate::ops::{apply, first_scene, parse_id};
use crate::params_vector_erase::VectorErase;
use crate::server::{FlareServer, Outcome};

#[tool_router(router = vector_erase_tools, vis = "pub(super)")]
impl FlareServer {
    #[tool(
        description = "Vector Eraser: take lines away at `frame`, as a drag with the desktop app's Eraser does with a vector eraser picked, as one undo step. `type` segment (the default) takes from each line a round tip `size` across touches along `points` (one point is a dot) the stretch out to where other lines on its layer cross it, or the whole line when nothing crosses it; lines are those drawn with a stroke and fills no wider than 64 units (Brush strokes), and wider fills are left. whole takes every shape the tip touches; rectangular every shape wholly inside the box between two `points`; freehand every shape wholly inside the loop through three or more, so a shape the area only crosses stays. Symbols, bitmaps and text are left. `layer` keeps it to one layer or folder; otherwise every shown, unlocked layer. Taking nothing is refused."
    )]
    fn vector_erase(&self, Parameters(args): Parameters<VectorErase>) -> Outcome {
        self.write(args.expected_revision, |state| {
            let scene = first_scene(state)?;
            let name = args.kind.as_deref().unwrap_or("segment");
            let kind = VectorEraserType::parse(name).ok_or_else(|| {
                format!("Vector Eraser: no type '{name}'; segment, whole, rectangular or freehand")
            })?;
            let refuse = |e: VectorEraseError| format!("Vector Eraser: {e}");
            let points: Vec<DVec2> = args.points.iter().map(|p| DVec2::from(*p)).collect();
            let area = vector::area(kind, &points, args.size.unwrap_or(20.0)).map_err(refuse)?;
            let layer = args
                .layer
                .as_deref()
                .map(|id| parse_id(id, "layer").map(LayerId::from_uuid))
                .transpose()?;
            let frame = args.frame.unwrap_or(0);
            let commands = vector::erase_with(
                state.session.document(),
                scene,
                frame,
                &[area],
                kind.take(),
                layer,
            );
            if commands.is_empty() {
                return Err(refuse(VectorEraseError::NothingTaken));
            }
            let revision = apply(state, commands)?;
            Ok(json!({
                "type": kind.name(),
                "frame": frame,
                "erased": true,
                "revision": revision["revision"].clone(),
            }))
        })
    }
}
