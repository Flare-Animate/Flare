//! Reads the project document's JSON into the model. Every field is read with a default, so a
//! document that lacks a field from another format generation still parses.

use serde_json::{Map, Value};

use crate::channel::channel;
use crate::container::UTF8_BOM;
use crate::error::MohoError;
use crate::limits::{
    MAX_BONES, MAX_DOCUMENT_BYTES, MAX_LAYER_DEPTH, MAX_LAYERS, MAX_SWITCH_ALTERNATIVES,
};
use crate::model::{Binding, Bone, Layer, LayerKind, Project, SwitchCells};
use crate::tree;

pub(crate) fn parse_document(bytes: &[u8], entry: String) -> Result<Project, MohoError> {
    let size = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if size > MAX_DOCUMENT_BYTES {
        return Err(MohoError::TooLarge {
            what: "the project document",
            size,
            limit: MAX_DOCUMENT_BYTES,
        });
    }
    let bytes = bytes.strip_prefix(&UTF8_BOM).unwrap_or(bytes);
    let value: Value = serde_json::from_slice(bytes)?;
    let root = value.as_object().ok_or(MohoError::NotObject)?;
    let empty = Map::new();
    let project_data = root
        .get("project_data")
        .and_then(Value::as_object)
        .unwrap_or(&empty);

    let mut layer_count = 0;
    let layers = read_layers(root.get("layers"), 0, &mut layer_count)?;
    let bones = read_bones(root)?;
    let harvest = tree::harvest(&value)?;
    Ok(Project {
        entry,
        version: int(root, "version", 0),
        major_version: int(root, "major_version", 0),
        width: u32::try_from(int(project_data, "width", 0)).unwrap_or(0),
        height: u32::try_from(int(project_data, "height", 0)).unwrap_or(0),
        fps: float(project_data, "fps", 0.0),
        start_frame: int(project_data, "start_frame", 0),
        end_frame: int(project_data, "end_frame", 0),
        layers,
        bones,
        tracks: harvest.tracks,
        images: harvest.images.into_iter().collect(),
        audio: harvest.audio.into_iter().collect(),
    })
}

fn read_layers(
    value: Option<&Value>,
    depth: usize,
    count: &mut usize,
) -> Result<Vec<Layer>, MohoError> {
    let Some(items) = value.and_then(Value::as_array) else {
        return Ok(Vec::new());
    };
    if items.is_empty() {
        return Ok(Vec::new());
    }
    if depth >= MAX_LAYER_DEPTH {
        return Err(MohoError::Limit {
            what: "layer nesting",
            limit: MAX_LAYER_DEPTH,
        });
    }
    let mut layers = Vec::with_capacity(items.len().min(MAX_LAYERS));
    for item in items {
        let Some(o) = item.as_object() else {
            continue;
        };
        *count += 1;
        if *count > MAX_LAYERS {
            return Err(MohoError::Limit {
                what: "layers",
                limit: MAX_LAYERS,
            });
        }
        let kind = LayerKind::from_moho(&text(o, "type"));
        let switch = (kind == LayerKind::Switch).then(|| switch_cells(o));
        layers.push(Layer {
            name: text(o, "name"),
            kind,
            uuid: text(o, "uuid"),
            visible: flag(o, "visible", true),
            depth,
            binding: Binding::from_raw(int(o, "parent_bone", -2)),
            flexible_bone_subset: text(o, "flexi_bone_subset"),
            image_path: opt_text(o, "image_path"),
            texture_path: opt_text(o, "fill_texture_path")
                .or_else(|| opt_text(o, "line_texture_path")),
            switch,
            children: read_layers(o.get("layers"), depth + 1, count)?,
        });
    }
    Ok(layers)
}

fn switch_cells(o: &Map<String, Value>) -> SwitchCells {
    let alternatives: Vec<String> = o
        .get("layers")
        .and_then(Value::as_array)
        .map(|children| {
            children
                .iter()
                .filter_map(Value::as_object)
                .take(MAX_SWITCH_ALTERNATIVES)
                .map(|c| text(c, "name"))
                .collect()
        })
        .unwrap_or_default();
    let keys = o.get("switch_keys").map(channel).unwrap_or_default();
    let at_rest = keys
        .keys
        .first()
        .and_then(|k| k.value.as_text())
        .map(str::to_owned);
    let at_end = keys
        .keys
        .last()
        .and_then(|k| k.value.as_text())
        .map(str::to_owned);
    SwitchCells {
        alternatives,
        keys,
        at_rest,
        at_end,
    }
}

fn read_bones(root: &Map<String, Value>) -> Result<Vec<Bone>, MohoError> {
    let empty = Map::new();
    let skeleton = root
        .get("skeleton")
        .and_then(Value::as_object)
        .unwrap_or(&empty);
    let Some(list) = skeleton.get("bones").and_then(Value::as_array) else {
        return Ok(Vec::new());
    };
    if list.len() > MAX_BONES {
        return Err(MohoError::Limit {
            what: "bones",
            limit: MAX_BONES,
        });
    }
    let mut bones = Vec::with_capacity(list.len());
    for (index, item) in list.iter().enumerate() {
        let empty = Map::new();
        let o = item.as_object().unwrap_or(&empty);
        // A parent must be an earlier bone, so the hierarchy is a forest with no cycles.
        let raw = int(o, "parent", -1);
        let parent = match usize::try_from(raw) {
            Ok(p) if p < index => Some(p),
            Ok(_) => return Err(MohoError::BadBoneParent { index, parent: raw }),
            Err(_) => None,
        };
        bones.push(Bone {
            name: text(o, "name"),
            parent,
            length: float(o, "length", 0.0),
            strength: float(o, "strength", 0.0),
            dynamic: flag(o, "bone_dynamics", false),
        });
    }
    Ok(bones)
}

fn int(o: &Map<String, Value>, key: &str, default: i64) -> i64 {
    o.get(key).and_then(Value::as_i64).unwrap_or(default)
}

fn float(o: &Map<String, Value>, key: &str, default: f64) -> f64 {
    o.get(key).and_then(Value::as_f64).unwrap_or(default)
}

fn flag(o: &Map<String, Value>, key: &str, default: bool) -> bool {
    o.get(key).and_then(Value::as_bool).unwrap_or(default)
}

fn text(o: &Map<String, Value>, key: &str) -> String {
    o.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

fn opt_text(o: &Map<String, Value>, key: &str) -> Option<String> {
    o.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}
