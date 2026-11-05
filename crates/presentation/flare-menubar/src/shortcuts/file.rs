//! Saving and loading a person's shortcuts: the set they started from plus what they changed, so
//! a newer set's keys still reach someone who never touched them.
//!
//! ```json
//! {"schema":1,"set":"flare","bindings":{"Open":"CmdOrCtrl+O","Undo":null}}
//! ```

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{ShortcutError, ShortcutMap, set_bindings};
use crate::MenuAction;

/// The largest shortcut file read.
pub const MAX_SHORTCUT_FILE_BYTES: usize = 64 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    schema: u32,
    set: String,
    bindings: BTreeMap<String, Option<String>>,
}

impl ShortcutMap {
    /// The file text: only the differences from the base set.
    pub fn to_json(&self) -> String {
        let base = set_bindings(self.set);
        let mut bindings = BTreeMap::new();
        for (action, accel) in self.entries() {
            let same = base
                .iter()
                .any(|(a, text)| *a == action && accel.to_string() == canonical(text, action));
            if !same {
                bindings.insert(action.id(), Some(accel.to_string()));
            }
        }
        for (action, _) in &base {
            if self.get(*action).is_none() {
                bindings.insert(action.id(), None);
            }
        }
        let file = File {
            schema: 1,
            set: self.set.to_owned(),
            bindings,
        };
        serde_json::to_string_pretty(&file).unwrap_or_default()
    }

    /// Read a file's text. Anything wrong (size, syntax, an unknown set or command, a bad
    /// accelerator, two commands on one key) is an error and builds nothing.
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
        let mut edits = Vec::new();
        for (id, value) in &file.bindings {
            let action =
                MenuAction::from_id(id).ok_or_else(|| ShortcutError::UnknownAction(id.clone()))?;
            edits.push((action, value.as_deref()));
        }
        // Free first, so that a swap of two keys in the file is not a conflict on the way.
        for (action, value) in &edits {
            if value.is_none() {
                map.unbind(*action);
            }
        }
        for (action, value) in &edits {
            if value.is_some() {
                map.bindings.remove(&action.id());
            }
        }
        for (action, value) in &edits {
            if let Some(text) = value {
                map.bind(*action, text)?;
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

fn canonical(text: &str, action: MenuAction) -> String {
    super::Accelerator::parse(text, action).map_or_else(|_| text.to_owned(), |a| a.to_string())
}

impl ShortcutMap {
    /// Every command with a shortcut, for the command line and the agent server to print.
    pub fn list_json(&self) -> serde_json::Value {
        let shortcuts: Vec<_> = self
            .entries()
            .into_iter()
            .map(|(action, accel)| {
                serde_json::json!({ "action": action.id(), "shortcut": accel.to_string() })
            })
            .collect();
        serde_json::json!({
            "set": self.set,
            "sets": super::SHORTCUT_SET_IDS,
            "shortcuts": shortcuts,
            "conflicts": self.conflicts().len(),
        })
    }

    /// The file text as a value, so a surface can print it with its other output.
    pub fn file_value(&self) -> serde_json::Value {
        serde_json::from_str(&self.to_json()).unwrap_or(serde_json::Value::Null)
    }
}
