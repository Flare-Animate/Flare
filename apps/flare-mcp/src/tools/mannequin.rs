//! The reference mannequin: what a figure can be, a picture of one, and placing one on a layer
//! to draw over.

use flare_mannequin::{Look, Mannequin, Placement, Request};
use rmcp::{handler::server::wrapper::Parameters, tool, tool_router};
use serde_json::json;

use super::export::{export_dir, fresh};
use crate::color::safe_file_name_with;
use crate::ops::{apply, first_scene, target_layer};
use crate::params_mannequin::{PictureMannequin, PlaceMannequin};
use crate::server::{FlareServer, Outcome};

/// Longest layer name taken.
const MAX_NAME: usize = 256;

/// What the figure is asked to be, as both tools take it.
struct Ask {
    figure: Option<serde_json::Value>,
    body: Option<String>,
    preset: Option<String>,
    joints: Option<std::collections::BTreeMap<String, [f64; 3]>>,
    reach: Option<std::collections::BTreeMap<String, [f64; 3]>>,
    yaw: Option<f64>,
    pitch: Option<f64>,
    fov: Option<f64>,
}

impl Ask {
    fn build(self) -> Result<Mannequin, String> {
        Request {
            figure: self.figure.as_ref().map(serde_json::Value::to_string),
            body: self.body,
            preset: self.preset,
            pose: None,
            joints: self.joints.unwrap_or_default().into_iter().collect(),
            reach: self.reach.unwrap_or_default().into_iter().collect(),
            yaw: self.yaw,
            pitch: self.pitch,
            fov: self.fov,
        }
        .build()
        .map_err(|e| e.to_string())
    }
}

/// The figure standing in a picture `width` by `height`, nine tenths as tall as it.
fn picture_png(
    figure: &Mannequin,
    width: u32,
    height: u32,
    guides: bool,
) -> Result<Vec<u8>, String> {
    let (w, h) = (f64::from(width), f64::from(height));
    let tall = (h * 0.9).min(w * 1.6);
    let placement = Placement::new([w / 2.0, h / 2.0], tall)
        .ok_or_else(|| "the picture is too small".to_owned())?;
    let look = Look {
        guides,
        ..Look::default()
    };
    let picture = figure
        .render(placement, width, height, look)
        .ok_or_else(|| "the picture is too large".to_owned())?;
    flare_runtime::rgba_png(width, height, picture.rgba())
}

#[tool_router(router = mannequin_tools, vis = "pub(super)")]
impl FlareServer {
    #[tool(
        description = "List what the reference mannequin can be: its bodies, built-in poses, proportions with their ranges, joints with how far each bends, twists and swings (degrees), and the hands and feet that can reach for a point. Includes an example figure."
    )]
    fn list_mannequin(&self) -> Outcome {
        self.read(|_| {
            let mut catalog = flare_mannequin::catalog();
            let example: serde_json::Value =
                serde_json::from_str(&Mannequin::default().to_json()).map_err(|e| e.to_string())?;
            catalog["example"] = example;
            Ok(catalog)
        })
    }

    #[tool(
        description = "Picture the posed reference mannequin as a shaded 3D figure (guide lines, outline and floor shadow, see-through elsewhere) in a PNG in the export directory, to look at a pose before placing it (disabled when no export directory is configured)."
    )]
    fn picture_mannequin(&self, Parameters(args): Parameters<PictureMannequin>) -> Outcome {
        self.read(|state| {
            let figure = Ask {
                figure: args.figure,
                body: args.body,
                preset: args.preset,
                joints: args.joints,
                reach: args.reach,
                yaw: args.yaw,
                pitch: args.pitch,
                fov: args.fov,
            }
            .build()?;
            let (width, height) = (args.width.unwrap_or(600), args.height.unwrap_or(800));
            let sides = 16..=flare_mannequin::MAX_PICTURE_SIDE;
            if !(sides.contains(&width) && sides.contains(&height)) {
                return Err(format!(
                    "the picture's width and height are each 16 to {}",
                    sides.end()
                ));
            }
            let dir = export_dir(state)?;
            let out = fresh(&dir, &safe_file_name_with(&args.name, "png")?)?;
            let bytes = picture_png(&figure, width, height, args.guides.unwrap_or(true))?;
            std::fs::write(&out, &bytes).map_err(|e| e.to_string())?;
            Ok(json!({ "path": out.to_string_lossy(), "width": width, "height": height }))
        })
    }

    #[tool(
        description = "Pose the reference mannequin and place it as a shaded picture of the 3D figure to draw over (x, y and height say where and how large): on a new layer at the top of the first scene, or with `layer` into that layer's keyframe at `frame` beside the figures already there, or with `new_keyframe` as a keyframe of its own from `frame` (shown half see-through while editing; hide or delete the layer before exporting). One undo step (needs write access)."
    )]
    fn place_mannequin(&self, Parameters(args): Parameters<PlaceMannequin>) -> Outcome {
        self.write(args.expected_revision, |state| {
            let figure = Ask {
                figure: args.figure,
                body: args.body,
                preset: args.preset,
                joints: args.joints,
                reach: args.reach,
                yaw: args.yaw,
                pitch: args.pitch,
                fov: args.fov,
            }
            .build()?;
            let name = args.name.unwrap_or_else(|| "Mannequin".to_owned());
            if name.trim().is_empty() || name.chars().count() > MAX_NAME {
                return Err(format!(
                    "the layer needs a name of 1 to {MAX_NAME} characters"
                ));
            }
            let scene = first_scene(state)?;
            let stage = state.session.document().stage();
            let (w, h) = (f64::from(stage.width()), f64::from(stage.height()));
            let placement = Placement::new(
                [args.x.unwrap_or(w / 2.0), args.y.unwrap_or(h / 2.0)],
                args.height.unwrap_or(h * 0.8),
            )
            .ok_or_else(|| {
                format!(
                    "x and y must be numbers and height from above 0 to {}",
                    flare_mannequin::MAX_PLACEMENT_HEIGHT
                )
            })?;
            let frame = args.frame.unwrap_or(0);
            let document = state.session.document();
            let placed = match args.layer.as_deref() {
                None if args.new_keyframe.unwrap_or(false) => {
                    return Err("new_keyframe needs the layer to place it on".to_owned());
                }
                None => {
                    flare_tools::mannequin::place(document, scene, frame, &figure, placement, &name)
                }
                Some(text) => {
                    let layer = target_layer(state, Some(text))?;
                    let keyframe = if args.new_keyframe.unwrap_or(false) {
                        flare_tools::mannequin::Keyframe::New
                    } else {
                        flare_tools::mannequin::Keyframe::Join
                    };
                    flare_tools::mannequin::place_onto(
                        state.session.document(),
                        scene,
                        frame,
                        &figure,
                        placement,
                        layer,
                        keyframe,
                        &name,
                    )
                }
            }
            .map_err(|e| format!("cannot place the mannequin: {e}"))?;
            let mut out = apply(state, placed.commands)?;
            out["layer"] = json!(placed.layer.as_uuid());
            out["bitmap"] = json!({ "width": placed.size.0, "height": placed.size.1 });
            out["keyframe"] = json!(placed.at.start);
            out["element"] = json!(placed.at.element);
            Ok(out)
        })
    }
}
