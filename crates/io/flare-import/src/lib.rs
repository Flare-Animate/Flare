//! Import of modern Flash documents: XFL directories and `.fla` files that are zip
//! archives of XFL. See [`xfl`] for exactly what is and is not imported.
#![forbid(unsafe_code)]

mod bitmap;
mod edges;
mod file_kinds;
mod foreign;
mod keep;
pub mod legacy;
mod source;
mod svg;
mod swf;
mod toonz;
mod xfl;
mod xml;

pub use bitmap::{MAX_IMAGE_FRAMES, import_image_sequence};
pub use file_kinds::{
    IMPORT_EXTENSIONS, PROJECT_EXTENSIONS, PathKind, XFL_DOCUMENT_FILE, classify, is_xfl_document,
};
pub use foreign::{foreign_kind, import_foreign_bytes, import_foreign_path};
pub use source::{DirSource, ZipSource, import_bytes, import_fla_bytes, import_path};
pub use swf::{import_swf_bytes, looks_like_swf};
pub use xfl::{Diagnostic, FileSource, ImportError, Imported, Severity, frame_rate, import_xfl};
