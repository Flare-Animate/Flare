//! Parameters of the tool that changes a command's keyboard shortcuts.

use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct EditShortcutsParams {
    /// The command, by id ("Save", "ToolBrush"; `list_shortcuts` shows them).
    pub action: String,
    /// The set to start from: "flare" (the default), "opentoonz", "animate" or "tahoma".
    pub set: Option<String>,
    /// The text of a shortcut file to start from instead of `set`.
    pub file: Option<String>,
    /// Give the command these keys in place of all it has ("CmdOrCtrl+Shift+S", "B").
    pub keys: Option<String>,
    /// Give the command these keys beside those it has (a command has at most 4).
    pub add: Option<String>,
    /// Take these keys off the command; its other shortcuts stay.
    pub remove: Option<String>,
    /// Leave the command with no shortcut.
    pub clear: Option<bool>,
    /// Give the command its set's own keys again.
    pub reset: Option<bool>,
}
