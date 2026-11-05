//! The reasons a FlipaClip file is refused. Each names the part that failed, so the message can
//! say what to fix; a message never carries file contents.

use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FlipaError {
    #[error("not a zip archive")]
    NotZip,
    #[error("the entry {0:?} has a name that would leave the project")]
    UnsafeName(String),
    #[error("the entry {0:?} is listed twice")]
    DuplicateEntry(String),
    #[error("the entry {0:?} is corrupt")]
    CorruptEntry(String),
    #[error("{0} is over the limit")]
    TooLarge(&'static str),
    #[error("there is no meta entry, so this is not a FlipaClip project")]
    NoMeta,
    #[error("meta is not a JSON object: {0}")]
    BadMeta(&'static str),
    #[error("format {0} is not read; only format 1 is")]
    UnknownFormat(u64),
    #[error("meta field {0} is missing or out of range")]
    BadField(&'static str),
    #[error("the entry {entry:?}: {reason}")]
    BadPicture { entry: String, reason: &'static str },
}
