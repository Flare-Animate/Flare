//! Saving and loading a person's shortcuts: the set they started from plus what they changed, so
//! a newer set's keys still reach someone who never touched them. A command with one shortcut is
//! written as its text, one with several as a list, first one first, and one freed as `null`.
//!
//! ```json
//! {"schema":1,"set":"flare","bindings":{"Open":"CmdOrCtrl+O","ToolBrush":["B","Shift+B"],"Undo":null}}
//! ```

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{Accelerator, MAX_KEYS_PER_COMMAND, ShortcutError, ShortcutMap};
use crate::MenuAction;

/// The largest shortcut file read.
pub const MAX_SHORTCUT_FILE_BYTES: usize = 64 * 1024;

/// A command's keys in the file: one, or a list of them.
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum Keys {
    One(String),
    Many(Vec<String>),
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    schema: u32,
    set: String,
    bindings: BTreeMap<String, Option<Keys>>,
}

impl ShortcutMap {
    /// The file text: only the differences from the base set.
    pub fn to_json(&self) -> String {
        let base = Self::from_set(self.set).unwrap_or_else(|_| Self::empty(self.set));
        let mut bindings = BTreeMap::new();
        for action in MenuAction::ALL {
            let keys = self.keys(action);
            if keys == base.keys(action) {
                continue;
            }
            let mut texts: Vec<String> = keys.iter().map(ToString::to_string).collect();
            let value = match texts.len() {
                0 => None,
                1 => texts.pop().map(Keys::One),
                _ => Some(Keys::Many(texts)),
            };
            bindings.insert(action.id(), value);
        }
        let file = File {
            schema: 1,
            set: self.set.to_owned(),
            bindings,
        };
        serde_json::to_string_pretty(&file).unwrap_or_default()
    }

    /// Read a file's text. Anything wrong (size, syntax, an unknown set or command, a bad
    /// accelerator, too many keys on a command, two commands on one key) is an error and builds
    /// nothing.
    pub fn from_json(text: &str) -> Result<Self, ShortcutError> {
        if text.len() > MAX_SHORTCUT_FILE_BYTES {
            return Err(ShortcutError::TooLarge(MAX_SHORTCUT_FILE_BYTES));
        }
        let file: File =
            serde_json::from_str(text).map_err(|e| ShortcutError::BadFile(e.to_string()))?;
        if file.schema != 1 {
            return Err(ShortcutError::BadFile(format!(
                "schema {} is not known",
                file.schema
            )));
        }
        let mut map = Self::from_set(&file.set)?;
        let mut edits: Vec<(MenuAction, Vec<String>)> = Vec::new();
        for (id, value) in file.bindings {
            let action =
                MenuAction::from_id(&id).ok_or_else(|| ShortcutError::UnknownAction(id.clone()))?;
            let keys = match value {
                None => Vec::new(),
                Some(Keys::One(text)) => vec![text],
                Some(Keys::Many(texts)) => texts,
            };
            if keys.len() > MAX_KEYS_PER_COMMAND {
                return Err(ShortcutError::TooManyKeys(MAX_KEYS_PER_COMMAND));
            }
            edits.push((action, keys));
        }
        // Free every command the file names first, so that a swap of two keys in the file is
        // not a conflict on the way.
        for (action, _) in &edits {
            map.unbind(*action);
        }
        for (action, keys) in &edits {
            for text in keys {
                map.add_key(*action, text)?;
            }
        }
        Ok(map)
    }

    /// Like [`from_json`](Self::from_json), but a file that cannot be used gives the default set
    /// and says why, so a corrupt file never leaves a person without shortcuts.
    pub fn from_json_or_default(text: &str) -> (Self, Option<ShortcutError>) {
        match Self::from_json(text) {
            Ok(map) => (map, None),
            Err(error) => (Self::default(), Some(error)),
        }
    }
}

/// An action's keys as the file and the listings write them.
fn key_texts(keys: &[Accelerator]) -> Vec<String> {
    keys.iter().map(ToString::to_string).collect()
}

impl ShortcutMap {
    /// Every command with a shortcut, for the command line and the agent server to print: its
    /// first shortcut as `shortcut`, and all of them, first one first, as `keys`.
    pub fn list_json(&self) -> serde_json::Value {
        let shortcuts: Vec<_> = MenuAction::ALL
            .into_iter()
            .filter(|action| !self.keys(*action).is_empty())
            .map(|action| self.row_json(action))
            .collect();
        serde_json::json!({
            "set": self.set,
            "sets": super::SHORTCUT_SET_IDS,
            "shortcuts": shortcuts,
            "conflicts": self.conflicts().len(),
        })
    }

    /// One command's row of a listing.
    pub(super) fn row_json(&self, action: MenuAction) -> serde_json::Value {
        serde_json::json!({
            "action": action.id(),
            "shortcut": self.get(action).map(ToString::to_string),
            "keys": key_texts(self.keys(action)),
        })
    }

    /// The file text as a value, so a surface can print it with its other output.
    pub fn file_value(&self) -> serde_json::Value {
        serde_json::from_str(&self.to_json()).unwrap_or(serde_json::Value::Null)
    }
}
