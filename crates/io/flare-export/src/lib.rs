//! Export of a Flare document as XFL (an unpacked folder) or a zipped `.fla`.
//!
//! XFL stores outlines as lines and quadratic curves only, so cubic curves are approximated
//! within a stated tolerance and that is reported. Only what Flare can represent is written;
//! the output has been checked by reading it back with `flare-import`, not by opening it in
//! Adobe Animate, so treat compatibility with Animate as unverified.
#![forbid(unsafe_code)]

pub mod curves;
pub mod lossless;
pub mod psd;
pub mod svg;
mod writer;

pub use writer::{
    Diagnostic, ExportError, Output, Severity, export_xfl, fla_bytes, replace_directory,
    replace_fla, write_directory, write_fla,
};
