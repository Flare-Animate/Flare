//! File sources for the importer: an unpacked XFL directory or a zipped `.fla`.

use std::cell::RefCell;
use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use crate::xfl::{FileSource, ImportError, Imported, import_xfl, read_capped};

/// Whether `name` stays inside the document: relative, `/`-separated, no `..` or empty parts.
pub(crate) fn is_safe_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('/')
        && !name.contains('\\')
        && !name.contains('\0')
        && name
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

fn unsafe_name(name: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        format!("unsafe file name {name:?}"),
    )
}

/// An unpacked XFL directory.
pub struct DirSource(pub PathBuf);

impl FileSource for DirSource {
    fn read(&self, name: &str) -> io::Result<Option<Vec<u8>>> {
        if !is_safe_name(name) {
            return Err(unsafe_name(name));
        }
        let path = self.0.join(name);
        // Refuse to follow a symlink out of the document directory.
        match std::fs::symlink_metadata(&path) {
            Ok(meta) if meta.file_type().is_symlink() => return Err(unsafe_name(name)),
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        }
        let mut file = File::open(&path)?;
        read_capped(&mut file, name).map(Some).map_err(|e| match e {
            ImportError::Io(io) => io,
            other => io::Error::new(io::ErrorKind::InvalidData, other.to_string()),
        })
    }
}

/// A zipped modern `.fla`, which is an XFL directory in an archive.
pub struct ZipSource(RefCell<zip::ZipArchive<File>>);

impl ZipSource {
    pub fn open(path: &Path) -> Result<Self, ImportError> {
        let archive = zip::ZipArchive::new(File::open(path)?)
            .map_err(|e| ImportError::NotXfl(format!("not a readable zip archive: {e}")))?;
        Ok(Self(RefCell::new(archive)))
    }
}

impl FileSource for ZipSource {
    fn read(&self, name: &str) -> io::Result<Option<Vec<u8>>> {
        if !is_safe_name(name) {
            return Err(unsafe_name(name));
        }
        let mut archive = self.0.borrow_mut();
        let mut entry = match archive.by_name(name) {
            Ok(entry) => entry,
            Err(zip::result::ZipError::FileNotFound) => return Ok(None),
            Err(error) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    error.to_string(),
                ));
            }
        };
        let mut bytes = Vec::new();
        // The declared size is not trusted; the read itself is capped.
        let mut limited = (&mut entry).take(64 * 1024 * 1024 + 1);
        limited.read_to_end(&mut bytes)?;
        Ok(Some(bytes))
    }
}

const ZIP_MAGIC: &[u8] = b"PK\x03\x04";
const CFB_MAGIC: &[u8] = &[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];

/// Imports from a path: an XFL directory, a `.xfl` stub file (its folder is used), or a
/// zipped `.fla`. Legacy binary `.fla` files are recognised and refused with a clear reason.
pub fn import_path(path: &Path) -> Result<Imported, ImportError> {
    if path.is_dir() {
        return import_xfl(&DirSource(path.to_owned()));
    }
    let mut head = [0_u8; 8];
    let read = File::open(path)?.read(&mut head)?;
    let head = &head[..read];
    if head.starts_with(ZIP_MAGIC) {
        return import_xfl(&ZipSource::open(path)?);
    }
    if head.starts_with(CFB_MAGIC) {
        return Err(ImportError::NotXfl(
            "this is a legacy binary FLA (pre-CS5); open it in Animate and save as XFL or a modern FLA".into(),
        ));
    }
    if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("xfl"))
        && let Some(parent) = path.parent()
    {
        return import_xfl(&DirSource(parent.to_owned()));
    }
    Err(ImportError::NotXfl(
        "expected an XFL directory, a modern FLA (zip) or an .xfl file".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_plain_relative_names_are_safe() {
        for ok in ["DOMDocument.xml", "LIBRARY/Symbol 1.xml", "a/b/c.xml"] {
            assert!(is_safe_name(ok), "{ok}");
        }
        for bad in [
            "",
            "/etc/passwd",
            "../x",
            "a/../b",
            "a//b",
            "./a",
            "a\\b",
            "LIBRARY/",
            "x\0y",
        ] {
            assert!(!is_safe_name(bad), "{bad:?}");
        }
    }
}
