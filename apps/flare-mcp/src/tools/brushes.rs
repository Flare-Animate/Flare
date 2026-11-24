//! Another art tool's pen profiles read as Flare's brush presets: the same reading the desktop
//! app and the command line use, from a file in the configured export directory. The server
//! keeps no brush list of its own; the presets come back so the caller can keep or use them.
//! The file is passed by name (`source`) because profile files are binary. The Brush itself
//! lays strokes given as points, as `flare_tools::brush` lays a desktop drag.

use std::num::NonZeroU32;
use std::path::PathBuf;

use flare_core::Rgba8;
use flare_tools::brush::{self, BrushRequest, Placement};

use rmcp::{handler::server::wrapper::Parameters, tool, tool_router};
use serde_json::{Value, json};

use crate::ops::{apply, color_or, first_scene, target_layer};
use crate::params::ImportBrushes;
use crate::params_brush::BrushStrokes;
use crate::server::{FlareServer, Outcome, State};

/// Longest pen-profile file read, the bound the library itself applies.
const MAX_PROFILE_BYTES: u64 = flare_import::brushes::MAX_OWNED_BYTES;

impl State {
    /// The pen-profile file called `source` in the export directory, which it must stay inside.
    fn profile_path(&self, source: &str) -> Result<PathBuf, String> {
        super::export::export_file(self, source, "the profile file")
    }
}

#[tool_router(router = brush_tools, vis = "pub(super)")]
impl FlareServer {
    #[tool(
        description = "Read another art tool's pen profiles from a file in the export directory and return them as Flare's brush presets: each with its name, nib (shape, angle, flatness), pressure response, velocity, flow, stabilizer and taper, the parts that map over. `source` names a Photoshop `.abr` (versions 1 and 2), a Krita `.kpp` or a Krita `.bundle`; the ending decides the reader. Returns `presets` and `notes`, one per thing the file held that does not map (a tip that is a picture, a preset too large). A file that is not a pen profile at all is refused by name."
    )]
    fn import_brush_profiles(&self, Parameters(args): Parameters<ImportBrushes>) -> Outcome {
        self.read(|state| {
            let path = state.profile_path(&args.source)?;
            let meta = std::fs::metadata(&path).map_err(|e| e.to_string())?;
            if meta.len() > MAX_PROFILE_BYTES {
                return Err(format!(
                    "{source} is larger than {MAX_PROFILE_BYTES} bytes, more than a pen-profile file holds",
                    source = args.source
                ));
            }
            let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
            let set = flare_import::brushes::import_profile_bytes(&path, &bytes)
                .map_err(|e| e.to_string())?;
            let presets: Vec<Value> = set
                .presets
                .iter()
                .map(serde_json::to_value)
                .collect::<std::result::Result<_, _>>()
                .map_err(|e| e.to_string())?;
            Ok(json!({ "presets": presets, "notes": set.notes }))
        })
    }
    #[tool(
        description = "Brush: lay brush strokes on a layer of the first scene, as the desktop's Brush lays a drag when the pen lifts, all of them as one undo step. Each stroke is a filled outline in `color` whose width follows the pressure given with each point, shaped by the ready-made `profile` (its nib, pressure curve, taper and texture) and the settings given. They join the layer's keyframe at `frame` as more elements, or start a keyframe there; with `merge` they are Merge Drawing, as for `draw_rect`. Locked, hidden, folder, mask, guide and camera layers, and keyframes that tween, are refused, as are settings out of range and points that are not numbers; a refusal changes nothing (needs write access)."
    )]
    fn brush_stroke(&self, Parameters(args): Parameters<BrushStrokes>) -> Outcome {
        self.write(args.expected_revision, |state| {
            let taper = match (args.taper_start, args.taper_end) {
                (None, None) => None,
                (start, end) => Some((start.unwrap_or(0.0), end.unwrap_or(0.0))),
            };
            let request = BrushRequest {
                profile: args.profile.clone(),
                size: args.size.unwrap_or(12.0),
                color: color_or(args.color.as_deref(), Rgba8::opaque(0, 0, 0))?,
                flow: args.flow,
                stabilizer: args.stabilizer,
                taper,
                smoothing: args.smoothing,
            };
            let (stroke, paint) = request.resolve().map_err(|e| format!("Brush: {e}"))?;
            let strokes = args
                .strokes
                .iter()
                .enumerate()
                .map(|(i, points)| brush::stroke_samples(i, points))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| format!("Brush: {e}"))?;
            let target = match args.layer.as_deref() {
                Some(text) => flare_tools::ops::Target::Layer(target_layer(state, Some(text))?),
                None => flare_tools::ops::Target::Default,
            };
            let duration = match args.duration {
                Some(frames) => {
                    Some(NonZeroU32::new(frames).ok_or("Brush: duration must be at least 1")?)
                }
                None => None,
            };
            let placement = Placement {
                scene: first_scene(state)?,
                target,
                frame: args.frame.unwrap_or(0),
                duration,
                merge: args.merge.unwrap_or(false),
            };
            let plan = brush::lay_strokes(
                state.session.document(),
                placement,
                &stroke,
                paint,
                &strokes,
            )
            .map_err(|e| format!("Brush: {e}"))?;
            let elements: Vec<usize> = plan.draw.placed.iter().map(|at| at.element).collect();
            let layer = plan.draw.layer;
            let mut out = apply(state, plan.draw.commands)?;
            out["drawn"] = json!(elements.len());
            out["layer"] = json!(layer.as_uuid());
            out["elements"] = json!(elements);
            out["skipped"] = json!(plan.skipped);
            Ok(out)
        })
    }
}
