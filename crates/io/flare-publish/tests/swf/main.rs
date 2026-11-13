//! Publish documents and check the SWF by parsing it back with the `swf` crate (the reader
//! the Ruffle project uses), simulating its display list, and re-rendering what it decodes.
//! This shows the file is well-formed and says what we meant. It does not show how Flash
//! Player or Ruffle would play it.
#![allow(clippy::expect_used)]

use flare_core::{DocumentId, FrameRate, Rgba8, SceneId, ShapeId};
use flare_document::{Content, Document, PathSegment, Scene, Shape, Stage, Stroke, Timeline};
use flare_publish::{PublishError, PublishOptions, Severity, publish_swf};
use swf::Tag;

mod armature;
mod camera;
mod elements;
mod frames;
mod shapes;
mod sounds;
mod support;
mod symbols;
mod symbols_reverse;

use support::*;
