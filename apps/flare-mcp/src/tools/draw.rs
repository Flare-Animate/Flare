//! Tools that add things to the project.

use std::num::NonZeroU32;

use flare_core::{InstanceId, LayerId, Rgba8, SymbolId, Transform2D};
use flare_document::{Content, FrameSpan, Instance, Layer, Stage, Symbol, Timeline, Tween};
use flare_edit::Command;
use glamx::{DAffine2, DVec2};
use rmcp::{handler::server::wrapper::Parameters, tool, tool_router};
use serde_json::json;

use crate::ops::{apply, color_or, duration, first_scene, parse_id, rect_shape, target_layer};
use crate::params::{AddLayer, AddSymbol, DrawRect, PlaceInstance, SetStage};
use crate::server::{FlareServer, Outcome};

#[tool_router(router = draw_tools, vis = "pub(super)")]
impl FlareServer {
    #[tool(
        description = "Draw a filled rectangle on a layer of the first scene (needs write access)."
    )]
    fn draw_rect(&self, Parameters(args): Parameters<DrawRect>) -> Outcome {
        self.write(args.expected_revision, |state| {
            let (scene, layer) = (
                first_scene(state)?,
                target_layer(state, args.layer.as_deref())?,
            );
            let span = FrameSpan::new(
                args.start.unwrap_or(0),
                duration(args.duration)?,
                Content::Shape(args.shape()?),
            );
            apply(state, vec![Command::InsertSpan { scene, layer, span }])
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

    #[tool(description = "Change the stage width, height and/or background (needs write access).")]
    fn set_stage(&self, Parameters(args): Parameters<SetStage>) -> Outcome {
        self.write(args.expected_revision, |state| {
            let current = state.session.document().stage();
            let width = args.width.unwrap_or(current.width());
            let height = args.height.unwrap_or(current.height());
            let background: Rgba8 = color_or(args.background.as_deref(), current.background())?;
            let stage = Stage::new(width, height, background).map_err(|e| e.to_string())?;
            apply(state, vec![Command::SetStage(stage)])
        })
    }
}
