//! The Magnet from the command line: pull the lines and shapes near a stage point to another, as
//! a drag with the desktop app's Magnet does, in one undo step.

use anyhow::{Context, Result};
use flare_core::LayerId;
use flare_tools::magnet::{self, Pull};
use glamx::DVec2;
use serde_json::{Value, json};
use std::path::Path;

use super::*;

/// What `flare magnet` was asked.
pub struct MagnetRequest {
    pub from: (f64, f64),
    pub to: (f64, f64),
    pub radius: f64,
    pub frame: u32,
    /// Only the shapes on this layer; every unlocked layer without it.
    pub layer: Option<uuid::Uuid>,
    /// Say which shapes the pull reaches and change nothing.
    pub dry_run: bool,
}

/// Pulls what `request` reaches and writes the project back.
pub fn magnet(path: &Path, request: &MagnetRequest) -> Result<Value> {
    let mut session = open(path)?;
    let scene = session
        .document()
        .scenes()
        .first()
        .context("document has no scene")?
        .id();
    let pull = Pull::new(
        DVec2::new(request.from.0, request.from.1),
        DVec2::new(request.to.0, request.to.1),
        request.radius,
    )
    .map_err(|e| anyhow::anyhow!("Magnet: {e}"))?;
    let layer = request.layer.map(LayerId::from_uuid);
    let targets = magnet::in_reach(session.document(), scene, request.frame, layer, &pull);
    let reached: Vec<Value> = targets
        .iter()
        .map(|at| json!({ "layer": at.layer.as_uuid(), "start": at.start, "element": at.element }))
        .collect();
    if request.dry_run {
        return Ok(json!({ "reached": reached, "radius": pull.radius, "pulled": false }));
    }
    let commands = magnet::pull(session.document(), scene, &targets, &pull)
        .map_err(|e| anyhow::anyhow!("Magnet: {e}"))?;
    session.apply(commands)?;
    commit(&session, path)?;
    Ok(json!({ "reached": reached, "radius": pull.radius, "pulled": true }))
}
