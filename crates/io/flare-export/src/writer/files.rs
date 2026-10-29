//! Writing an exported project to a folder or a .fla.

use std::fs;
use std::io::Write;
use std::path::Path;

use super::*;

// ---- writing --------------------------------------------------------------------------

/// Writes the XFL folder to `dir`, which must not exist. The folder appears only when it is
/// complete: files go to a staging directory beside it that is renamed into place, and that is
/// removed again if anything goes wrong.
pub fn write_directory(output: &Output, dir: &Path) -> Result<(), ExportError> {
    if dir.exists() {
        return Err(ExportError::AlreadyExists(dir.to_owned()));
    }
    let parent = parent_of(dir);
    fs::create_dir_all(parent)?;
    let staging = tempfile::Builder::new()
        .prefix(".xfl-")
        .tempdir_in(parent)?;
    for (name, body) in &output.files {
        let path = staging.path().join(name);
        if let Some(directory) = path.parent() {
            fs::create_dir_all(directory)?;
        }
        fs::write(path, body)?;
    }
    // The `.xfl` stub that tells Animate a folder is an XFL document.
    fs::write(
        staging.path().join(format!("{}.xfl", output.name)),
        "PROXY-CS5",
    )?;
    fs::rename(staging.keep(), dir)?;
    Ok(())
}

fn parent_of(path: &Path) -> &Path {
    path.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
}

/// The bytes of a zipped modern `.fla`, for a place with no file system to write to.
pub fn fla_bytes(output: &Output) -> Result<Vec<u8>, ExportError> {
    let mut cursor = std::io::Cursor::new(Vec::new());
    write_zip(&mut cursor, output)?;
    Ok(cursor.into_inner())
}

fn write_zip(sink: &mut (impl Write + std::io::Seek), output: &Output) -> Result<(), ExportError> {
    let mut zip = zip::ZipWriter::new(sink);
    let stored =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let deflated = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    zip.start_file("mimetype", stored)?;
    zip.write_all(b"application/vnd.adobe.xfl")?;
    for (name, body) in &output.files {
        zip.start_file(name.as_str(), deflated)?;
        zip.write_all(body.as_bytes())?;
    }
    zip.finish()?;
    Ok(())
}

/// Writes a zipped modern `.fla` to `path`, which must not exist. The file is written whole
/// beside it and moved into place, and an existing file is never replaced, even by one that
/// appeared while this was writing.
pub fn write_fla(output: &Output, path: &Path) -> Result<(), ExportError> {
    if path.exists() {
        return Err(ExportError::AlreadyExists(path.to_owned()));
    }
    let parent = parent_of(path);
    fs::create_dir_all(parent)?;
    let mut staged = tempfile::NamedTempFile::new_in(parent)?;
    write_zip(staged.as_file_mut(), output)?;
    staged.as_file_mut().flush()?;
    staged.persist_noclobber(path).map_err(|error| {
        if error.error.kind() == std::io::ErrorKind::AlreadyExists {
            ExportError::AlreadyExists(path.to_owned())
        } else {
            ExportError::Io(error.error)
        }
    })?;
    Ok(())
}
