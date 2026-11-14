//! A video from the configured export directory brought into the open project to draw over: what
//! an import would make, and the import. Both are `flare_tools::video` and `flare_import`'s.

use std::path::PathBuf;

use rmcp::{handler::server::wrapper::Parameters, tool, tool_router};
use serde_json::{Value, json};

use crate::color::safe_name;
use crate::ops::{apply, first_scene};
use crate::params_video::{ImportVideo, PlanVideoImport};
use crate::server::{FlareServer, Outcome, State};
use flare_tools::video::{Asked, Plan, plan, plan_json};

/// Longest layer name taken.
const MAX_NAME: usize = 256;

impl State {
    /// The movie called `source` in the export directory, which it must stay inside.
    fn video_path(&self, source: &str) -> Result<PathBuf, String> {
        let dir = super::export::export_dir(self)?
            .canonicalize()
            .map_err(|error| format!("cannot resolve the export directory: {error}"))?;
        let path = dir.join(safe_name(source)?);
        if !path.is_file() {
            return Err(format!("{source} is not a file in the export directory"));
        }
        let path = path
            .canonicalize()
            .map_err(|error| format!("cannot resolve {source}: {error}"))?;
        if !path.starts_with(&dir) {
            return Err("the video must stay inside the export directory".to_owned());
        }
        Ok(path)
    }

    /// What importing the movie at `path` with `asked` would make in the open project.
    fn video_plan(&self, path: &std::path::Path, asked: &Asked) -> Result<Plan, String> {
        let source = match &self.video_tools {
            Some(tools) => flare_import::probe_video_with(tools, path),
            None => flare_import::probe_video(path),
        }
        .map_err(|e| e.to_string())?;
        let document = self.session.document();
        let settings = asked
            .settings(document, &source)
            .map_err(|e| e.to_string())?;
        plan(document, &source, &settings).map_err(|e| e.to_string())
    }
}

#[tool_router(router = video_tools, vis = "pub(super)")]
impl FlareServer {
    #[tool(
        description = "Say what importing a video from the export directory would make, reading nothing and changing nothing: the video's size, rate and length, the frames it makes (one picture per frame of the timeline, taken at `fps`), the pictures' size and megabytes, how long the animation lasts at the document's rate and whether that keeps the video's timing, and, when it is too large for the document, why and what length (fitting_minutes) or size (fitting_side) would fit. Needs ffmpeg on the server's PATH."
    )]
    fn plan_video_import(&self, Parameters(args): Parameters<PlanVideoImport>) -> Outcome {
        self.read(|state| {
            let path = state.video_path(&args.source)?;
            let asked = Asked {
                fps: args.fps,
                max_minutes: args.max_minutes,
                max_side: args.max_side,
                opacity: None,
            };
            Ok(plan_json(&state.video_plan(&path, &asked)?))
        })
    }

    #[tool(
        description = "Import a video from the export directory to draw over: its pictures, taken at `fps` from its start for at most `max_minutes`, one to a frame of the timeline from `frame`, in one Graphic symbol played once on a new see-through layer at the bottom of the first scene, fitted to the stage. Moving or scaling that instance moves every picture. Refused, with what would fit, when too large for the document (plan_video_import says first). Hide or delete the layer before exporting. One undo step (needs write access and ffmpeg on the server's PATH)."
    )]
    fn import_video(&self, Parameters(args): Parameters<ImportVideo>) -> Outcome {
        self.write(args.expected_revision, |state| {
            let path = state.video_path(&args.source)?;
            let asked = Asked {
                fps: args.fps,
                max_minutes: args.max_minutes,
                max_side: args.max_side,
                opacity: args.opacity,
            };
            let plan = state.video_plan(&path, &asked)?;
            if let Some(why) = plan.too_large {
                return Err(format!(
                    "the video is too large to import: {why}; at most {:.2} minutes fit at this rate and size{}",
                    plan.fitting_seconds / 60.0,
                    plan.fitting_side
                        .map_or_else(String::new, |side| format!(", or pictures {side} pixels long"))
                ));
            }
            let name = match args.name {
                Some(name) => name,
                None => path
                    .file_stem()
                    .map_or_else(|| "Video".to_owned(), |n| n.to_string_lossy().into_owned()),
            };
            if name.trim().is_empty() || name.chars().count() > MAX_NAME {
                return Err(format!("the layer needs a name of 1 to {MAX_NAME} characters"));
            }
            let scene = first_scene(state)?;
            let frame = args.frame.unwrap_or(0);
            let document = state.session.document();
            let placed = match &state.video_tools {
                Some(tools) => flare_import::import_video_with(
                    tools, document, scene, frame, &name, &path, &plan,
                ),
                None => flare_import::import_video(document, scene, frame, &name, &path, &plan),
            }
            .map_err(|e| e.to_string())?;
            let mut out: Value = apply(state, placed.commands)?;
            out["plan"] = plan_json(&plan);
            out["layer"] = json!(placed.layer.as_uuid());
            out["symbol"] = json!(placed.symbol.as_uuid());
            out["frame"] = json!(frame);
            out["frames"] = json!(placed.frames);
            out["distinct_pictures"] = json!(placed.distinct);
            Ok(out)
        })
    }
}
