//! Reader for Toonz (OpenToonz lineage) files: scene XML (`.tnz`), palette XML (`.tpl`, `.plt`),
//! and header sniffing for vector (`.pli`) and raster (`.tlv`) levels.
//!
//! What is read is a typed summary of the file. Levels are not decoded yet and a scene is not
//! converted into a Flare document; both return [`ToonzError::NotYetImplemented`]. See
//! docs/ROADMAP.md for the phases and docs/SURFACES.md for the surfaces that do not have this yet.
#![forbid(unsafe_code)]

mod error;
mod levels;
mod limits;
mod palette;
mod scene;
mod sniff;
mod tree;

pub use error::ToonzError;
pub use levels::{decode_pli, decode_tlv};
pub use limits::{MAX_INPUT_BYTES, MAX_NODES};
pub use palette::{Palette, StylePage, parse_palette};
pub use scene::{CellRun, Column, FrameId, LevelDecl, Scene, SoundCells, parse_scene};
pub use sniff::{Format, sniff};
