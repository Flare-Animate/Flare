//! What the tools share: finding things in the document and applying commands.

use std::num::NonZeroU32;

use flare_commands::Command;
use flare_core::{LayerId, Rgba8, SceneId, ShapeId};
use flare_document::{Shape, Stroke};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::color::parse_color;
use crate::params::DrawRect;
use crate::server::State;

pub fn first_scene(state: &State) -> Result<SceneId, String> {
    state
        .session
        .document()
        .scenes()
        .first()
        .map(|s| s.id())
        .ok_or_else(|| "the document has no scene".to_owned())
}

pub fn parse_id(text: &str, what: &str) -> Result<Uuid, String> {
    Uuid::parse_str(text).map_err(|_| format!("{what} must be a UUID"))
}

/// The layer named by `layer`, or the first one.
pub fn target_layer(state: &State, layer: Option<&str>) -> Result<LayerId, String> {
    let scene = state
        .session
        .document()
        .scenes()
        .first()
        .ok_or("the document has no scene")?;
    let layers = scene.timeline().layers();
    let found = match layer {
        Some(text) => {
            let id = parse_id(text, "layer")?;
            layers
                .iter()
                .find(|l| *l.id().as_uuid() == id)
                .ok_or_else(|| format!("no layer {id} in the first scene"))?
        }
        None => layers.first().ok_or("the scene has no layer")?,
    };
    Ok(found.id())
}

/// Applies `commands` as one undo step and reports the new revision.
pub fn apply(state: &mut State, commands: Vec<Command>) -> Result<Value, String> {
    state.session.apply(commands).map_err(|e| e.to_string())?;
    Ok(json!({ "revision": state.session.revision() }))
}

pub fn duration(frames: Option<u32>) -> Result<NonZeroU32, String> {
    NonZeroU32::new(frames.unwrap_or(1)).ok_or_else(|| "duration must be at least 1".to_owned())
}

pub fn color_or(text: Option<&str>, default: Rgba8) -> Result<Rgba8, String> {
    text.map_or(Ok(default), parse_color)
}

/// The rectangle a `draw_rect` or `add_symbol` call describes.
pub fn rect_shape(
    (x, y, w, h): (f64, f64, f64, f64),
    color: Option<&str>,
    outline: Option<(&str, f64)>,
) -> Result<Shape, String> {
    if !(w > 0.0 && h > 0.0) {
        return Err("width and height must be positive".to_owned());
    }
    let fill = color_or(color, Rgba8::opaque(0, 0, 0))?;
    let mut shape = Shape::rect(ShapeId::new(), x, y, w, h, fill)
        .ok_or("rectangle coordinates must be finite")?;
    if let Some((stroke, width)) = outline {
        let stroke = Stroke::new(width, parse_color(stroke)?).map_err(|e| e.to_string())?;
        shape = shape.with_stroke(stroke);
    }
    Ok(shape)
}

impl DrawRect {
    pub fn shape(&self) -> Result<Shape, String> {
        let outline = self
            .stroke
            .as_deref()
            .map(|s| (s, self.stroke_width.unwrap_or(2.0)));
        rect_shape(
            (self.x, self.y, self.width, self.height),
            self.color.as_deref(),
            outline,
        )
    }
}
