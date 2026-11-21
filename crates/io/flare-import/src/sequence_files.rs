//! The picture files of an image sequence, read from the disk: the one way the desktop app, the
//! command line and the MCP server turn what a person points at (a folder, or the files
//! themselves) into the `(name, bytes)` list [`import_image_sequence`](crate::import_image_sequence)
//! decodes. Everything named is untrusted: counts and sizes are bounded before bytes are read.

use std::path::{Path, PathBuf};

use crate::bitmap::{MAX_DECODED_BYTES, MAX_IMAGE_FRAMES, bad};
use crate::xfl::ImportError;

/// The endings of the files a sequence is made of (the decoder reads PNG).
pub const SEQUENCE_EXTENSIONS: [&str; 1] = ["png"];

fn is_picture(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| {
        SEQUENCE_EXTENSIONS
            .iter()
            .any(|x| e.eq_ignore_ascii_case(x))
    })
}

/// The files `inputs` name: a folder stands for the pictures directly inside it, a file for
/// itself. Not sorted: the importer orders by natural name.
pub fn sequence_paths(inputs: &[PathBuf]) -> Result<Vec<PathBuf>, ImportError> {
    let mut out = Vec::new();
    for input in inputs {
        let name = input.display().to_string();
        let meta = std::fs::metadata(input).map_err(|e| bad(&name, e))?;
        if meta.is_dir() {
            let entries = std::fs::read_dir(input).map_err(|e| bad(&name, e))?;
            for entry in entries {
                let path = entry.map_err(|e| bad(&name, e))?.path();
                if path.is_file() && is_picture(&path) {
                    out.push(path);
                    if out.len() > MAX_IMAGE_FRAMES {
                        return Err(bad(&name, format!("more than {MAX_IMAGE_FRAMES} pictures")));
                    }
                }
            }
        } else if meta.is_file() {
            out.push(input.clone());
        } else {
            return Err(bad(&name, "not a file or a folder"));
        }
        if out.len() > MAX_IMAGE_FRAMES {
            return Err(bad(&name, format!("more than {MAX_IMAGE_FRAMES} pictures")));
        }
    }
    if out.is_empty() {
        return Err(bad("sequence", "no pictures found"));
    }
    Ok(out)
}

/// Reads the files [`sequence_paths`] finds, each named by its file name, refusing a sequence
/// whose files together are larger than the decoder will hold.
pub fn read_sequence_files(inputs: &[PathBuf]) -> Result<Vec<(String, Vec<u8>)>, ImportError> {
    let paths = sequence_paths(inputs)?;
    let mut total = 0_u64;
    let mut files = Vec::with_capacity(paths.len());
    for path in paths {
        let name = path.file_name().map_or_else(
            || path.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        );
        let size = std::fs::metadata(&path).map_err(|e| bad(&name, e))?.len();
        total += size;
        if size > crate::still::MAX_STILL_BYTES as u64 || total > MAX_DECODED_BYTES {
            return Err(bad(&name, "the sequence's files are too large"));
        }
        let bytes = std::fs::read(&path).map_err(|e| bad(&name, e))?;
        files.push((name, bytes));
    }
    Ok(files)
}
