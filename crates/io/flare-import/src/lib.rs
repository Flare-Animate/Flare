//! Import of modern Flash documents: XFL directories and `.fla` files that are zip
//! archives of XFL. See [`xfl`] for exactly what is and is not imported.
#![forbid(unsafe_code)]

mod edges;
mod file_kinds;
mod source;
mod swf;
mod xfl;
mod xml;

pub use file_kinds::{IMPORT_EXTENSIONS, PROJECT_EXTENSIONS, PathKind, classify};
pub use source::{DirSource, ZipSource, import_bytes, import_fla_bytes, import_path};
pub use swf::{import_swf_bytes, looks_like_swf};
pub use xfl::{Diagnostic, FileSource, ImportError, Imported, Severity, frame_rate, import_xfl};
