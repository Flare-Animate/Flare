//! `project.json` as the importer reads it: the project's own fields and the table of objects by
//! their IDs. Every field is looked up by name and checked, so a missing or odd field is a typed
//! refusal rather than a panic.

use serde_json::{Map, Value};

use super::error::WickError;
use super::limits::{MAX_CHILDREN, MAX_FPS, MAX_NAME_CHARS, MAX_OBJECTS};
use flare_document::MAX_STAGE_SIDE;

/// The project's own fields.
pub(super) struct Project {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub background: Option<String>,
    pub framerate: u32,
    pub focus: Option<String>,
    pub children: Vec<String>,
}

/// The object table, keyed by object ID.
pub(super) struct Objects(Map<String, Value>);

pub(super) struct Parsed {
    pub project: Project,
    pub objects: Objects,
}

pub(super) fn parse(bytes: &[u8]) -> Result<Parsed, WickError> {
    let value: Value =
        serde_json::from_slice(bytes).map_err(|e| WickError::BadJson(e.to_string()))?;
    let Value::Object(mut top) = value else {
        return Err(WickError::NoProjectObject);
    };
    let project_value = top.remove("project").ok_or(WickError::NoProjectObject)?;
    let objects = match top.remove("objects") {
        Some(Value::Object(map)) => map,
        _ => return Err(WickError::NoProjectObject),
    };
    if objects.len() > MAX_OBJECTS {
        return Err(WickError::TooLarge("the project's objects"));
    }
    if class_of(&project_value) != "Project" {
        return Err(WickError::BadObject("project".into(), "Project"));
    }
    let project = Project {
        name: name_of(text(&project_value, "name"), "Project"),
        width: whole(
            number(&project_value, "width"),
            1,
            u64::from(MAX_STAGE_SIDE),
        )
        .ok_or(WickError::BadField("width"))?,
        height: whole(
            number(&project_value, "height"),
            1,
            u64::from(MAX_STAGE_SIDE),
        )
        .ok_or(WickError::BadField("height"))?,
        background: text(&project_value, "backgroundColor").map(str::to_owned),
        framerate: whole(number(&project_value, "framerate"), 1, u64::from(MAX_FPS))
            .ok_or(WickError::BadField("framerate"))?,
        focus: text(&project_value, "focus").map(str::to_owned),
        children: child_ids(&project_value)?,
    };
    Ok(Parsed {
        project,
        objects: Objects(objects),
    })
}

impl Objects {
    pub fn get(&self, id: &str) -> Result<&Value, WickError> {
        self.0
            .get(id)
            .ok_or_else(|| WickError::BadObject(short(id), "object"))
    }

    /// The object's `classname`, or `""` when it has none.
    pub fn class<'a>(&'a self, id: &str) -> &'a str {
        self.0.get(id).map_or("", class_of)
    }

    /// The object, if it exists and is of the class `want`.
    pub fn of_class(&self, id: &str, want: &'static str) -> Result<&Value, WickError> {
        let value = self.get(id)?;
        if class_of(value) == want {
            Ok(value)
        } else {
            Err(WickError::BadObject(short(id), want))
        }
    }

    /// The ID of the first child of `obj` of class `want`.
    pub fn child_of_class(&self, obj: &Value, want: &'static str) -> Result<String, WickError> {
        child_ids(obj)?
            .into_iter()
            .find(|id| self.class(id) == want)
            .ok_or(WickError::BadObject(String::new(), want))
    }
}

/// The IDs of an object's children, in order, each a string and the list no longer than the bound.
pub(super) fn child_ids(obj: &Value) -> Result<Vec<String>, WickError> {
    let Some(list) = array(obj, "children") else {
        return Ok(Vec::new());
    };
    if list.len() > MAX_CHILDREN {
        return Err(WickError::TooLarge("an object's children"));
    }
    list.iter()
        .map(|v| {
            v.as_str()
                .map(str::to_owned)
                .ok_or(WickError::BadField("children"))
        })
        .collect()
}

pub(super) fn class_of(value: &Value) -> &str {
    text(value, "classname").unwrap_or("")
}

pub(super) fn text<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

pub(super) fn number(value: &Value, key: &str) -> Option<f64> {
    value.get(key).and_then(Value::as_f64)
}

pub(super) fn array<'a>(value: &'a Value, key: &str) -> Option<&'a Vec<Value>> {
    value.get(key).and_then(Value::as_array)
}

/// A whole number from `lo` to `hi`, or `None`.
pub(super) fn whole(value: Option<f64>, lo: u64, hi: u64) -> Option<u32> {
    let n = value?;
    if n.fract() != 0.0 || n < lo as f64 || n > hi as f64 {
        return None;
    }
    u32::try_from(n as u64).ok()
}

fn name_of(value: Option<&str>, fallback: &str) -> String {
    let name: String = value
        .unwrap_or(fallback)
        .chars()
        .filter(|c| !c.is_control())
        .take(MAX_NAME_CHARS)
        .collect();
    if name.trim().is_empty() {
        fallback.to_owned()
    } else {
        name
    }
}

/// An ID for a message: at most 64 characters.
fn short(id: &str) -> String {
    id.chars().take(64).collect()
}
