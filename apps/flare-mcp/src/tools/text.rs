//! Tools for text objects: words set in a font that stay editable.

use flare_commands::Command;
use flare_core::{DocumentPoint, SceneId, ShapeId};
use flare_document::{Content, FrameSpan, Shape, TextSource};
use flare_tools::ops::{SpanRef, break_text_apart, replace_shape};
use rmcp::{handler::server::wrapper::Parameters, tool, tool_router};
use serde_json::json;

use crate::color::parse_color;
use crate::ops::{apply, duration, first_scene, parse_id, target_layer};
use crate::params::{AddText, BreakText, SetText};
use crate::server::{FlareServer, Outcome, State};

fn origin(x: f64, y: f64) -> Result<DocumentPoint, String> {
    DocumentPoint::new(x, y).map_err(|_| "the position must be finite".to_owned())
}

/// The text object `shape` in the first scene, and where it is.
fn find_text(state: &State, shape: &str) -> Result<(SceneId, SpanRef, Shape), String> {
    let id = parse_id(shape, "shape")?;
    let scene = state
        .session
        .document()
        .scenes()
        .first()
        .ok_or("the document has no scene")?;
    for layer in scene.timeline().layers() {
        for span in layer.spans() {
            if let Content::Shape(found) = span.content()
                && *found.id().as_uuid() == id
            {
                if found.text().is_none() {
                    return Err(format!("shape {id} is not text"));
                }
                let at = SpanRef {
                    layer: layer.id(),
                    start: span.start(),
                    element: 0,
                };
                return Ok((scene.id(), at, found.clone()));
            }
        }
    }
    Err(format!("no shape {id} in the first scene"))
}

#[tool_router(router = text_tools, vis = "pub(super)")]
impl FlareServer {
    #[tool(
        description = "Add editable text on a layer of the first scene. It is drawn as outlines and remembers its words and font, so set_text can change them (needs write access)."
    )]
    fn add_text(&self, Parameters(args): Parameters<AddText>) -> Outcome {
        self.write(args.expected_revision, |state| {
            let (scene, layer) = (
                first_scene(state)?,
                target_layer(state, args.layer.as_deref())?,
            );
            let source = TextSource::new(
                args.text,
                args.family
                    .unwrap_or_else(|| flare_text::DEFAULT_FAMILY.to_owned()),
                args.size.unwrap_or(24.0),
                args.bold.unwrap_or(false),
                args.italic.unwrap_or(false),
                origin(args.x, args.y)?,
            )
            .map_err(|e| e.to_string())?;
            let fill = args
                .color
                .as_deref()
                .map_or(Ok(flare_core::Rgba8::opaque(0, 0, 0)), parse_color)?;
            let id = ShapeId::new();
            let shape = flare_runtime::text_shape(&source, id, fill, &state.fonts)?;
            let span = FrameSpan::new(
                args.start.unwrap_or(0),
                duration(args.duration)?,
                Content::Shape(shape),
            );
            let mut out = apply(state, vec![Command::InsertSpan { scene, layer, span }])?;
            out["shape"] = json!(id.as_uuid());
            Ok(out)
        })
    }

    #[tool(
        description = "Change a text object: its words, font, size, place or color. What is not given stays as it was (needs write access)."
    )]
    fn set_text(&self, Parameters(args): Parameters<SetText>) -> Outcome {
        self.write(args.expected_revision, |state| {
            let (scene, at, old) = find_text(state, &args.shape)?;
            let before = old.text().ok_or("the shape is not text")?;
            let was = before.origin().as_vec();
            let source = TextSource::new(
                args.text.unwrap_or_else(|| before.text().to_owned()),
                args.family.unwrap_or_else(|| before.family().to_owned()),
                args.size.unwrap_or(before.size()),
                args.bold.unwrap_or(before.bold()),
                args.italic.unwrap_or(before.italic()),
                origin(args.x.unwrap_or(was.x), args.y.unwrap_or(was.y))?,
            )
            .map_err(|e| e.to_string())?;
            let fill = match args.color.as_deref() {
                Some(color) => parse_color(color)?,
                None => old.fill(),
            };
            let new = flare_runtime::text_shape(&source, old.id(), fill, &state.fonts)?
                .painted_like(&old);
            let commands = replace_shape(state.session.document(), scene, at, new)
                .ok_or("nothing about the text changed")?;
            apply(state, commands)
        })
    }

    #[tool(
        description = "Take text apart into plain outlines, which can then be edited point by point (needs write access)."
    )]
    fn break_text(&self, Parameters(args): Parameters<BreakText>) -> Outcome {
        self.write(args.expected_revision, |state| {
            let (scene, at, _) = find_text(state, &args.shape)?;
            let commands = break_text_apart(state.session.document(), scene, at)
                .ok_or("the shape is not text")?;
            apply(state, commands)
        })
    }

    #[tool(description = "List the font families text can be set in.")]
    fn list_fonts(&self) -> Outcome {
        self.read(|state| Ok(json!({ "families": flare_runtime::font_families(&state.fonts) })))
    }
}
