//! One walk over the whole document, collecting animated values (tracks) and relative file
//! references. The walk follows the tree rather than a list of key names: channels sit under
//! transforms, meshes, bones and actions, and a format generation that adds one is picked up
//! without a code change.

use std::collections::BTreeSet;
use std::path::Path;

use serde_json::{Map, Value};

use crate::channel::channel;
use crate::error::MohoError;
use crate::limits::{MAX_JSON_DEPTH, MAX_KEYFRAMES, MAX_REFERENCES, MAX_TRACKS};
use crate::model::{Channel, Track};

/// Keys that hold a bitmap's path. Moho stores absolute paths beside relative ones; only the
/// relative ones resolve against the project's folder.
const IMAGE_KEYS: [&str; 6] = [
    "image_path",
    "fill_texture_path",
    "line_texture_path",
    "fill_texture_fileref",
    "line_texture_fileref",
    "layer_ref_path",
];
/// Keys that hold a sound's path. Not documented by the original importer; unverified against
/// real projects.
const AUDIO_KEYS: [&str; 2] = ["audio_path", "sound_path"];

#[derive(Default)]
pub(crate) struct Harvest {
    pub(crate) tracks: Vec<Track>,
    pub(crate) images: BTreeSet<String>,
    pub(crate) audio: BTreeSet<String>,
    keyframes: usize,
    path: String,
}

pub(crate) fn harvest(root: &Value) -> Result<Harvest, MohoError> {
    let mut harvest = Harvest::default();
    harvest.visit(root, 0)?;
    Ok(harvest)
}

impl Harvest {
    fn visit(&mut self, node: &Value, depth: usize) -> Result<(), MohoError> {
        if depth > MAX_JSON_DEPTH {
            return Err(MohoError::Limit {
                what: "JSON nesting",
                limit: MAX_JSON_DEPTH,
            });
        }
        match node {
            Value::Object(o) if o.contains_key("when") && o.contains_key("val") => {
                let found = channel(node);
                if !found.is_empty() {
                    self.add_track(found)?;
                }
                Ok(())
            }
            Value::Object(o) => {
                self.references(o)?;
                for (key, child) in o {
                    let mark = self.path.len();
                    if mark > 0 {
                        self.path.push('.');
                    }
                    self.path.push_str(key);
                    self.visit(child, depth + 1)?;
                    self.path.truncate(mark);
                }
                Ok(())
            }
            Value::Array(items) => {
                for (index, child) in items.iter().enumerate() {
                    let mark = self.path.len();
                    self.path.push('[');
                    self.path.push_str(&index.to_string());
                    self.path.push(']');
                    self.visit(child, depth + 1)?;
                    self.path.truncate(mark);
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn add_track(&mut self, channel: Channel) -> Result<(), MohoError> {
        if self.tracks.len() >= MAX_TRACKS {
            return Err(MohoError::Limit {
                what: "keyframe tracks",
                limit: MAX_TRACKS,
            });
        }
        self.keyframes += channel.len();
        if self.keyframes > MAX_KEYFRAMES {
            return Err(MohoError::Limit {
                what: "keyframes",
                limit: MAX_KEYFRAMES,
            });
        }
        self.tracks.push(Track {
            path: self.path.clone(),
            channel,
        });
        Ok(())
    }

    fn references(&mut self, o: &Map<String, Value>) -> Result<(), MohoError> {
        for key in IMAGE_KEYS {
            if let Some(path) = relative_path(o, key) {
                insert_reference(&mut self.images, path)?;
            }
        }
        for key in AUDIO_KEYS {
            if let Some(path) = relative_path(o, key) {
                insert_reference(&mut self.audio, path)?;
            }
        }
        Ok(())
    }
}

fn relative_path<'a>(o: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    let path = o.get(key)?.as_str()?;
    // A leading slash is rooted on every platform; Windows' `is_absolute` needs a drive too.
    let rooted = path.starts_with(['/', '\\']);
    let relative =
        !path.is_empty() && !path.contains("://") && !rooted && !Path::new(path).is_absolute();
    relative.then_some(path)
}

fn insert_reference(set: &mut BTreeSet<String>, path: &str) -> Result<(), MohoError> {
    if set.contains(path) {
        return Ok(());
    }
    if set.len() >= MAX_REFERENCES {
        return Err(MohoError::Limit {
            what: "file references",
            limit: MAX_REFERENCES,
        });
    }
    set.insert(path.to_owned());
    Ok(())
}
