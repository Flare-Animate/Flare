//! A video from the configured export directory brought into the open project to draw over: what
//! an import would make, and the import. Both are `flare_tools::video` and `flare_import`'s.

use std::path::PathBuf;

use rmcp::{handler::server::wrapper::Parameters, tool, tool_router};
use serde_json::{Value, json};

use crate::ops::{apply, first_scene};
use crate::params_video::{ImportVideo, PlanVideoImport, SetBitmapCompression};
use crate::server::{FlareServer, Outcome, State};
use flare_core::BitmapId;
use flare_document::DEFAULT_PHOTO_QUALITY;
use flare_tools::library::{Compression, set_bitmap_compression};
use flare_tools::video::{Asked, Plan, plan, plan_json};

/// Longest layer name taken.
const MAX_NAME: usize = 256;

impl State {
    /// The movie called `source` in the export directory, which it must stay inside.
    fn video_path(&self, source: &str) -> Result<PathBuf, String> {
        super::export::export_file(self, source, "the video")
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
                quality: args.quality,
                lossless: args.lossless.unwrap_or(false),
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
                quality: args.quality,
                lossless: args.lossless.unwrap_or(false),
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

    #[tool(
        description = "Choose how the library's bitmaps are stored, as Animate's Bitmap Properties do: \"photo\" keeps each as a JPEG at `quality` (about a ninth of the room for a frame of video, which looks the same; a drawing with hard edges is better lossless), \"lossless\" keeps every pixel. Every bitmap unless `bitmaps` names some; one a JPEG would not shrink stays lossless. Says how many changed and the bytes before and after. One undo step (needs write access)."
    )]
    fn set_bitmap_compression(
        &self,
        Parameters(args): Parameters<SetBitmapCompression>,
    ) -> Outcome {
        self.write(args.expected_revision, |state| {
            let compression = match (args.compression.as_str(), args.quality) {
                ("photo", quality) => Compression::Photo {
                    quality: quality.unwrap_or(DEFAULT_PHOTO_QUALITY),
                },
                ("lossless", None) => Compression::Lossless,
                ("lossless", Some(_)) => {
                    return Err("quality is for photo; lossless keeps every pixel".to_owned());
                }
                (other, _) => {
                    return Err(format!(
                        "compression is \"photo\" or \"lossless\", not {other:?}"
                    ));
                }
            };
            let which = args
                .bitmaps
                .unwrap_or_default()
                .iter()
                .map(|id| {
                    uuid::Uuid::parse_str(id)
                        .map(BitmapId::from_uuid)
                        .map_err(|_| format!("{id:?} is not a bitmap id"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let done = set_bitmap_compression(state.session.document(), &which, compression)
                .map_err(|e| e.to_string())?;
            let mut out = if done.commands.is_empty() {
                json!({ "revision": state.session.revision() })
            } else {
                apply(state, done.commands)?
            };
            out["changed"] = json!(done.changed);
            out["unchanged"] = json!(done.unchanged);
            out["kept_lossless_not_smaller"] = json!(done.not_smaller);
            out["bitmap_bytes_before"] = json!(done.bytes_before);
            out["bitmap_bytes_after"] = json!(done.bytes_after);
            Ok(out)
        })
    }
}
