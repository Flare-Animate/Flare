//! Writing an exported project to a folder or a .fla.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use super::*;

// ---- writing --------------------------------------------------------------------------

/// Writes the XFL folder to `dir`, which must not exist. The folder appears only when it is
/// complete: files go to a sibling directory that is renamed into place.
pub fn write_directory(output: &Output, dir: &Path) -> Result<(), ExportError> {
    if dir.exists() {
        return Err(ExportError::AlreadyExists(dir.to_owned()));
    }
    let parent = dir
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;
    let staging = parent.join(format!(
        ".{}.partial-{}",
        dir.file_name()
            .map_or_else(|| "xfl".into(), |n| n.to_string_lossy().into_owned()),
        std::process::id()
    ));
    let result = (|| -> Result<(), ExportError> {
        fs::create_dir_all(&staging)?;
        for (name, body) in &output.files {
            let path = staging.join(name);
            if let Some(directory) = path.parent() {
                fs::create_dir_all(directory)?;
            }
            fs::write(path, body)?;
        }
        // The `.xfl` stub that tells Animate a folder is an XFL document.
        fs::write(staging.join(format!("{}.xfl", output.name)), "PROXY-CS5")?;
        fs::rename(&staging, dir)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&staging);
    }
    result
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

/// Writes a zipped modern `.fla` to `path`, which must not exist.
pub fn write_fla(output: &Output, path: &Path) -> Result<(), ExportError> {
    if path.exists() {
        return Err(ExportError::AlreadyExists(path.to_owned()));
    }
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;
    let (staging, mut file) = tempfile_in(parent)?;
    let result = (|| -> Result<(), ExportError> {
        write_zip(&mut file, output)?;
        file.flush()?;
        fs::rename(&staging, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&staging);
    }
    result
}

/// A uniquely named file in `dir` to stage output in; renamed into place on success.
pub(super) fn tempfile_in(dir: &Path) -> Result<(PathBuf, fs::File), ExportError> {
    for attempt in 0..100 {
        let path = dir.join(format!(
            ".flare-export-{}-{attempt}.tmp",
            std::process::id()
        ));
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.into()),
        }
    }
    Err(std::io::Error::other("could not create a staging file").into())
}
