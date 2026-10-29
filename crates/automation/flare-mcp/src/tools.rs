//! The tools an agent can call. Each is a thin layer over the same edit commands, frame
//! operations and renderer that the CLI and desktop app use.

use std::num::NonZeroU32;

use flare_core::{InstanceId, LayerId, Rgba8, ShapeId, SymbolId, Transform2D};
use flare_document::{
    Content, FrameSpan, Instance, Layer, Shape, Stage, Stroke, Symbol, Timeline, Tween,
};
use flare_edit::{Command, LayerChange, frames};
use glamx::{DAffine2, DVec2};
use serde_json::{Map, Value, json};
use uuid::Uuid;

use crate::{Permissions, Server};

type Args = Map<String, Value>;
type Outcome = Result<Value, String>;

/// Tools that change the document or write the project file.
const WRITE_TOOLS: &[&str] = &[
    "draw_rect",
    "add_layer",
    "add_symbol",
    "place_instance",
    "frame_operation",
    "layer_operation",
    "set_stage",
    "undo",
    "redo",
    "save",
];

const READ_TOOLS: &[&str] = &["get_info", "evaluate_frame", "export_png"];

pub fn exists(name: &str) -> bool {
    WRITE_TOOLS.contains(&name) || READ_TOOLS.contains(&name)
}

fn schema(properties: Value, required: &[&str]) -> Value {
    json!({ "type": "object", "properties": properties, "required": required })
}

fn tool(name: &str, description: &str, input: Value) -> Value {
    json!({ "name": name, "description": description, "inputSchema": input })
}

pub fn definitions(permissions: &Permissions) -> Value {
    let revision = json!({ "type": "integer", "minimum": 0,
        "description": "Fail unless the project is at this revision (from get_info)." });
    let layer =
        json!({ "type": "string", "description": "Layer id; defaults to the first layer." });
    let color = json!({ "type": "string", "description": "RRGGBB or RRGGBBAA hex." });
    let write = if permissions.write {
        ""
    } else {
        " (needs write access; this server is read-only)"
    };
    let export = if permissions.export_dir.is_some() {
        ""
    } else {
        " (disabled: no export directory configured)"
    };
    Value::Array(vec![
        tool(
            "get_info",
            "Summarize the project: revision, stage, scenes, layers and symbols.",
            schema(json!({}), &[]),
        ),
        tool(
            "evaluate_frame",
            "List the instances and shapes visible at a frame of the first scene.",
            schema(
                json!({ "frame": { "type": "integer", "minimum": 0 } }),
                &["frame"],
            ),
        ),
        tool(
            "export_png",
            &format!("Render a frame to a PNG in the export directory{export}."),
            schema(
                json!({ "frame": { "type": "integer", "minimum": 0 },
                        "name": { "type": "string", "description": "File name only, letters, digits, . _ -" },
                        "width": { "type": "integer" }, "height": { "type": "integer" } }),
                &["frame", "name"],
            ),
        ),
        tool(
            "draw_rect",
            &format!("Draw a filled rectangle on a layer of the first scene{write}."),
            schema(
                json!({ "x": {"type":"number"}, "y": {"type":"number"}, "width": {"type":"number"},
                        "height": {"type":"number"}, "color": color, "stroke": color,
                        "stroke_width": {"type":"number"}, "start": {"type":"integer"},
                        "duration": {"type":"integer"}, "layer": layer, "expected_revision": revision }),
                &["x", "y", "width", "height"],
            ),
        ),
        tool(
            "add_layer",
            &format!("Add an empty layer below the existing ones{write}."),
            schema(
                json!({ "name": {"type":"string"}, "expected_revision": revision }),
                &["name"],
            ),
        ),
        tool(
            "add_symbol",
            &format!("Add a Graphic symbol, optionally holding one rectangle{write}."),
            schema(
                json!({ "name": {"type":"string"}, "x": {"type":"number"}, "y": {"type":"number"},
                        "width": {"type":"number"}, "height": {"type":"number"}, "color": color,
                        "expected_revision": revision }),
                &["name"],
            ),
        ),
        tool(
            "place_instance",
            &format!("Place a symbol instance on a layer{write}."),
            schema(
                json!({ "symbol": {"type":"string"}, "x": {"type":"number"}, "y": {"type":"number"},
                        "start": {"type":"integer"}, "duration": {"type":"integer"},
                        "tween": {"type":"boolean"}, "layer": layer, "expected_revision": revision }),
                &["symbol"],
            ),
        ),
        tool(
            "frame_operation",
            &format!("Insert or remove frames and keyframes on a layer{write}."),
            schema(
                json!({ "operation": { "type":"string", "enum": ["insert_frame","remove_frame",
                        "insert_keyframe","insert_blank_keyframe","clear_keyframe"] },
                        "at": {"type":"integer","minimum":0}, "layer": layer, "expected_revision": revision }),
                &["operation", "at"],
            ),
        ),
        tool(
            "layer_operation",
            &format!("Rename, show/hide, lock/unlock, reorder or delete a layer{write}."),
            schema(
                json!({ "operation": { "type":"string", "enum": ["rename","delete","hide","show",
                        "lock","unlock","move_up","move_down"] },
                        "layer": layer, "name": {"type":"string"}, "expected_revision": revision }),
                &["operation"],
            ),
        ),
        tool(
            "set_stage",
            &format!("Change the stage width, height and/or background{write}."),
            schema(
                json!({ "width": {"type":"integer"}, "height": {"type":"integer"},
                        "background": color, "expected_revision": revision }),
                &[],
            ),
        ),
        tool(
            "undo",
            &format!("Undo the last change{write}."),
            schema(json!({ "expected_revision": revision }), &[]),
        ),
        tool(
            "redo",
            &format!("Redo the last undone change{write}."),
            schema(json!({ "expected_revision": revision }), &[]),
        ),
        tool(
            "save",
            &format!("Save the project file{write}."),
            schema(json!({ "expected_revision": revision }), &[]),
        ),
    ])
}

