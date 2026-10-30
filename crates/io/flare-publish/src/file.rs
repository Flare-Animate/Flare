//! Publishing straight to a file that appears only when it is complete.

use std::io::Write;
use std::path::{Path, PathBuf};

use flare_document::Document;
use thiserror::Error;

use crate::{PublishError, PublishOptions, Published, publish_swf};

#[derive(Debug, Error)]
pub enum PublishFileError {
    #[error("{} already exists", .0.display())]
    Exists(PathBuf),
    #[error(transparent)]
    Publish(#[from] PublishError),
    #[error("cannot write the file: {0}")]
    Io(#[from] std::io::Error),
}

/// Publishes `document` to `out` as a SWF. Never overwrites: the bytes are written beside the
/// target and renamed into place, so a failure leaves nothing behind.
pub fn publish_to_file(
    document: &Document,
    options: &PublishOptions,
    out: &Path,
) -> Result<Published, PublishFileError> {
    if out.exists() {
        return Err(PublishFileError::Exists(out.to_path_buf()));
    }
    let published = publish_swf(document, options)?;
    let directory = out
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut staged = tempfile::NamedTempFile::new_in(directory)?;
    staged.write_all(&published.bytes)?;
    // Refuses atomically if the file appeared since the check above.
    staged.persist_noclobber(out).map_err(|error| {
        if error.error.kind() == std::io::ErrorKind::AlreadyExists {
            PublishFileError::Exists(out.to_path_buf())
        } else {
            error.error.into()
        }
    })?;
    Ok(published)
}

/// Publishes `document` to `out` as an HTML5 page, under the same rules as [`publish_to_file`].
pub fn publish_html5_to_file(
    document: &Document,
    options: &crate::Html5Options,
    out: &Path,
) -> Result<crate::PublishedHtml, PublishFileError> {
    if out.exists() {
        return Err(PublishFileError::Exists(out.to_path_buf()));
    }
    let published = crate::publish_html5(document, options)?;
    let directory = out
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut staged = tempfile::NamedTempFile::new_in(directory)?;
    staged.write_all(published.html.as_bytes())?;
    staged.persist_noclobber(out).map_err(|error| {
        if error.error.kind() == std::io::ErrorKind::AlreadyExists {
            PublishFileError::Exists(out.to_path_buf())
        } else {
            error.error.into()
        }
    })?;
    Ok(published)
}
