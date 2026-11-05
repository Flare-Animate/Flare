//! Reads the project document's JSON into the model. Every field is read with a default, so a
//! document that lacks a field from another format generation still parses.
//!
//! Bones live in two places: a Moho 11 and 12 document has one root `skeleton`, while Moho 13
//! keeps a skeleton inside each bone layer (`layers[i].skeleton`). Both are read, and each bone
//! records the path of its keyframe tracks.

use serde_json::{Map, Value};

use crate::channel::channel;
use crate::container::UTF8_BOM;
use crate::error::MohoError;
use crate::limits::{
    MAX_BONES, MAX_DOCUMENT_BYTES, MAX_LAYER_DEPTH, MAX_LAYERS, MAX_SWITCH_ALTERNATIVES,
};
use crate::model::{Binding, Bone, Layer, LayerKind, Project, SwitchCells};
use crate::tree;

/// Counters shared by the whole walk, so each limit holds for the project as a whole.
#[derive(Default)]
struct Walk {
    layers: usize,
    mesh_budget: (usize, usize),
    bones: Vec<Bone>,
}

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

    let mut walk = Walk::default();
    read_skeleton(root.get("skeleton"), "skeleton", &mut walk)?;
    let layers = read_layers(root.get("layers"), "layers", 0, &mut walk)?;
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
        bones: walk.bones,
        tracks: harvest.tracks,
        images: harvest.images.into_iter().collect(),
        audio: harvest.audio.into_iter().collect(),
    })
}

/// `path` is the document path of the layer list the entries sit in (`layers` at the top, then
/// `layers[0].layers`, and so on); each entry's path is that plus its index.
fn read_layers(
    value: Option<&Value>,
    path: &str,
    depth: usize,
    walk: &mut Walk,
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
    for (index, item) in items.iter().enumerate() {
        let Some(o) = item.as_object() else {
            continue;
        };
        walk.layers += 1;
        if walk.layers > MAX_LAYERS {
            return Err(MohoError::Limit {
                what: "layers",
                limit: MAX_LAYERS,
            });
        }
        let here = format!("{path}[{index}]");
        let kind = LayerKind::from_moho(&text(o, "type"));
        let kind_is_vector = kind == LayerKind::Vector;
        let switch = (kind == LayerKind::Switch).then(|| switch_cells(o));
        if kind == LayerKind::Bone {
            read_skeleton(o.get("skeleton"), &format!("{here}.skeleton"), walk)?;
        }
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
            mesh: if kind_is_vector {
                crate::mesh::read(o.get("mesh"), &mut walk.mesh_budget)?
            } else {
                None
            },
            children: read_layers(o.get("layers"), &format!("{here}.layers"), depth + 1, walk)?,
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

/// Appends the bones of one skeleton. A parent index is local to its skeleton; it is stored as
/// an index into the whole project's bones, and one that names no bone of the skeleton makes a
/// root instead of failing the file.
fn read_skeleton(value: Option<&Value>, path: &str, walk: &mut Walk) -> Result<(), MohoError> {
    let Some(list) = value
        .and_then(Value::as_object)
        .and_then(|s| s.get("bones"))
        .and_then(Value::as_array)
    else {
        return Ok(());
    };
    if walk.bones.len() + list.len() > MAX_BONES {
        return Err(MohoError::Limit {
            what: "bones",
            limit: MAX_BONES,
        });
    }
    let base = walk.bones.len();
    let empty = Map::new();
    let mut local: Vec<Option<usize>> = list
        .iter()
        .map(|item| {
            let raw = int(item.as_object().unwrap_or(&empty), "parent", -1);
            usize::try_from(raw).ok().filter(|p| *p < list.len())
        })
        .collect();
    break_cycles(&mut local);
    for (index, item) in list.iter().enumerate() {
        let o = item.as_object().unwrap_or(&empty);
        walk.bones.push(Bone {
            name: text(o, "name"),
            parent: local[index].map(|p| base + p),
            length: float(o, "length", 0.0),
            strength: float(o, "strength", 0.0),
            dynamic: flag(o, "bone_dynamics", false),
            path: format!("{path}.bones[{index}]"),
        });
    }
    Ok(())
}

/// Clears the parent of every bone that is on a loop of parents, so each chain ends at a root.
/// Quadratic in the bones of one skeleton, which [`MAX_BONES`] bounds.
fn break_cycles(parents: &mut [Option<usize>]) {
    let n = parents.len();
    for start in 0..n {
        let mut cur = start;
        for _ in 0..n {
            match parents[cur] {
                Some(p) => cur = p,
                None => break,
            }
            if cur == start {
                parents[start] = None;
                break;
            }
        }
    }
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
