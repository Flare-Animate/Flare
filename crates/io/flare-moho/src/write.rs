//! Writes the model back as a Moho project document: pretty JSON with Moho's field names,
//! shaped so that reading it again gives the same model. That is the round trip this crate
//! promises, and it is what the tests check (`parse(write(parse(x))) == parse(x)`).
//!
//! The model is what the reader kept, so it is also what the writer can give back: Bezier
//! handles, the audio references' original fields, a drawing's fills past the first, and
//! layer entries the source held as something other than an object are not kept and so are
//! not written. Keyframe tracks are placed by the JSON path they were read from, so a
//! generation that adds an animated field in a place this crate does not model is still
//! written back where it was found.

use serde_json::{Map, Value, json};

use crate::error::MohoError;
use crate::limits::MAX_JSON_DEPTH;
use crate::model::{Binding, Bone, Channel, ChannelKind, KeyValue, Layer, LayerKind, Project};

/// The Moho `type` name of each layer kind, the reverse of the reader's mapping.
fn moho_type(kind: &LayerKind) -> &str {
    match kind {
        LayerKind::Vector => "MeshLayer",
        LayerKind::Image => "ImageLayer",
        LayerKind::Bone => "BoneLayer",
        LayerKind::Switch => "SwitchLayer",
        LayerKind::Group => "GroupLayer",
        LayerKind::Other(raw) => raw.as_str(),
    }
}

fn binding_raw(binding: &Binding) -> i64 {
    match binding {
        Binding::None => -2,
        Binding::Region => -1,
        Binding::Bone(index) => i64::try_from(*index).unwrap_or(-2),
    }
}

/// Writes the project as a `.mohoproj` document. Writing from the model cannot fail except
/// for a track path deeper than the reader's own nesting bound.
pub fn write_document(project: &Project) -> Result<Vec<u8>, MohoError> {
    let mut root = Map::new();
    root.insert("version".into(), json!(project.version));
    root.insert("major_version".into(), json!(project.major_version));
    root.insert(
        "project_data".into(),
        json!({
            "width": project.width,
            "height": project.height,
            "fps": project.fps,
            "start_frame": project.start_frame,
            "end_frame": project.end_frame,
        }),
    );
    let mut document = Value::Object(root);

    // The root skeleton holds the bones whose path says it does. Bones are collected with
    // their position in the project's list, because a Moho parent index is local to its
    // skeleton and the model stores the project-wide one.
    let root_bones: Vec<(usize, &Bone)> = project
        .bones
        .iter()
        .enumerate()
        .filter(|(_, bone)| bone.path.starts_with("skeleton.bones["))
        .collect();
    if let Some((base, _)) = root_bones.first() {
        document["skeleton"] = skeleton_json(&root_bones, *base);
    }
    document["layers"] = layers_json(project, &project.layers, "layers")?;

    // Tracks go back where they were read from, over a rest value the layer already wrote.
    for track in &project.tracks {
        place_track(&mut document, &track.path, channel_json(&track.channel))?;
    }
    let mut bytes = serde_json::to_vec_pretty(&document)?;
    bytes.push(b'\n');
    Ok(bytes)
}

/// One skeleton: the bones' local parents, back from the model's project-wide ones. `base` is
/// the first bone's position in the project's list, which the reader added to every local
/// parent of this skeleton.
fn skeleton_json(bones: &[(usize, &Bone)], base: usize) -> Value {
    let list: Vec<Value> = bones
        .iter()
        .map(|(_, bone)| {
            let local = bone
                .parent
                .and_then(|parent| parent.checked_sub(base))
                .filter(|local| *local < bones.len());
            json!({
                "name": bone.name,
                "parent": local.map_or(-1, |local| {
                    i64::try_from(local).unwrap_or(-1)
                }),
                "length": bone.length,
                "strength": bone.strength,
                "bone_dynamics": bone.dynamic,
            })
        })
        .collect();
    json!({ "bones": list })
}

fn layers_json(project: &Project, layers: &[Layer], path: &str) -> Result<Value, MohoError> {
    let mut out = Vec::with_capacity(layers.len());
    for (index, layer) in layers.iter().enumerate() {
        let here = format!("{path}[{index}]");
        out.push(layer_json(project, layer, &here)?);
    }
    Ok(Value::Array(out))
}

fn layer_json(project: &Project, layer: &Layer, here: &str) -> Result<Value, MohoError> {
    let mut o = Map::new();
    o.insert("name".into(), json!(layer.name));
    o.insert("type".into(), json!(moho_type(&layer.kind)));
    if !layer.uuid.is_empty() {
        o.insert("uuid".into(), json!(layer.uuid));
    }
    o.insert("visible".into(), json!(layer.visible));
    o.insert("parent_bone".into(), json!(binding_raw(&layer.binding)));
    if !layer.flexible_bone_subset.is_empty() {
        o.insert(
            "flexi_bone_subset".into(),
            json!(layer.flexible_bone_subset),
        );
    }
    if let Some(image) = &layer.image_path {
        o.insert("image_path".into(), json!(image));
    }
    if let Some(texture) = &layer.texture_path {
        // The model keeps one texture path, not which slot it came from; the fill slot is
        // written and reads back the same.
        o.insert("fill_texture_path".into(), json!(texture));
    }
    if let Some(audio) = &layer.audio_path {
        // The model keeps one audio path, not which key it came from; `audio_path` is
        // written and reads back the same.
        o.insert("audio_path".into(), json!(audio));
    }
    if let Some(mesh) = &layer.mesh {
        o.insert("mesh".into(), mesh_json(mesh));
    }
    if layer.kind == LayerKind::Switch
        && let Some(switch) = &layer.switch
    {
        o.insert("switch_keys".into(), channel_json(&switch.keys));
    }
    if layer.kind == LayerKind::Bone {
        let prefix = format!("{here}.skeleton.bones[");
        let bones: Vec<(usize, &Bone)> = project
            .bones
            .iter()
            .enumerate()
            .filter(|(_, bone)| bone.path.starts_with(&prefix))
            .collect();
        if let Some((base, _)) = bones.first() {
            o.insert("skeleton".into(), skeleton_json(&bones, *base));
        }
    }
    if !layer.children.is_empty() {
        let children_path = format!("{here}.layers");
        o.insert(
            "layers".into(),
            layers_json(project, &layer.children, &children_path)?,
        );
    }
    Ok(Value::Object(o))
}

