//! Moho (Lost Marble) project reader: the container, layers, bones, switch layers, keyframe
//! tracks, and the image and audio files a project refers to, read into a typed model. Also a
//! BVH reader and writer for bone hierarchies and motion ([`bvh`]).
//!
//! Reading is read-only and total. Every field has a default because Moho drops fields between
//! format generations, and every size, depth and count is bounded (see [`limits`]). [`to_flare_document`] and
//! [`bvh::to_document`] turn a model into a Flare document as one validated command batch.
#![forbid(unsafe_code)]

pub mod bvh;
mod channel;
mod container;
mod convert;
mod convert_anim;
mod convert_vector;
mod error;
pub mod limits;
mod mesh;
mod model;
mod parse;
mod tree;

pub use container::{Container, detect, is_moho_path, parse_bytes, read_file};
pub use convert::{Converted, Note, Severity, to_flare_document};
pub use error::MohoError;
pub use model::{
    Binding, Bone, Census, Channel, ChannelKind, Key, KeyValue, Layer, LayerKind, Mesh, MeshCurve,
    Project, SwitchCells, Track,
};
