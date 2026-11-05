//! File sources for the importer: an unpacked XFL directory or a zipped `.fla`.

use std::cell::RefCell;
use std::fs::File;
use std::io::{self, Cursor, Read, Seek};
use std::path::{Path, PathBuf};

use std::collections::HashMap;

use flare_document::{OriginData, OriginFile, OriginKind};

use crate::keep;
use crate::legacy;
use crate::swf::{import_swf_bytes, looks_like_swf};
use crate::xfl::{
    FileSource, ImportError, Imported, SymbolFiles, import_xfl, import_xfl_with_files, read_capped,
};

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
pub struct ZipSource<R = File>(RefCell<zip::ZipArchive<R>>);

impl ZipSource {
    pub fn open(path: &Path) -> Result<Self, ImportError> {
        Self::from_reader(File::open(path)?)
    }
}

impl ZipSource<Cursor<Vec<u8>>> {
    /// A `.fla` already in memory, for a place with no file system to read from.
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, ImportError> {
        Self::from_reader(Cursor::new(bytes))
    }
}

impl<R: Read + Seek> ZipSource<R> {
    fn from_reader(reader: R) -> Result<Self, ImportError> {
        let archive = zip::ZipArchive::new(reader)
            .map_err(|e| ImportError::NotXfl(format!("not a readable zip archive: {e}")))?;
        Ok(Self(RefCell::new(archive)))
    }
}

/// An XFL folder already read into memory. Importing from it, not from the disk, is what makes
/// the kept source and the imported document agree even if the folder changes meanwhile.
struct MemorySource<'a>(HashMap<&'a str, &'a [u8]>);

impl<'a> MemorySource<'a> {
    fn new(files: &'a [OriginFile]) -> Self {
        Self(
            files
                .iter()
                .filter(|f| !f.name.ends_with('/'))
                .map(|f| (f.name.as_str(), f.bytes.as_slice()))
                .collect(),
        )
    }
}

impl FileSource for MemorySource<'_> {
    fn read(&self, name: &str) -> io::Result<Option<Vec<u8>>> {
        if !is_safe_name(name) {
            return Err(unsafe_name(name));
        }
        Ok(self.0.get(name).map(|bytes| bytes.to_vec()))
    }
}

/// Imports a zipped `.fla` held in memory and keeps it, byte for byte, as the document's source.
fn import_fla_kept(bytes: Vec<u8>, name: &str) -> Result<Imported, ImportError> {
    let source = ZipSource::from_reader(Cursor::new(&bytes[..]))?;
    let (mut imported, files) = import_xfl_with_files(&source)?;
    drop(source);
    imported.document = keep::attach(
        imported.document,
        OriginKind::Fla,
        name,
        OriginData::Bytes(bytes),
        &files,
        &mut imported.diagnostics,
    );
    Ok(imported)
}

/// Imports a Flash movie and keeps its bytes as the document's source.
fn import_swf_kept(bytes: Vec<u8>, name: &str) -> Result<Imported, ImportError> {
    let mut imported = import_swf_bytes(&bytes)?;
    imported.document = keep::attach(
        imported.document,
        OriginKind::Swf,
        name,
        OriginData::Bytes(bytes),
        &SymbolFiles::new(),
        &mut imported.diagnostics,
    );
    Ok(imported)
}

/// Imports an XFL folder, keeping all its files as the document's source when that can be done.
fn import_folder_kept(root: &Path, name: &str) -> Result<Imported, ImportError> {
    match keep::read_folder(root) {
        Ok(files) => {
            let (mut imported, symbol_files) = import_xfl_with_files(&MemorySource::new(&files))?;
            imported.document = keep::attach(
                imported.document,
                OriginKind::Xfl,
                name,
                OriginData::Files(files),
                &symbol_files,
                &mut imported.diagnostics,
            );
            Ok(imported)
        }
        Err(reason) => {
            let mut imported = import_xfl(&DirSource(root.to_owned()))?;
            keep::not_kept(&mut imported.diagnostics, &reason);
            Ok(imported)
        }
    }
}

/// Imports a `.fla` held in memory.
pub fn import_fla_bytes(bytes: Vec<u8>) -> Result<Imported, ImportError> {
    import_fla_kept(bytes, "Document")
}

/// Imports a file held in memory: a Flash movie if it starts like one, a legacy binary `.fla` if it is a
/// compound file, else a (zipped) `.fla`. A movie or a `.fla` is kept as the document's source.
pub fn import_bytes(bytes: Vec<u8>) -> Result<Imported, ImportError> {
    if looks_like_swf(&bytes) {
        import_swf_kept(bytes, "Document")
    } else if crate::looks_like_ppm(&bytes) {
        crate::flipnote::import("Flipnote.ppm", &bytes)
    } else if crate::looks_like_kwz(&bytes) {
        crate::flipnote::import("Flipnote.kwz", &bytes)
    } else if bytes.starts_with(&legacy::CFB_MAGIC) {
        legacy::import_legacy_bytes(&bytes)
    } else {
        import_fla_bytes(bytes)
    }
}

impl<R: Read + Seek> FileSource for ZipSource<R> {
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

/// Imports from a path: an XFL directory, a `.xfl` stub file or the folder's `DOMDocument.xml` (the folder is used), a
/// zipped `.fla` or a Flash movie (`.swf`). Legacy binary `.fla` files are read as a skeleton.
///
/// The source is opened for reading only; nothing is written, created or deleted beside or inside it. What was read
/// is kept with the document so an export of the unchanged import is the original (see `flare_document::Origin`).
pub fn import_path(path: &Path) -> Result<Imported, ImportError> {
    if crate::foreign_kind(path).is_some() {
        return crate::import_foreign_path(path);
    }
    if path.is_dir() {
        return import_folder_kept(path, &keep::stem(path));
    }
    let mut head = [0_u8; 8];
    let read = File::open(path)?.read(&mut head)?;
    let head = &head[..read];
    let name = keep::stem(path);
    if looks_like_swf(head) {
        let bytes = read_capped(&mut File::open(path)?, "the movie")?;
        return import_swf_kept(bytes, &name);
    }
    if head.starts_with(ZIP_MAGIC) {
        let bytes = keep::read_all_capped(File::open(path)?)?;
        if bytes.len() <= flare_document::MAX_ORIGIN_BYTES {
            return import_fla_kept(bytes, &name);
        }
        // Too big to keep: read it from the disk as before and say the original was not kept.
        drop(bytes);
        let mut imported = import_xfl(&ZipSource::open(path)?)?;
        keep::not_kept(
            &mut imported.diagnostics,
            &format!(
                "it is larger than {} bytes",
                flare_document::MAX_ORIGIN_BYTES
            ),
        );
        return Ok(imported);
    }
    if head.starts_with(&legacy::CFB_MAGIC) {
        let mut bytes = Vec::new();
        File::open(path)?
            .take(legacy::MAX_LEGACY_FILE_BYTES + 1)
            .read_to_end(&mut bytes)?;
        return legacy::import_legacy_bytes(&bytes);
    }
    if (path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("xfl"))
        || crate::is_xfl_document(path))
        && let Some(parent) = path.parent()
    {
        let parent = if parent.as_os_str().is_empty() {
            Path::new(".")
        } else {
            parent
        };
        return import_folder_kept(parent, &name);
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
