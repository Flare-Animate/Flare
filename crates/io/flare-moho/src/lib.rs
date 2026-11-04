//! Moho (Lost Marble) project reader: the container, layers, bones, switch layers, keyframe
//! tracks, and the image and audio files a project refers to, read into a typed model. Also a
//! BVH reader and writer for bone hierarchies and motion ([`bvh`]).
//!
//! Reading is read-only and total. Every field has a default because Moho drops fields between
//! format generations, and every size, depth and count is bounded (see [`limits`]). Turning the
//! model into a Flare document is not built yet: [`to_flare_document`] returns a typed
//! `NotYetImplemented` error. docs/ROADMAP.md has the phases.
#![forbid(unsafe_code)]

pub mod bvh;
mod channel;
mod container;
mod convert;
mod error;
pub mod limits;
mod model;
mod parse;
mod tree;

pub use container::{Container, detect, is_moho_path, parse_bytes, read_file};
pub use convert::to_flare_document;
pub use error::MohoError;
pub use model::{
    Binding, Bone, Census, Channel, ChannelKind, Key, KeyValue, Layer, LayerKind, Project,
    SwitchCells, Track,
};
