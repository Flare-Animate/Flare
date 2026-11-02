//! Saving over an existing file or folder: the same files as the writers in `files`, but the
//! previous one is replaced, and only once the new one is whole.

use std::fs;
use std::io::Write;
use std::path::Path;

use super::files::{parent_of, stage_directory, write_zip};
use super::{ExportError, Output};

/// The file name of an XFL folder's document, whose presence says a folder is one.
const DOCUMENT: &str = "DOMDocument.xml";

/// Writes a zipped modern `.fla` to `path`, replacing the file that is there. The new file is
/// written whole beside it and moved over it, so a failure at any point leaves the old file as
/// it was; a read-only file is refused rather than replaced behind the system's back.
pub fn replace_fla(output: &Output, path: &Path) -> Result<(), ExportError> {
    let existing = fs::metadata(path).ok();
    if let Some(meta) = &existing {
        if meta.is_dir() {
            return Err(ExportError::AlreadyExists(path.to_owned()));
        }
        if meta.permissions().readonly() {
            return Err(ExportError::ReadOnly(path.to_owned()));
        }
    }
    let parent = parent_of(path);
    fs::create_dir_all(parent)?;
    let mut staged = tempfile::NamedTempFile::new_in(parent)?;
    write_zip(staged.as_file_mut(), output)?;
    staged.as_file_mut().flush()?;
    staged.as_file().sync_all()?;
    if let Some(meta) = existing {
        // The temporary file is private to its owner; the document keeps the permissions it had.
        fs::set_permissions(staged.path(), meta.permissions())?;
    }
    staged
        .persist(path)
        .map_err(|error| ExportError::Io(error.error))?;
    Ok(())
}

/// Writes the XFL folder `dir`, replacing the one that is there. Only a folder that holds a
/// `DOMDocument.xml` is replaced; any other folder, or a file, is refused. The new folder is
/// built beside the old one, the old one is moved aside, the new one moved into place and the old
/// one deleted, and if the move into place fails the old one is moved back.
pub fn replace_directory(output: &Output, dir: &Path) -> Result<(), ExportError> {
    let exists = match fs::metadata(dir) {
        Ok(meta) if meta.is_dir() => true,
        Ok(_) => return Err(ExportError::NotXfl(dir.to_owned())),
        Err(_) => false,
    };
    if exists && !dir.join(DOCUMENT).is_file() {
        return Err(ExportError::NotXfl(dir.to_owned()));
    }
    if !exists {
        return super::write_directory(output, dir);
    }
    let parent = parent_of(dir);
    let staging = stage_directory(output, parent)?;
    let aside = tempfile::Builder::new()
        .prefix(".xfl-old-")
        .tempdir_in(parent)?;
    // `rename` over an empty directory is how the old folder is put aside whole.
    let old = aside.path().join("old");
    fs::rename(dir, &old)?;
    if let Err(error) = fs::rename(staging.path(), dir) {
        // Put the old folder back; the new one is removed with its staging directory. If even
        // that fails the old folder is left where it is, not deleted with its holder.
        if fs::rename(&old, dir).is_err() {
            let _ = aside.keep();
        }
        return Err(ExportError::Io(error));
    }
    // The staging directory has been moved; there is nothing left for it to remove.
    let _ = staging.keep();
    // `aside` removes the old folder when it goes out of scope; a failure to do so only leaves
    // the hidden folder behind.
    drop(aside);
    Ok(())
}
