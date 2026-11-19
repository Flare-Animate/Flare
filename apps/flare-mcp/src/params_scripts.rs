//! The arguments of the tools that read and patch a Flash movie's ActionScript 3.

use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListScripts {
    /// The movie's file name inside the export directory (a `.swf`).
    pub source: String,
    /// Only classes whose full name contains this, ignoring case.
    pub find: Option<String>,
    /// The most classes returned (200 by default); `matched` counts them all.
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ScriptClass {
    /// The movie's file name inside the export directory.
    pub source: String,
    /// The class's full name (`alternativa.tanks.Main`), or its short name when only one class
    /// has it.
    pub class: String,
    /// Only this member (and the constructor when it is the class's name).
    pub member: Option<String>,
    /// List each method body as instructions; otherwise only its size is given.
    pub code: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ScriptStrings {
    /// The movie's file name inside the export directory.
    pub source: String,
    /// Only strings containing this, ignoring case.
    pub find: Option<String>,
    /// Only strings the code uses as text, not those that are only names.
    pub text_only: Option<bool>,
    /// The most strings returned (500 by default); `matched` counts them all.
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct StringReplacement {
    /// The string to replace, matched whole and exactly.
    pub from: String,
    /// What it becomes.
    pub to: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PatchStrings {
    /// The movie's file name inside the export directory. It is not changed.
    pub source: String,
    /// File name for the patched movie, written beside it; `.swf` is added when missing.
    pub name: String,
    pub replacements: Vec<StringReplacement>,
    /// Also replace strings the code links by (class, property, method names). That breaks
    /// every link to the old name not renamed with it. Off by default.
    pub rename_names: Option<bool>,
}