// ---- argument helpers ---------------------------------------------------------------

fn number(args: &Args, key: &str) -> Result<Option<f64>, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => match value.as_f64() {
            Some(n) if n.is_finite() => Ok(Some(n)),
            _ => Err(format!("{key} must be a finite number")),
        },
    }
}

fn required_number(args: &Args, key: &str) -> Result<f64, String> {
    number(args, key)?.ok_or_else(|| format!("{key} is required"))
}

fn uint(args: &Args, key: &str) -> Result<Option<u32>, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .map(Some)
            .ok_or_else(|| format!("{key} must be a non-negative integer that fits 32 bits")),
    }
}

fn text<'a>(args: &'a Args, key: &str) -> Result<Option<&'a str>, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s)),
        Some(_) => Err(format!("{key} must be a string")),
    }
}

fn required_text<'a>(args: &'a Args, key: &str) -> Result<&'a str, String> {
    text(args, key)?.ok_or_else(|| format!("{key} is required"))
}

fn uuid(args: &Args, key: &str) -> Result<Option<Uuid>, String> {
    text(args, key)?
        .map(|s| Uuid::parse_str(s).map_err(|_| format!("{key} must be a UUID")))
        .transpose()
}

pub fn parse_color(text: &str) -> Result<Rgba8, String> {
    let hex = text.trim_start_matches('#');
    let byte = |i: usize| {
        hex.get(i..i + 2)
            .and_then(|pair| u8::from_str_radix(pair, 16).ok())
            .ok_or_else(|| format!("bad color {text:?}, expected RRGGBB or RRGGBBAA"))
    };
    match hex.len() {
        6 => Ok(Rgba8::opaque(byte(0)?, byte(2)?, byte(4)?)),
        8 => Ok(Rgba8::new(byte(0)?, byte(2)?, byte(4)?, byte(6)?)),
        _ => Err(format!("bad color {text:?}, expected RRGGBB or RRGGBBAA")),
    }
}

fn color_arg(args: &Args, key: &str, default: Rgba8) -> Result<Rgba8, String> {
    text(args, key)?.map_or(Ok(default), parse_color)
}

// ---- document helpers ---------------------------------------------------------------

fn first_scene(server: &Server) -> Result<flare_core::SceneId, String> {
    server
        .session
        .document()
        .scenes()
        .first()
        .map(|s| s.id())
        .ok_or_else(|| "the document has no scene".to_owned())
}

