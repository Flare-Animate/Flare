//! Bend Edge: reshape one edge of merged drawing, as a drag on the edge with the desktop app's
//! Selection tool.

use flare_core::LayerId;
use flare_tools::reshape::{self, DEFAULT_TOLERANCE, EdgeGrab};
use glamx::DVec2;
use rmcp::{handler::server::wrapper::Parameters, tool, tool_router};
use serde_json::{Value, json};

use crate::ops::{apply, first_scene, parse_id};
use crate::params_bend_edge::BendEdge;
use crate::server::{FlareServer, Outcome, State};

#[tool_router(router = bend_edge_tools, vis = "pub(super)")]
impl FlareServer {
    #[tool(
        description = "Bend Edge: reshape a drawn line by dragging its edge, as the desktop app's Selection tool does with merged drawing, as one undo step. Takes hold of the edge of merged drawing (a line, or the outline of a fill) nearest `from` within `tolerance` and drags that point to `to`: the edge becomes a curve through it and its two ends stay put. Drawing objects are not reshaped this way. `layer` keeps it to one layer; otherwise every unlocked layer at `frame`. No edge near `from` is refused. `dry_run` only says which edge would be taken and needs no write access."
    )]
    fn bend_edge(&self, Parameters(args): Parameters<BendEdge>) -> Outcome {
        if args.dry_run.unwrap_or(false) {
            return self.read(|state| {
                let scene = first_scene(state)?;
                let tolerance = reshape::clamp_tolerance(args.tolerance.unwrap_or(DEFAULT_TOLERANCE));
                let layer = args
                    .layer
                    .as_deref()
                    .map(|id| parse_id(id, "layer").map(LayerId::from_uuid))
                    .transpose()?;
                let from = DVec2::from(args.from);
                let grab = reshape::edge_at(
                    state.session.document(),
                    scene,
                    args.frame.unwrap_or(0),
                    layer,
                    from,
                    tolerance,
                )
                .ok_or_else(|| {
                    format!(
                        "Bend edge: no merged line or shape has an edge within {tolerance} of ({}, {})",
                        from.x, from.y
                    )
                })?;
                Ok(report(&grab, false))
            });
        }
        self.write(args.expected_revision, |state| {
            let (_, grab, commands) = take(state, &args)?;
            let revision = apply(state, commands)?;
            let mut out = report(&grab, true);
            out["revision"] = revision["revision"].clone();
            Ok(out)
        })
    }
}

/// The edge `args` takes hold of and the commands that pull it.
fn take(
    state: &State,
    args: &BendEdge,
) -> Result<(flare_core::SceneId, EdgeGrab, Vec<flare_commands::Command>), String> {
    let scene = first_scene(state)?;
    let layer = args
        .layer
        .as_deref()
        .map(|id| parse_id(id, "layer").map(LayerId::from_uuid))
        .transpose()?;
    let (grab, commands) = reshape::bend_edge_near(
        state.session.document(),
        scene,
        args.frame.unwrap_or(0),
        layer,
        DVec2::from(args.from),
        DVec2::from(args.to),
        args.tolerance.unwrap_or(DEFAULT_TOLERANCE),
    )
    .map_err(|e| format!("Bend edge: {e}"))?;
    Ok((scene, grab, commands))
}

fn report(grab: &EdgeGrab, bent: bool) -> Value {
    json!({
        "edge": {
            "layer": grab.at.layer.as_uuid(),
            "start": grab.at.start,
            "element": grab.at.element,
            "segment": grab.segment,
            "t": grab.t,
        },
        "bent": bent,
    })
}
