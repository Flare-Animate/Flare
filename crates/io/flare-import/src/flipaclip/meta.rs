//! The `meta` entry: a JSON object with the canvas, the frame rate, the frame count and the
//! layers. The layers are a JSON array held inside a JSON string (`layersState`), as FlipaClip
//! writes them, so that string is parsed a second time.

use serde_json::{Map, Value};

use super::archive::shorten;
use super::error::FlipaError;
use super::limits::{MAX_FPS, MAX_FRAMES, MAX_LAYERS, MAX_META_BYTES};

pub(super) struct Meta {
    pub name: String,
    pub version: Option<u64>,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub frame_count: u32,
    /// In FlipaClip's own order; assumed to list the bottom layer first (docs/FLIPACLIP.md).
    pub layers: Vec<String>,
}

pub(super) fn parse(bytes: &[u8]) -> Result<Meta, FlipaError> {
    let root: Value =
        serde_json::from_slice(bytes).map_err(|_| FlipaError::BadMeta("it is not JSON"))?;
    let object = root
        .as_object()
        .ok_or(FlipaError::BadMeta("the top level is not an object"))?;
    let format = object
        .get("format")
        .and_then(Value::as_u64)
        .ok_or(FlipaError::BadField("format"))?;
    if format != 1 {
        return Err(FlipaError::UnknownFormat(format));
    }
    let width = bounded(object, "canvasWidth", 1, flare_document::MAX_BITMAP_SIDE)?;
    let height = bounded(object, "canvasHeight", 1, flare_document::MAX_BITMAP_SIDE)?;
    if u64::from(width) * u64::from(height) > flare_document::MAX_BITMAP_PIXELS {
        return Err(FlipaError::BadField("canvasWidth"));
    }
    let fps = bounded(object, "fps", 1, MAX_FPS)?;
    let frame_count = bounded(object, "frameCount", 1, MAX_FRAMES)?;
    let name = object
        .get("name")
        .and_then(Value::as_str)
        .map(clean)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "FlipaClip".to_owned());
    let text = object
        .get("layersState")
        .and_then(Value::as_str)
        .ok_or(FlipaError::BadField("layersState"))?;
    if text.len() as u64 > MAX_META_BYTES {
        return Err(FlipaError::TooLarge("layersState"));
    }
    let items: Value =
        serde_json::from_str(text).map_err(|_| FlipaError::BadMeta("layersState is not JSON"))?;
    let items = items
        .as_array()
        .ok_or(FlipaError::BadField("layersState"))?;
    if items.len() > MAX_LAYERS {
        return Err(FlipaError::TooLarge("the layers"));
    }
    let layers = items
        .iter()
        .enumerate()
        .map(|(index, item)| layer_name(item, index))
        .collect();
    Ok(Meta {
        name,
        version: object.get("version").and_then(Value::as_u64),
        width,
        height,
        fps,
        frame_count,
        layers,
    })
}

fn bounded(
    object: &Map<String, Value>,
    key: &'static str,
    min: u32,
    max: u32,
) -> Result<u32, FlipaError> {
    object
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|v| u32::try_from(v).ok())
        .filter(|v| (min..=max).contains(v))
        .ok_or(FlipaError::BadField(key))
}

fn layer_name(item: &Value, index: usize) -> String {
    item.get("name")
        .and_then(Value::as_str)
        .map(clean)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("Layer {}", index + 1))
}

/// A name from the file: no control characters, at most 64 characters, trimmed.
fn clean(text: &str) -> String {
    shorten(text).trim().to_owned()
}
