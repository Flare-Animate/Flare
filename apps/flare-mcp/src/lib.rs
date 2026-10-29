//! A Model Context Protocol server over stdio for one open Flare project.
//!
//! Agents get the same edit commands and validation as the UI: every change goes through an
//! `EditorSession`, so a failed batch changes nothing, edits are undoable, and nothing reaches
//! disk until `save`. Safety defaults: the server is read-only unless writing is allowed,
//! exports only go to a configured directory under a sanitized file name, and a mutating call
//! can carry `expected_revision` so an agent cannot overwrite changes it has not seen.
//!
//! The protocol itself (framing, initialization, schemas, dispatch) is `rmcp`'s; this crate is
//! the tools and the rules about who may call them.
#![forbid(unsafe_code)]

mod color;
mod ops;
mod params;
mod server;
mod tools;

pub use color::{parse_color, safe_file_name};
pub use server::{FlareServer, Permissions};
