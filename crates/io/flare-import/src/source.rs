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
    Diagnostic, FileSource, ImportError, Imported, Severity, SymbolFiles, import_xfl,
    import_xfl_with_files, read_capped,
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

    /// The one `.fla` file inside a ZIP wrapper, if the archive has exactly one and nothing else
    /// named `.fla`.
    fn wrapped_fla(&self) -> Result<Option<Vec<u8>>, ImportError> {
        let mut archive = self.0.borrow_mut();
        if archive.len() > 4_096 {
            return Err(ImportError::NotXfl(
                "the archive has more than 4096 entries".into(),
            ));
        }

        let mut fla = None;
        for index in 0..archive.len() {
            let mut entry = archive
                .by_index(index)
                .map_err(|error| ImportError::NotXfl(format!("cannot read zip entry: {error}")))?;
            let name = entry.name().map_err(|error| {
                ImportError::NotXfl(format!("cannot read zip entry name: {error}"))
            })?;
            if entry.is_dir()
                || !name
                    .rsplit('/')
                    .next()
                    .is_some_and(|part| part.to_ascii_lowercase().ends_with(".fla"))
            {
                continue;
            }
            if fla.is_some() {
                return Err(ImportError::NotXfl(
                    "the archive contains more than one .fla file".into(),
                ));
            }
            if entry.size() > flare_document::MAX_ORIGIN_BYTES as u64 {
                return Err(ImportError::NotXfl(format!(
                    "the .fla inside the archive exceeds the {} byte limit",
                    flare_document::MAX_ORIGIN_BYTES
                )));
            }
            let mut bytes = Vec::new();
            (&mut entry)
                .take(flare_document::MAX_ORIGIN_BYTES as u64 + 1)
                .read_to_end(&mut bytes)?;
            if bytes.len() > flare_document::MAX_ORIGIN_BYTES {
                return Err(ImportError::NotXfl(format!(
                    "the .fla inside the archive exceeds the {} byte limit",
                    flare_document::MAX_ORIGIN_BYTES
                )));
            }
            fla = Some(bytes);
        }
        Ok(fla)
    }

    /// The compiled clip a component library (.swc) carries: a `library.swf` at the archive's
    /// root, with no XFL document beside it. Read whole, bounded like a `.fla`.
    fn root_library_swf(&self) -> Result<Option<Vec<u8>>, ImportError> {
        let mut archive = self.0.borrow_mut();
        if archive.len() > 4_096 {
            return Err(ImportError::NotXfl(
                "the archive has more than 4096 entries".into(),
            ));
        }
        for index in 0..archive.len() {
            let mut entry = archive
                .by_index(index)
                .map_err(|error| ImportError::NotXfl(format!("cannot read zip entry: {error}")))?;
            let name = entry.name().map_err(|error| {
                ImportError::NotXfl(format!("cannot read zip entry name: {error}"))
            })?;
            if entry.is_dir() || !name.eq_ignore_ascii_case("library.swf") {
                continue;
            }
            let mut bytes = Vec::new();
            (&mut entry)
                .take(flare_document::MAX_ORIGIN_BYTES as u64 + 1)
                .read_to_end(&mut bytes)?;
            if bytes.len() > flare_document::MAX_ORIGIN_BYTES {
                return Err(ImportError::NotXfl(format!(
                    "the .swf inside the archive exceeds the {} byte limit",
                    flare_document::MAX_ORIGIN_BYTES
                )));
            }
            return Ok(Some(bytes));
        }
        Ok(None)
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

/// Imports a zipped `.fla` held in memory and keeps direct XFL archives byte for byte as the source.
fn import_fla_kept(bytes: Vec<u8>, name: &str) -> Result<Imported, ImportError> {
    let source = ZipSource::from_reader(Cursor::new(&bytes[..]))?;
    if source.read("DOMDocument.xml")?.is_none()
        && let Some(fla) = source.wrapped_fla()?
    {
        return import_wrapped_fla(&fla);
    }
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

/// Imports the one FLA inside a ZIP wrapper as a new document, never as the wrapper's source.
fn import_wrapped_fla(bytes: &[u8]) -> Result<Imported, ImportError> {
    if bytes.starts_with(&legacy::CFB_MAGIC) {
        return legacy::import_legacy_bytes(bytes);
    }
    if looks_like_swf(bytes) {
        return import_swf_bytes(bytes);
    }

    let source = ZipSource::from_reader(Cursor::new(bytes))?;
    if source.read("DOMDocument.xml")?.is_none() {
        return Err(ImportError::NotXfl(
            "the .fla inside the archive is not an XFL document".into(),
        ));
    }
    let (imported, _) = import_xfl_with_files(&source)?;
    Ok(imported)
}

/// Imports the compiled clip a component library (.swc) carries. The .swc itself is not kept:
/// Flare does not rewrite a component library, so an export rebuilds from what was understood.
fn import_compiled_library(bytes: Vec<u8>) -> Result<Imported, ImportError> {
    let mut imported = import_swf_bytes(&bytes)?;
    keep::not_kept(
        &mut imported.diagnostics,
        "it is the compiled clip inside a component library (.swc), which Flare does not rewrite",
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
/// compound file, else a (zipped) `.fla` — or the compiled clip inside a `.swc`. Direct movies
/// and `.fla` files are kept as the source.
pub fn import_bytes(bytes: Vec<u8>) -> Result<Imported, ImportError> {
    if looks_like_swf(&bytes) {
        import_swf_kept(bytes, "Document")
    } else if crate::looks_like_ppm(&bytes) {
        crate::flipnote::import("Flipnote.ppm", &bytes)
    } else if crate::looks_like_kwz(&bytes) {
        crate::flipnote::import("Flipnote.kwz", &bytes)
    } else if bytes.starts_with(&legacy::CFB_MAGIC) {
        legacy::import_legacy_bytes(&bytes)
    } else if crate::looks_like_wick(&bytes) {
        crate::wick::import("Project.wick", &bytes)
    } else if looks_like_flash_project(&bytes) {
        // A .flp read whole can name the files beside it, but here there is no beside: the
        // person who has only the project file is sent to the document it names.
        Err(ImportError::Foreign(
            "a Flash project file (.flp) lists the files beside it, which cannot be read \
             from here; open the .fla or .swf it names"
                .into(),
        ))
    } else if bytes.starts_with(ZIP_MAGIC)
        && let Some(library) = library_swf_in_zip(&bytes)?
    {
        import_compiled_library(library)
    } else {
        import_fla_bytes(bytes)
    }
}

/// Whether `bytes` are the XML of a Flash project file (.flp). Scanned, not parsed: the files
/// differ by Flash version, and the header word is what all of them share.
fn looks_like_flash_project(bytes: &[u8]) -> bool {
    let head = &bytes[..bytes.len().min(64 * 1024)];
    String::from_utf8_lossy(head)
        .to_ascii_lowercase()
        .contains("flash_project")
}

/// The compiled clip inside a `.swc`'s zip, if the archive is one.
fn library_swf_in_zip(bytes: &[u8]) -> Result<Option<Vec<u8>>, ImportError> {
    ZipSource::from_reader(Cursor::new(bytes))?.root_library_swf()
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
/// zipped `.fla`, a ZIP wrapper containing one `.fla`, or a Flash movie (`.swf`). Legacy binary
/// `.fla` files import stage, library and recoverable layer metadata; artwork and keyframe contents are not read.
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
        let source = ZipSource::open(path)?;
        if source.read("DOMDocument.xml")?.is_none()
            && let Some(fla) = source.wrapped_fla()?
        {
            return import_wrapped_fla(&fla);
        }
        if let Some(library) = source.root_library_swf()? {
            return import_compiled_library(library);
        }
        let mut imported = import_xfl(&source)?;
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
    // A Flash project file (.flp) is a list of the files beside it; the one Flash opens by
    // default is imported. Its own extension keeps it out of the movie sniffing above.
    if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("flp"))
    {
        return import_flp_path(path);
    }
    // A file named .swf that does not start like a movie is not an XFL either: say what it is.
    if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("swf") || e.eq_ignore_ascii_case("swt"))
    {
        return Err(ImportError::NotSwf(
            "it does not start like a Flash movie".into(),
        ));
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
    if let Some(reason) = crate::file_kinds::unsupported_reason(path) {
        let file = path.file_name().and_then(|n| n.to_str()).unwrap_or("file");
        return Err(ImportError::Foreign(format!("{file}: {reason}")));
    }
    Err(ImportError::NotXfl(
        "expected an XFL directory, a modern FLA (zip) or an .xfl file".into(),
    ))
}

/// The most text a Flash project file (.flp) may hold.
const MAX_FLP_BYTES: usize = 4 * 1024 * 1024;

/// The kinds of file a Flash project names that Flare imports itself. Anything else in the list
/// (scripts, pictures, sounds) is counted and named, not opened.
const FLP_TARGETS: [&str; 4] = ["fla", "xfl", "swf", "swc"];

/// Imports the Flash project file at `path`: the file Flash opens by default out of the files it
/// lists beside itself, or its first listed Flash document. The project XML itself is not kept —
/// Flare imports one document, and the project's other files are named in a note.
fn import_flp_path(path: &Path) -> Result<Imported, ImportError> {
    let file = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("project")
        .to_owned();
    let refused = |reason: &str| ImportError::Foreign(format!("{file}: {reason}"));
    let mut text = String::new();
    File::open(path)?
        .take(MAX_FLP_BYTES as u64 + 1)
        .read_to_string(&mut text)
        .map_err(|_| refused("not UTF-8 text"))?;
    if text.len() > MAX_FLP_BYTES {
        return Err(ImportError::TooLarge("the Flash project".into()));
    }
    if !text.to_ascii_lowercase().contains("flash_project") {
        return Err(refused(
            "not a Flash project file: it has no <flash_project> header",
        ));
    }
    // Flash 8 marks the file its Project panel opens first; without one, the first listed
    // Flash document is the project's movie, as a person opening the project would take it.
    let default_file = attr_value(&text, "flashDefaultFile")
        .or_else(|| attr_value(&text, "flashdefaultfile"))
        .filter(|name| names_a_target(name));
    let listed: Vec<String> = attr_values(&text, "filename")
        .into_iter()
        .chain(attr_values(&text, "name"))
        .filter(|name| names_a_target(name))
        .collect();
    let chosen = match default_file.or(listed.first().cloned()) {
        Some(name) => name,
        None => {
            return Err(refused(
                "the Flash project lists no .fla, .xfl, .swf or .swc file to import",
            ));
        }
    };
    let name = safe_project_reference(&chosen)
        .ok_or_else(|| refused("the file it names does not stay beside the project"))?;
    let folder = match path.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => dir.to_path_buf(),
        _ => std::path::PathBuf::from("."),
    };
    let target = folder.join(&name);
    let others = listed
        .iter()
        .filter(|candidate| **candidate != chosen)
        .count();
    let mut imported = import_path(&target)?;
    if others > 0 {
        imported.diagnostics.push(Diagnostic {
            severity: Severity::Info,
            location: file.clone(),
            message: format!(
                "the Flash project lists {others} more file(s) that were not imported"
            ),
        });
    }
    imported.diagnostics.push(Diagnostic {
        severity: Severity::Info,
        location: file,
        message: format!("the Flash project's default file is {name}"),
    });
    Ok(imported)
}

/// Whether a name from a Flash project ends in a kind Flare imports.
fn names_a_target(name: &str) -> bool {
    let lowered = name.replace('\\', "/").to_ascii_lowercase();
    FLP_TARGETS
        .iter()
        .any(|ext| lowered.rsplit('.').next().is_some_and(|e| e == *ext))
}

/// A file reference out of a Flash project, normalized to `/` and checked to stay beside the
/// project file: relative, no `..`, no empty parts, and no drive letter. `None` for anything else.
fn safe_project_reference(name: &str) -> Option<String> {
    let normalized = name.replace('\\', "/");
    if normalized.contains(':') || !is_safe_name(&normalized) {
        return None;
    }
    Some(normalized)
}

/// The values of the first attribute `name` in XML `text`, if it has one.
fn attr_value(text: &str, name: &str) -> Option<String> {
    attr_values(text, name).into_iter().next()
}

/// Every value of every attribute `name` in XML `text`, in order. A tolerant scan, like the
/// rest of the tolerant readers: Flash's project file is XML, but its shapes differ by version.
fn attr_values(text: &str, name: &str) -> Vec<String> {
    let marker = format!("{name}=\"");
    let mut values = Vec::new();
    let mut from = 0;
    while from < text.len() {
        let Some(found) = text.get(from..).and_then(|rest| rest.find(&marker)) else {
            break;
        };
        let start = from + found;
        // `name="` also sits inside `filename="`; an attribute begins after a space or a tag,
        // so one whose preceding byte is a letter belongs to the longer name.
        if start > 0 && text.as_bytes()[start - 1].is_ascii_alphabetic() {
            from = start + marker.len();
            continue;
        }
        let at = start + marker.len();
        let Some(after) = text.get(at..) else { break };
        let Some(end) = after.find('"') else { break };
        values.push(after.get(..end).unwrap_or("").to_owned());
        from = at + end + 1;
    }
    values
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