fn target_layer(server: &Server, layer: Option<Uuid>) -> Result<LayerId, String> {
    let scene = server
        .session
        .document()
        .scenes()
        .first()
        .ok_or("the document has no scene")?;
    let layers = scene.timeline().layers();
    let found = match layer {
        Some(id) => layers
            .iter()
            .find(|l| *l.id().as_uuid() == id)
            .ok_or_else(|| format!("no layer {id} in the first scene"))?,
        None => layers.first().ok_or("the scene has no layer")?,
    };
    Ok(found.id())
}

fn apply(server: &mut Server, commands: Vec<Command>) -> Outcome {
    server.session.apply(commands).map_err(|e| e.to_string())?;
    Ok(json!({ "revision": server.session.revision() }))
}

fn duration(args: &Args) -> Result<NonZeroU32, String> {
    let n = uint(args, "duration")?.unwrap_or(1);
    NonZeroU32::new(n).ok_or_else(|| "duration must be at least 1".to_owned())
}

fn rect_shape(args: &Args) -> Result<Shape, String> {
    let (x, y) = (required_number(args, "x")?, required_number(args, "y")?);
    let (w, h) = (
        required_number(args, "width")?,
        required_number(args, "height")?,
    );
    if !(w > 0.0 && h > 0.0) {
        return Err("width and height must be positive".to_owned());
    }
    let fill = color_arg(args, "color", Rgba8::opaque(0, 0, 0))?;
    let mut shape = Shape::rect(ShapeId::new(), x, y, w, h, fill)
        .ok_or("rectangle coordinates must be finite")?;
    if let Some(stroke) = text(args, "stroke")? {
        let width = number(args, "stroke_width")?.unwrap_or(2.0);
        shape =
            shape.with_stroke(Stroke::new(width, parse_color(stroke)?).map_err(|e| e.to_string())?);
    }
    Ok(shape)
}

// ---- dispatch -----------------------------------------------------------------------

pub fn call(server: &mut Server, name: &str, args: &Args) -> Outcome {
    if WRITE_TOOLS.contains(&name) {
        if !server.permissions.write {
            return Err(
                "this server is read-only; restart it with write access to change the project"
                    .to_owned(),
            );
        }
        if let Some(expected) = args.get("expected_revision") {
            let expected = expected
                .as_u64()
                .ok_or("expected_revision must be a non-negative integer")?;
            let current = server.session.revision();
            if expected != current {
                return Err(format!(
                    "revision mismatch: expected {expected} but the project is at {current}; call get_info and re-read before editing"
                ));
            }
        }
    }
    match name {
        "get_info" => get_info(server),
        "evaluate_frame" => evaluate(server, args),
        "export_png" => export_png(server, args),
        "draw_rect" => {
            let (scene, layer) = (
                first_scene(server)?,
                target_layer(server, uuid(args, "layer")?)?,
            );
            let start = uint(args, "start")?.unwrap_or(0);
            let span = FrameSpan::new(start, duration(args)?, Content::Shape(rect_shape(args)?));
            apply(server, vec![Command::InsertSpan { scene, layer, span }])
        }
        "add_layer" => {
            let scene = first_scene(server)?;
            let id = LayerId::new();
            let layer =
                Layer::new(id, required_text(args, "name")?, vec![]).map_err(|e| e.to_string())?;
            let mut out = apply(server, vec![Command::AddLayer { scene, layer }])?;
            out["layer"] = json!(id.as_uuid());
            Ok(out)
        }
        "add_symbol" => {
            let id = SymbolId::new();
            let timeline = if args.contains_key("width") {
                let span = FrameSpan::new(0, NonZeroU32::MIN, Content::Shape(rect_shape(args)?));
                let layer =
                    Layer::new(LayerId::new(), "Layer 1", vec![span]).map_err(|e| e.to_string())?;
                Timeline::new(vec![layer])
            } else {
                Timeline::default()
            };
            let symbol = Symbol::new(id, required_text(args, "name")?, timeline);
            let mut out = apply(server, vec![Command::AddSymbol(symbol)])?;
            out["symbol"] = json!(id.as_uuid());
            Ok(out)
        }
        "place_instance" => {
            let symbol = uuid(args, "symbol")?.ok_or("symbol is required")?;
            let (scene, layer) = (
                first_scene(server)?,
                target_layer(server, uuid(args, "layer")?)?,
            );
            let at = DVec2::new(
                number(args, "x")?.unwrap_or(0.0),
                number(args, "y")?.unwrap_or(0.0),
            );
            let transform =
                Transform2D::new(DAffine2::from_translation(at)).map_err(|e| e.to_string())?;
            let instance = Instance::new(InstanceId::new(), SymbolId::from_uuid(symbol), transform);
            let mut span = FrameSpan::new(
                uint(args, "start")?.unwrap_or(0),
                duration(args)?,
                Content::Instance(instance.clone()),
            );
            if args.get("tween").and_then(Value::as_bool) == Some(true) {
                span = span.with_tween(Tween::Linear);
            }
            let mut out = apply(server, vec![Command::InsertSpan { scene, layer, span }])?;
            out["instance"] = json!(instance.id().as_uuid());
            Ok(out)
        }
        "frame_operation" => frame_operation(server, args),
        "layer_operation" => layer_operation(server, args),
        "set_stage" => {
            let current = server.session.document().stage();
            let width = uint(args, "width")?.unwrap_or(current.width());
            let height = uint(args, "height")?.unwrap_or(current.height());
            let background = color_arg(args, "background", current.background())?;
            let stage = Stage::new(width, height, background).map_err(|e| e.to_string())?;
            apply(server, vec![Command::SetStage(stage)])
        }
        "undo" | "redo" => {
            let changed = if name == "undo" {
                server.session.undo()
            } else {
                server.session.redo()
            };
            Ok(json!({ "changed": changed, "revision": server.session.revision() }))
        }
        "save" => {
            flare_storage::save(server.session.document(), &server.path)
                .map_err(|e| e.to_string())?;
            server.session.mark_saved();
            Ok(json!({ "saved": server.path.to_string_lossy() }))
        }
        other => Err(format!("unknown tool {other}")),
    }
}

