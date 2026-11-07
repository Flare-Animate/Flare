//! The reasons a Wick project is refused. Each names the part that failed, so the message says
//! what to fix; a message never carries the project's own text.

use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum WickError {
    #[error("not a zip archive; a Wick project is a zip with project.json and assets/")]
    NotZip,
    #[error("gzip data; Wick does not write gzip, a Wick project is a zip archive")]
    Gzip,
    #[error("plain JSON, the layout of Wick projects before 1.0.9, which is not read")]
    LegacyJson,
    #[error("the entry {0:?} has a name that would leave the project")]
    UnsafeName(String),
    #[error("the entry {0:?} is listed twice")]
    DuplicateEntry(String),
    #[error("the entry {0:?} is corrupt")]
    CorruptEntry(String),
    #[error("there is no project.json, so this is not a Wick project")]
    NoProject,
    #[error("project.json is not valid JSON: {0}")]
    BadJson(String),
    #[error("project.json has no project object and no objects table")]
    NoProjectObject,
    #[error("{0} is over the limit")]
    TooLarge(&'static str),
    #[error("the field {0} is missing or out of range")]
    BadField(&'static str),
    #[error("the object {0} is missing, or is not a {1}")]
    BadObject(String, &'static str),
    #[error("the object {0} contains itself")]
    Cycle(String),
    #[error("frame spans in a layer overlap")]
    Overlap,
    #[error("a path {0}")]
    BadPath(&'static str),
    #[error("a picture: {0}")]
    Picture(String),
    #[error("the project has no root clip")]
    NoRootClip,
    #[error("the imported document is invalid: {0}")]
    Invalid(String),
}
