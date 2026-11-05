//! The pictures of a Moho project, read from the project's own folder. A path that leaves that
//! folder (`..`, an absolute path, a drive, or a link that points out) is refused, and nothing
//! else is opened. Only PNG pictures are read, within size limits.

use std::io::Read;
use std::path::{Component, Path, PathBuf};

use crate::bitmap::decode_png;

const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
/// Largest encoded picture read.
const MAX_PICTURE_BYTES: u64 = 32 * 1024 * 1024;
/// Most decoded pixel bytes all the pictures of one project may hold.
const MAX_DECODED_TOTAL: u64 = 512 * 1024 * 1024;

pub(crate) struct ProjectFolder {
    root: PathBuf,
    decoded: u64,
}

impl ProjectFolder {
    pub(crate) fn new(dir: &Path) -> Result<Self, String> {
        let root = std::fs::canonicalize(dir).map_err(|e| format!("project folder: {e}"))?;
        Ok(Self { root, decoded: 0 })
    }
}

/// The relative path as a file path, or why it is not one inside a folder. Moho may write
/// Windows separators, so both are accepted.
fn inside(path: &str) -> Result<PathBuf, String> {
    let unified = path.replace('\\', "/");
    if unified.is_empty() || unified.starts_with('/') || unified.contains(':') {
        return Err("the path is not relative to the project folder".to_owned());
    }
    let mut out = PathBuf::new();
    for component in Path::new(&unified).components() {
        match component {
            Component::Normal(part) => out.push(part),
            _ => return Err("the path leaves the project folder".to_owned()),
        }
    }
    Ok(out)
}

impl flare_moho::ImageSource for ProjectFolder {
    fn load(&mut self, path: &str) -> Result<flare_moho::Picture, String> {
        let relative = inside(path)?;
        let full = std::fs::canonicalize(self.root.join(&relative))
            .map_err(|_| "not found in the project folder".to_owned())?;
        if !full.starts_with(&self.root) {
            return Err("the file is outside the project folder".to_owned());
        }
        let meta = std::fs::metadata(&full).map_err(|e| e.to_string())?;
        if !meta.is_file() {
            return Err("not a file".to_owned());
        }
        if meta.len() > MAX_PICTURE_BYTES {
            return Err(format!("{} bytes is over the picture limit", meta.len()));
        }
        let mut bytes = Vec::new();
        std::fs::File::open(&full)
            .map_err(|e| e.to_string())?
            .take(MAX_PICTURE_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if !bytes.starts_with(&PNG_SIGNATURE) {
            return Err("only PNG pictures are read".to_owned());
        }
        let picture = decode_png(path, &bytes).map_err(|e| e.to_string())?;
        self.decoded = self
            .decoded
            .saturating_add(u64::try_from(picture.rgba.len()).unwrap_or(u64::MAX));
        if self.decoded > MAX_DECODED_TOTAL {
            return Err("the project's pictures are over the decoding limit".to_owned());
        }
        Ok(flare_moho::Picture {
            width: picture.width,
            height: picture.height,
            rgba: picture.rgba,
        })
    }
}
