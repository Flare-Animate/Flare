//! One animated value. Moho stores the frames (`when`) and the values (`val`) as parallel arrays;
//! the per-segment `interp` entries and the `mute` flag ride along.

use serde_json::{Map, Value};

use crate::limits::MAX_KEY_VECTOR;
use crate::model::{Channel, ChannelKind, Key, KeyValue};

/// Reads a channel object. Anything that is not a channel gives an empty one.
pub(crate) fn channel(node: &Value) -> Channel {
    let Some(o) = node.as_object() else {
        return Channel::default();
    };
    channel_from_object(o)
}

fn channel_from_object(o: &Map<String, Value>) -> Channel {
    let (Some(when), Some(values)) = (
        o.get("when").and_then(Value::as_array),
        o.get("val").and_then(Value::as_array),
    ) else {
        return Channel::default();
    };
    Channel {
        kind: ChannelKind::from_moho(o.get("type").and_then(Value::as_str).unwrap_or_default()),
        keys: when
            .iter()
            .zip(values)
            .map(|(frame, value)| Key {
                frame: frame.as_i64().unwrap_or(0),
                value: key_value(value),
            })
            .collect(),
        interp: o
            .get("interp")
            .and_then(Value::as_array)
            .map(|items| items.iter().map(interp_name).collect::<Vec<String>>())
            .unwrap_or_default(),
        muted: o.get("mute").and_then(Value::as_bool).unwrap_or(false),
    }
}

fn interp_name(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Object(o) => o
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        _ => String::new(),
    }
}

fn key_value(value: &Value) -> KeyValue {
    match value {
        Value::Bool(b) => KeyValue::Bool(*b),
        Value::Number(n) => n.as_f64().map_or(KeyValue::Unknown, KeyValue::Number),
        Value::String(s) => KeyValue::Text(s.clone()),
        Value::Array(items) if items.len() <= MAX_KEY_VECTOR => items
            .iter()
            .map(Value::as_f64)
            .collect::<Option<Vec<f64>>>()
            .map_or(KeyValue::Unknown, KeyValue::Vector),
        Value::Object(o) => {
            let parts: Vec<f64> = ["x", "y", "z", "w"]
                .iter()
                .map_while(|k| o.get(*k).and_then(Value::as_f64))
                .collect();
            if parts.is_empty() {
                KeyValue::Unknown
            } else {
                KeyValue::Vector(parts)
            }
        }
        _ => KeyValue::Unknown,
    }
}
