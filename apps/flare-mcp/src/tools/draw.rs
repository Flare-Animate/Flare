//! Tools that add things to the project.

use std::num::NonZeroU32;

use flare_commands::Command;
use flare_core::{InstanceId, LayerId, Rgba8, SymbolId, Transform2D};
use flare_document::{Content, FrameSpan, Instance, Layer, Stage, Symbol, Timeline, Tween};
use glamx::{DAffine2, DVec2};
use rmcp::{handler::server::wrapper::Parameters, tool, tool_router};
use serde_json::json;

use crate::ops::{apply, color_or, duration, first_scene, parse_id, rect_shape, target_layer};
use crate::params::{
    AddLayer, AddSymbol, DrawRect, PlaceInstance, RenameScene, SetCamera, SetStage,
};
use crate::server::{FlareServer, Outcome};

#[tool_router(router = draw_tools, vis = "pub(super)")]
impl FlareServer {
    #[tool(
        description = "Draw a filled rectangle into a layer of the first scene, as the desktop app does: it joins the layer's keyframe at `start` as one more element, or starts a keyframe of `duration` frames there. The layer is the one given, else the topmost that is not a folder or the camera; a project with none gets \"Layer 1\". Locked, hidden, folder, mask, guide and camera layers, and keyframes that tween, are refused (needs write access)."
    )]
    fn draw_rect(&self, Parameters(args): Parameters<DrawRect>) -> Outcome {
        self.write(args.expected_revision, |state| {
            let scene = first_scene(state)?;
            let target = match args.layer.as_deref() {
                Some(text) => flare_tools::ops::Target::Layer(target_layer(state, Some(text))?),
                None => flare_tools::ops::Target::Default,
            };
            let plan = flare_tools::ops::draw_shapes(
                state.session.document(),
                scene,
                target,
                args.start.unwrap_or(0),
                Some(duration(args.duration)?),
                vec![args.shape()?],
                "Layer 1",
            )
            .map_err(|e| format!("cannot draw: {e}"))?;
            let element = plan.placed.first().map_or(0, |at| at.element);
            let mut out = apply(state, plan.commands)?;
            out["layer"] = json!(plan.layer.as_uuid());
            out["element"] = json!(element);
            Ok(out)
        })
    }

    #[tool(description = "Add an empty layer below the existing ones (needs write access).")]
    fn add_layer(&self, Parameters(args): Parameters<AddLayer>) -> Outcome {
        self.write(args.expected_revision, |state| {
            let scene = first_scene(state)?;
            let id = LayerId::new();
            let layer = Layer::new(id, &args.name, vec![]).map_err(|e| e.to_string())?;
            let mut out = apply(state, vec![Command::AddLayer { scene, layer }])?;
            out["layer"] = json!(id.as_uuid());
            Ok(out)
        })
    }

    #[tool(
        description = "Add a Graphic symbol, optionally holding one rectangle (needs write access)."
    )]
    fn add_symbol(&self, Parameters(args): Parameters<AddSymbol>) -> Outcome {
        self.write(args.expected_revision, |state| {
            let id = SymbolId::new();
            let timeline = match args.width {
                Some(width) => {
                    let rect = (
                        args.x.unwrap_or(0.0),
                        args.y.unwrap_or(0.0),
                        width,
                        args.height.ok_or("height is required with width")?,
                    );
                    let shape = rect_shape(rect, args.color.as_deref(), None)?;
                    let span = FrameSpan::new(0, NonZeroU32::MIN, Content::Shape(shape));
                    let layer = Layer::new(LayerId::new(), "Layer 1", vec![span])
                        .map_err(|e| e.to_string())?;
                    Timeline::new(vec![layer])
                }
                None => Timeline::default(),
            };
            let symbol = Symbol::new(id, &args.name, timeline);
            let mut out = apply(state, vec![Command::AddSymbol(symbol)])?;
            out["symbol"] = json!(id.as_uuid());
            Ok(out)
        })
    }

    #[tool(description = "Place a symbol instance on a layer (needs write access).")]
    fn place_instance(&self, Parameters(args): Parameters<PlaceInstance>) -> Outcome {
        self.write(args.expected_revision, |state| {
            let symbol = parse_id(&args.symbol, "symbol")?;
            let (scene, layer) = (
                first_scene(state)?,
                target_layer(state, args.layer.as_deref())?,
            );
            let at = DVec2::new(args.x.unwrap_or(0.0), args.y.unwrap_or(0.0));
            let transform =
                Transform2D::new(DAffine2::from_translation(at)).map_err(|e| e.to_string())?;
            let instance = Instance::new(InstanceId::new(), SymbolId::from_uuid(symbol), transform);
            let mut span = FrameSpan::new(
                args.start.unwrap_or(0),
                duration(args.duration)?,
                Content::Instance(instance.clone()),
            );
            if args.tween == Some(true) {
                span = span.with_tween(Tween::Linear);
            }
            let mut out = apply(state, vec![Command::InsertSpan { scene, layer, span }])?;
            out["instance"] = json!(instance.id().as_uuid());
            Ok(out)
        })
    }

    #[tool(
        description = "Change the stage width, height, background and/or the frame rate (needs write access)."
    )]
    fn set_stage(&self, Parameters(args): Parameters<SetStage>) -> Outcome {
        self.write(args.expected_revision, |state| {
            let current = state.session.document().stage();
            let width = args.width.unwrap_or(current.width());
            let height = args.height.unwrap_or(current.height());
            let background: Rgba8 = color_or(args.background.as_deref(), current.background())?;
            let stage = Stage::new(width, height, background).map_err(|e| e.to_string())?;
            let mut commands = vec![Command::SetStage(stage)];
            if let Some(fps) = args.fps {
                let rate =
                    flare_core::FrameRate::from_per_second(fps).map_err(|e| e.to_string())?;
                commands.push(Command::SetFrameRate(rate));
            }
            apply(state, commands)
        })
    }

    #[tool(description = "Rename the first scene (needs write access).")]
    fn rename_scene(&self, Parameters(args): Parameters<RenameScene>) -> Outcome {
        self.write(args.expected_revision, |state| {
            let scene = first_scene(state)?;
            apply(
                state,
                vec![Command::RenameScene {
                    scene,
                    name: args.name,
                }],
            )
        })
    }

    #[tool(
        description = "Point the first scene's camera from a frame on, making the camera layer if there is none; what is not given stays what the camera has there (needs write access)."
    )]
    fn set_camera(&self, Parameters(args): Parameters<SetCamera>) -> Outcome {
        self.write(args.expected_revision, |state| {
            use flare_document::{Camera, Tween};
            let scene = first_scene(state)?;
            let document = state.session.document();
            let now = flare_editor::camera_now(document, scene, args.frame)
                .ok_or("the document has no scene")?;
            let point = flare_core::DocumentPoint::new(
                args.x.unwrap_or(now.center().as_vec().x),
                args.y.unwrap_or(now.center().as_vec().y),
            )
            .map_err(|e| e.to_string())?;
            let camera = Camera::new(
                point,
                args.zoom.unwrap_or(now.zoom()),
                args.rotation.map_or(now.rotation(), f64::to_radians),
            )
            .map_err(|e| e.to_string())?;
            let kind = match args.motion.as_deref() {
                None => None,
                Some("motion") => Some(Tween::Linear),
                Some("none") => Some(Tween::None),
                Some(other) => {
                    return Err(format!(
                        "motion must be \"motion\" or \"none\", not {other:?}"
                    ));
                }
            };
            let motion = match (kind, args.ease) {
                (None, None) => None,
                (kind, ease) => Some((kind.unwrap_or(Tween::Linear), ease.unwrap_or(0))),
            };
            let commands =
                flare_editor::set_camera_key(document, scene, args.frame, camera, motion)
                    .ok_or_else(|| {
                        format!(
                            "frame {} is past the last frame a camera can be keyed on, {}",
                            args.frame,
                            flare_editor::MAX_CAMERA_FRAME
                        )
                    })?;
            apply(state, commands)
        })
    }
}
