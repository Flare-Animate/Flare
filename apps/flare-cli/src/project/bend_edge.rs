//! Reshaping an edge from the command line: take hold of the merged line or shape edge nearest a
//! stage point and pull it to another, as a drag on the edge with the Selection tool does, in one
//! undo step.

use anyhow::{Context, Result};
use flare_core::LayerId;
use flare_tools::reshape;
use glamx::DVec2;
use serde_json::{Value, json};
use std::path::Path;

use super::*;

/// What `flare bend-edge` was asked.
pub struct BendEdgeRequest {
    pub from: (f64, f64),
    pub to: (f64, f64),
    pub tolerance: f64,
    pub frame: u32,
    pub layer: Option<uuid::Uuid>,
    /// Say which edge would be taken and change nothing.
    pub dry_run: bool,
}

/// Bends the edge `request` names and writes the project back.
pub fn bend_edge(path: &Path, request: &BendEdgeRequest) -> Result<Value> {
    let mut session = open(path)?;
    let scene = session
        .document()
        .scenes()
        .first()
        .context("document has no scene")?
        .id();
    let from = DVec2::new(request.from.0, request.from.1);
    let to = DVec2::new(request.to.0, request.to.1);
    let layer = request.layer.map(LayerId::from_uuid);
    if request.dry_run {
        let tolerance = reshape::clamp_tolerance(request.tolerance);
        let grab = reshape::edge_at(
            session.document(),
            scene,
            request.frame,
            layer,
            from,
            tolerance,
        )
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Bend edge: no merged line or shape has an edge within {tolerance} of ({}, {})",
                from.x,
                from.y
            )
        })?;
        return Ok(json!({ "edge": edge_json(&grab), "bent": false }));
    }
    let (grab, commands) = reshape::bend_edge_near(
        session.document(),
        scene,
        request.frame,
        layer,
        from,
        to,
        request.tolerance,
    )
    .map_err(|e| anyhow::anyhow!("Bend edge: {e}"))?;
    session.apply(commands)?;
    commit(&session, path)?;
    Ok(json!({ "edge": edge_json(&grab), "bent": true }))
}

fn edge_json(grab: &reshape::EdgeGrab) -> Value {
    json!({
        "layer": grab.at.layer.as_uuid(),
        "start": grab.at.start,
        "element": grab.at.element,
        "segment": grab.segment,
        "t": grab.t,
    })
}
