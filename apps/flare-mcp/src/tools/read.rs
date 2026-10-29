//! Tools that only look at the project.

use flare_core::InstanceId;
use rmcp::{handler::server::wrapper::Parameters, tool, tool_router};
use serde_json::{Value, json};

use crate::ops::first_scene;
use crate::params::{ExportPng, FrameArgs};
use crate::safe_file_name;
use crate::server::{FlareServer, Outcome};

#[tool_router(router = read_tools, vis = "pub(super)")]
impl FlareServer {
    #[tool(description = "Summarize the project: revision, stage, scenes, layers and symbols.")]
    fn get_info(&self) -> Outcome {
        self.read(|state| {
            let document = state.session.document();
            let scenes: Vec<Value> = document
                .scenes()
                .iter()
                .map(|scene| {
                    let layers: Vec<Value> = scene
                        .timeline()
                        .layers()
                        .iter()
                        .map(|l| {
                            json!({ "id": l.id().as_uuid(), "name": l.name(),
                                    "visible": l.is_visible(), "locked": l.is_locked(),
                                    "spans": l.spans().len() })
                        })
                        .collect();
                    json!({ "id": scene.id().as_uuid(), "name": scene.name(),
                            "frames": scene.timeline().frame_count(), "layers": layers })
                })
                .collect();
            let symbols: Vec<Value> = document
                .symbols()
                .iter()
                .map(|s| json!({ "id": s.id().as_uuid(), "name": s.name() }))
                .collect();
            let (rate, stage) = (document.frame_rate(), document.stage());
            Ok(json!({
                "project": state.path.to_string_lossy(),
                "revision": state.session.revision(),
                "dirty": state.session.is_dirty(),
                "can_undo": state.session.can_undo(),
                "can_redo": state.session.can_redo(),
                "frame_rate": { "numerator": rate.numerator(), "denominator": rate.denominator() },
                "stage": { "width": stage.width(), "height": stage.height(),
                           "background": stage.background().to_array() },
                "scenes": scenes,
                "symbols": symbols,
            }))
        })
    }

    #[tool(description = "List the instances and shapes visible at a frame of the first scene.")]
    fn evaluate_frame(&self, Parameters(args): Parameters<FrameArgs>) -> Outcome {
        self.read(|state| {
            let evaluated = flare_eval::evaluate_frame(
                state.session.document(),
                first_scene(state)?,
                args.frame,
            )
            .map_err(|e| e.to_string())?;
            let ids =
                |path: &[InstanceId]| path.iter().map(|i| json!(i.as_uuid())).collect::<Vec<_>>();
            Ok(json!({
                "frame": args.frame,
                "placed": evaluated.placed.iter().map(|p| json!({
                    "path": ids(&p.path), "symbol": p.symbol.as_uuid(),
                    "matrix": p.transform.to_cols_array() })).collect::<Vec<_>>(),
                "shapes": evaluated.shapes.iter().map(|s| json!({
                    "shape": s.shape.id().as_uuid(), "owner_path": ids(&s.path),
                    "matrix": s.transform.to_cols_array() })).collect::<Vec<_>>(),
            }))
        })
    }

    #[tool(
        description = "Render a frame to a PNG in the export directory (disabled when no export directory is configured)."
    )]
    fn export_png(&self, Parameters(args): Parameters<ExportPng>) -> Outcome {
        self.read(|state| {
            let dir = state
                .permissions
                .export_dir
                .clone()
                .ok_or("exports are disabled: no export directory was configured")?;
            let file = safe_file_name(&args.name)?;
            let document = state.session.document();
            let stage = document.stage();
            let (width, height) = (
                args.width.unwrap_or(stage.width()),
                args.height.unwrap_or(stage.height()),
            );
            const MAX_SIDE: u32 = 16384;
            if width == 0 || height == 0 || width > MAX_SIDE || height > MAX_SIDE {
                return Err(format!("width and height must be between 1 and {MAX_SIDE}"));
            }
            let evaluated = flare_eval::evaluate_frame(document, first_scene(state)?, args.frame)
                .map_err(|e| e.to_string())?;
            let image = flare_render::render(&evaluated, width, height, stage.background())
                .map_err(|e| e.to_string())?;
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            // Written beside the target and renamed, so a reader never sees half a file.
            let mut staged = tempfile::NamedTempFile::new_in(&dir).map_err(|e| e.to_string())?;
            image
                .write_png(std::io::BufWriter::new(&mut staged))
                .map_err(|e| e.to_string())?;
            let out = dir.join(&file);
            staged.persist(&out).map_err(|e| e.error.to_string())?;
            Ok(json!({ "path": out.to_string_lossy(), "width": width, "height": height }))
        })
    }
}