fn get_info(server: &Server) -> Outcome {
    let document = server.session.document();
    let scenes: Vec<Value> = document
        .scenes()
        .iter()
        .map(|scene| {
            let layers: Vec<Value> = scene
                .timeline()
                .layers()
                .iter()
                .map(|l| {
                    json!({ "id": l.id().as_uuid(), "name": l.name(), "visible": l.is_visible(),
                            "locked": l.is_locked(), "spans": l.spans().len() })
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
        "project": server.path.to_string_lossy(),
        "revision": server.session.revision(),
        "dirty": server.session.is_dirty(),
        "can_undo": server.session.can_undo(),
        "can_redo": server.session.can_redo(),
        "frame_rate": { "numerator": rate.numerator(), "denominator": rate.denominator() },
        "stage": { "width": stage.width(), "height": stage.height(), "background": stage.background().to_array() },
        "scenes": scenes,
        "symbols": symbols,
    }))
}

fn evaluate(server: &Server, args: &Args) -> Outcome {
    let frame = uint(args, "frame")?.ok_or("frame is required")?;
    let scene = first_scene(server)?;
    let evaluated = flare_eval::evaluate_frame(server.session.document(), scene, frame)
        .map_err(|e| e.to_string())?;
    let ids = |path: &[InstanceId]| path.iter().map(|i| json!(i.as_uuid())).collect::<Vec<_>>();
    Ok(json!({
        "frame": frame,
        "placed": evaluated.placed.iter().map(|p| json!({
            "path": ids(&p.path), "symbol": p.symbol.as_uuid(), "matrix": p.transform.to_cols_array() })).collect::<Vec<_>>(),
        "shapes": evaluated.shapes.iter().map(|s| json!({
            "shape": s.shape.id().as_uuid(), "owner_path": ids(&s.path), "matrix": s.transform.to_cols_array() })).collect::<Vec<_>>(),
    }))
}

/// A file name that cannot escape the export directory.
pub fn safe_file_name(name: &str) -> Result<String, String> {
    let ok = !name.is_empty()
        && name.len() <= 100
        && !name.starts_with(['.', '-'])
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    if !ok {
        return Err("name must be 1-100 characters of letters, digits, '.', '_' or '-', not starting with '.' or '-'".to_owned());
    }
    Ok(if name.to_ascii_lowercase().ends_with(".png") {
        name.to_owned()
    } else {
        format!("{name}.png")
    })
}

fn export_png(server: &Server, args: &Args) -> Outcome {
    let dir = server
        .permissions
        .export_dir
        .clone()
        .ok_or("exports are disabled: no export directory was configured")?;
    let file = safe_file_name(required_text(args, "name")?)?;
    let frame = uint(args, "frame")?.ok_or("frame is required")?;
    let document = server.session.document();
    let stage = document.stage();
    let (width, height) = (
        uint(args, "width")?.unwrap_or(stage.width()),
        uint(args, "height")?.unwrap_or(stage.height()),
    );
    let evaluated = flare_eval::evaluate_frame(document, first_scene(server)?, frame)
        .map_err(|e| e.to_string())?;
    let image = flare_render::render(&evaluated, width, height, stage.background())
        .map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut staged = tempfile::NamedTempFile::new_in(&dir).map_err(|e| e.to_string())?;
    image
        .write_png(std::io::BufWriter::new(&mut staged))
        .map_err(|e| e.to_string())?;
    let out = dir.join(&file);
    staged.persist(&out).map_err(|e| e.error.to_string())?;
    Ok(json!({ "path": out.to_string_lossy(), "width": width, "height": height }))
}

fn frame_operation(server: &mut Server, args: &Args) -> Outcome {
    let scene = first_scene(server)?;
    let layer_id = target_layer(server, uuid(args, "layer")?)?;
    let at = uint(args, "at")?.ok_or("at is required")?;
    let operation = required_text(args, "operation")?;
    let layer = server
        .session
        .document()
        .scenes()
        .first()
        .and_then(|s| s.timeline().layers().iter().find(|l| l.id() == layer_id))
        .ok_or("layer disappeared")?;
    let commands = match operation {
        "insert_frame" => frames::insert_frame(scene, layer, at),
        "remove_frame" => frames::remove_frame(scene, layer, at),
        "insert_keyframe" => frames::insert_keyframe(scene, layer, at),
        "insert_blank_keyframe" => frames::insert_blank_keyframe(scene, layer, at),
        "clear_keyframe" => frames::clear_keyframe(scene, layer, at),
        other => return Err(format!("unknown operation {other}")),
    }
    .ok_or_else(|| format!("{operation} does not apply at frame {at} of this layer"))?;
    apply(server, commands)
}

fn layer_operation(server: &mut Server, args: &Args) -> Outcome {
    let scene = first_scene(server)?;
    let layer = target_layer(server, uuid(args, "layer")?)?;
    let layers = server
        .session
        .document()
        .scenes()
        .first()
        .map(|s| s.timeline().layers())
        .unwrap_or_default();
    let position = layers
        .iter()
        .position(|l| l.id() == layer)
        .ok_or("layer disappeared")?;
    let count = layers.len();
    let update = |change| Command::UpdateLayer {
        scene,
        layer,
        change,
    };
    let command = match required_text(args, "operation")? {
        "rename" => {
            let name = required_text(args, "name")?.trim();
            if name.is_empty() {
                return Err("a layer needs a name".to_owned());
            }
            update(LayerChange::Name(name.to_owned()))
        }
        "delete" if count <= 1 => return Err("a scene needs at least one layer".to_owned()),
        "delete" => Command::RemoveLayer { scene, layer },
        "hide" => update(LayerChange::Visible(false)),
        "show" => update(LayerChange::Visible(true)),
        "lock" => update(LayerChange::Locked(true)),
        "unlock" => update(LayerChange::Locked(false)),
        "move_up" => {
            let index = position
                .checked_sub(1)
                .ok_or("the layer is already at the top")?;
            Command::MoveLayer {
                scene,
                layer,
                index,
            }
        }
        "move_down" if position + 1 >= count => {
            return Err("the layer is already at the bottom".to_owned());
        }
        "move_down" => Command::MoveLayer {
            scene,
            layer,
            index: position + 1,
        },
        other => return Err(format!("unknown operation {other}")),
    };
    apply(server, vec![command])
}
