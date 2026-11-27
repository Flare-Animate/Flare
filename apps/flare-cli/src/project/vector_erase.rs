//! The vector erasers from the command line: take the stretches of lines a round tip touches out
//! to where other lines cross them, the shapes it touches whole, or those wholly inside a box or
//! loop, at one frame, as a drag with the desktop app's Eraser does, in one undo step.

use anyhow::{Context, Result, anyhow};
use flare_core::LayerId;
use flare_tools::eraser::vector::{self, VectorEraseError, VectorEraserType};
use glamx::DVec2;
use serde_json::{Value, json};
use std::path::Path;

use super::*;

/// What `flare vector-erase` was asked.
pub struct VectorEraseRequest {
    pub kind: VectorEraserType,
    pub points: Vec<(f64, f64)>,
    /// How wide the round tip is, for Segment and Whole Line.
    pub size: f64,
    pub frame: u32,
    /// Only the shapes on this layer; every shown, unlocked layer without it.
    pub layer: Option<uuid::Uuid>,
}

/// Takes away what `request` covers and writes the project back. Taking nothing is an error,
/// and the file is left as it was.
pub fn vector_erase(path: &Path, request: &VectorEraseRequest) -> Result<Value> {
    let mut session = open(path)?;
    let scene = session
        .document()
        .scenes()
        .first()
        .context("document has no scene")?
        .id();
    let points: Vec<DVec2> = request
        .points
        .iter()
        .map(|&(x, y)| DVec2::new(x, y))
        .collect();
    let refuse = |e: VectorEraseError| anyhow!("Vector Eraser: {e}");
    let area = vector::area(request.kind, &points, request.size).map_err(refuse)?;
    let commands = vector::erase_with(
        session.document(),
        scene,
        request.frame,
        &[area],
        request.kind.take(),
        request.layer.map(LayerId::from_uuid),
    );
    if commands.is_empty() {
        return Err(refuse(VectorEraseError::NothingTaken));
    }
    session.apply(commands)?;
    commit(&session, path)?;
    Ok(json!({ "type": request.kind.name(), "frame": request.frame, "erased": true }))
}
