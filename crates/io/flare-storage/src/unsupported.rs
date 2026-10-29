//! Where there is no SQLite (the browser build), a project file cannot be read or written yet.
//! Saving there goes through a download of the exported `.fla` instead.

use std::path::Path;

use flare_document::Document;

use crate::StorageError;

const MESSAGE: &str = ".flare project files are not available in the browser yet";

pub fn save(_document: &Document, _path: &Path) -> Result<(), StorageError> {
    Err(StorageError::Unsupported(MESSAGE))
}

pub fn load(_path: &Path) -> Result<Document, StorageError> {
    Err(StorageError::Unsupported(MESSAGE))
}