fn mesh_json(mesh: &crate::model::Mesh) -> Value {
    json!({
        "points": mesh.points.iter().map(|point| {
            json!({ "position": { "x": point[0], "y": point[1] } })
        }).collect::<Vec<Value>>(),
        "curves": mesh.curves.iter().map(|curve| {
            json!({
                "closed": curve.closed,
                "points": curve.points.iter().map(|i| json!({ "point": i }))
                    .collect::<Vec<Value>>(),
            })
        }).collect::<Vec<Value>>(),
        // The model keeps the first filled shape's colour, so one shape is written.
        "shapes": mesh.fill.map(|fill| vec![json!({
            "has_fill": true,
            "style": { "fill_color": { "r": fill[0], "g": fill[1], "b": fill[2], "a": fill[3] } },
        })]).unwrap_or_default(),
    })
}

fn channel_json(channel: &Channel) -> Value {
    let mut o = Map::new();
    o.insert(
        "type".into(),
        json!(match &channel.kind {
            ChannelKind::Val => "Val",
            ChannelKind::Vec2 => "Vec2",
            ChannelKind::Vec3 => "Vec3",
            ChannelKind::Color => "Color",
            ChannelKind::Bool => "Bool",
            ChannelKind::String => "String",
            ChannelKind::Other(raw) => raw.as_str(),
        }),
    );
    // Values are written as arrays: the reader takes a vector from an array or an
    // `{x, y, ...}` object alike, and an array survives any component count.
    let mut when = Vec::with_capacity(channel.keys.len());
    let mut val = Vec::with_capacity(channel.keys.len());
    for key in &channel.keys {
        let Some(value) = value_json(&key.value) else {
            continue;
        };
        when.push(json!(key.frame));
        val.push(value);
    }
    o.insert("when".into(), Value::Array(when));
    o.insert("val".into(), Value::Array(val));
    if !channel.interp.is_empty() {
        o.insert(
            "interp".into(),
            json!(channel.interp.iter().collect::<Vec<&String>>()),
        );
    }
    if channel.muted {
        o.insert("mute".into(), json!(true));
    }
    Value::Object(o)
}

/// A key's value as JSON, or `None` for the values the model keeps out (a value of a shape
/// the reader did not know); such a key is dropped, frame and all.
fn value_json(value: &KeyValue) -> Option<Value> {
    match value {
        KeyValue::Number(n) => Some(json!(n)),
        KeyValue::Vector(parts) => Some(json!(parts)),
        KeyValue::Bool(b) => Some(json!(b)),
        KeyValue::Text(text) => Some(json!(text)),
        KeyValue::Unknown => None,
    }
}

/// Puts a channel at its document path, creating the objects and arrays along it. An array
/// that must grow is filled with nulls up to the index, the way a sparse Moho document reads.
fn place_track(document: &mut Value, path: &str, channel: Value) -> Result<(), MohoError> {
    let mut segments: Vec<(&str, Option<usize>)> = Vec::new();
    for part in path.split('.') {
        let (name, index) = match part.split_once('[') {
            Some((name, rest)) => {
                let end = rest
                    .split_once(']')
                    .map(|(index, _)| index)
                    .ok_or(MohoError::NotObject)?;
                (
                    name,
                    Some(end.parse::<usize>().map_err(|_| MohoError::NotObject)?),
                )
            }
            None => (part, None),
        };
        segments.push((name, index));
        if segments.len() > MAX_JSON_DEPTH {
            return Err(MohoError::Limit {
                what: "track path depth",
                limit: MAX_JSON_DEPTH,
            });
        }
    }
    let Some((last_name, _)) = segments.pop() else {
        return Err(MohoError::NotObject);
    };
    let mut node = document;
    for (name, index) in segments {
        node = descend(node, name, index)?;
    }
    let object = node.as_object_mut().ok_or(MohoError::NotObject)?;
    object.insert(last_name.to_owned(), channel);
    Ok(())
}

fn descend<'a>(
    node: &'a mut Value,
    name: &str,
    index: Option<usize>,
) -> Result<&'a mut Value, MohoError> {
    let object = node.as_object_mut().ok_or(MohoError::NotObject)?;
    let child = object
        .entry(name.to_owned())
        .or_insert_with(|| match index {
            Some(_) => Value::Array(Vec::new()),
            None => Value::Object(Map::new()),
        });
    match index {
        Some(index) => {
            let array = child.as_array_mut().ok_or(MohoError::NotObject)?;
            while array.len() <= index {
                array.push(Value::Null);
            }
            if array[index].is_null() {
                array[index] = Value::Object(Map::new());
            }
            Ok(&mut array[index])
        }
        None => Ok(child),
    }
}
