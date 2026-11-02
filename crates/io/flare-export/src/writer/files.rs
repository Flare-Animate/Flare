//! Writing an exported project to a folder or a .fla.

use std::fs;
use std::io::Write;
use std::path::Path;

use super::*;

// ---- writing --------------------------------------------------------------------------

use std::borrow::Cow;

use flare_document::{OriginData, OriginKind};

/// The text of the `.xfl` stub that tells Animate a folder is an XFL document.
const STUB: &str = "PROXY-CS5";

/// One file of the output, ready to write. A name ending in `/` is an empty folder.
type Entry<'a> = (Cow<'a, str>, Cow<'a, [u8]>);

/// Whether `name` is the `.xfl` stub at the root of a folder.
fn is_stub(name: &str) -> bool {
    !name.contains('/') && name.to_ascii_lowercase().ends_with(".xfl")
}

/// The files to write for an output that is not the original itself: what Flare wrote and what
/// it carried over, in the order of the original and then the rest.
fn planned(output: &Output) -> Vec<Entry<'_>> {
    let mut entries: Vec<Entry<'_>> = output
        .files
        .iter()
        .map(|(n, b)| (Cow::Borrowed(n.as_str()), Cow::Borrowed(b.as_bytes())))
        .chain(
            output
                .kept
                .iter()
                .map(|(n, b)| (Cow::Borrowed(n.as_str()), Cow::Borrowed(b.as_slice()))),
        )
        .collect();
    if !output.order.is_empty() {
        let rank: std::collections::HashMap<&str, usize> = output
            .order
            .iter()
            .enumerate()
            .map(|(i, n)| (n.as_str(), i))
            .collect();
        // Stable: what is not in the original keeps its place, after it.
        entries.sort_by_key(|(n, _)| rank.get(n.as_ref()).copied().unwrap_or(usize::MAX));
    }
    entries
}

/// The files of the original an exact output stands for, to write as a folder or an archive.
fn original_entries(output: &Output) -> Result<Vec<Entry<'static>>, ExportError> {
    let Some(origin) = &output.exact else {
        return Ok(Vec::new());
    };
    let files = match origin.data() {
        OriginData::Files(files) => files.clone(),
        OriginData::Bytes(_) => {
            let entries = crate::lossless::entries::entries_of(origin);
            if let Some(why) = entries.skipped.first() {
                return Err(ExportError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("the original cannot be unpacked: {why}"),
                )));
            }
            entries.files
        }
    };
    Ok(files
        .into_iter()
        .map(|f| (Cow::Owned(f.name), Cow::Owned(f.bytes)))
        .collect())
}

/// Writes the XFL folder to `dir`, which must not exist. The folder appears only when it is
/// complete: files go to a staging directory beside it that is renamed into place, and that is
/// removed again if anything goes wrong.
pub fn write_directory(output: &Output, dir: &Path) -> Result<(), ExportError> {
    if dir.exists() {
        return Err(ExportError::AlreadyExists(dir.to_owned()));
    }
    let parent = parent_of(dir);
    fs::create_dir_all(parent)?;
    let staging = stage_directory(output, parent)?;
    fs::rename(staging.keep(), dir)?;
    Ok(())
}

/// Writes the whole XFL folder into a new staging directory in `parent`, which removes itself
/// unless kept.
pub(super) fn stage_directory(
    output: &Output,
    parent: &Path,
) -> Result<tempfile::TempDir, ExportError> {
    let staging = tempfile::Builder::new()
        .prefix(".xfl-")
        .tempdir_in(parent)?;
    let entries = if output.exact.is_some() {
        original_entries(output)?
    } else {
        planned(output)
    };
    let has_stub = entries.iter().any(|(n, _)| is_stub(n));
    for (name, body) in &entries {
        let path = staging.path().join(name.as_ref());
        if name.ends_with('/') {
            fs::create_dir_all(path)?;
            continue;
        }
        if let Some(directory) = path.parent() {
            fs::create_dir_all(directory)?;
        }
        fs::write(path, body)?;
    }
    // The `.xfl` stub that tells Animate a folder is an XFL document. A folder that came with
    // its own is written back as it was, and an exact copy of one is not given a second.
    let exact_folder = output
        .exact
        .as_ref()
        .is_some_and(|o| o.kind() == OriginKind::Xfl);
    if !has_stub && !exact_folder {
        fs::write(staging.path().join(format!("{}.xfl", output.name)), STUB)?;
    }
    Ok(staging)
}

pub(super) fn parent_of(path: &Path) -> &Path {
    path.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
}

/// The bytes of a zipped modern `.fla`, for a place with no file system to write to. For an
/// unchanged `.fla` import these are the original file's bytes.
pub fn fla_bytes(output: &Output) -> Result<Vec<u8>, ExportError> {
    let mut cursor = std::io::Cursor::new(Vec::new());
    write_zip(&mut cursor, output)?;
    Ok(cursor.into_inner())
}

pub(super) fn write_zip(
    sink: &mut (impl Write + std::io::Seek),
    output: &Output,
) -> Result<(), ExportError> {
    if let Some(origin) = &output.exact
        && let OriginData::Bytes(bytes) = origin.data()
        && origin.kind() == OriginKind::Fla
    {
        sink.write_all(bytes)?;
        return Ok(());
    }
    let entries = if output.exact.is_some() {
        original_entries(output)?
    } else {
        planned(output)
    };
    let mut zip = zip::ZipWriter::new(sink);
    let stored =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let deflated = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    if !entries.iter().any(|(n, _)| n == "mimetype") {
        zip.start_file("mimetype", stored)?;
        zip.write_all(b"application/vnd.adobe.xfl")?;
    }
    for (name, body) in &entries {
        // A folder's stub names the folder; an archive has no use for it.
        if output.from_folder && is_stub(name) {
            continue;
        }
        if name.ends_with('/') {
            zip.add_directory(name.as_ref(), deflated)?;
            continue;
        }
        zip.start_file(
            name.as_ref(),
            if name == "mimetype" { stored } else { deflated },
        )?;
        zip.write_all(body)?;
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
