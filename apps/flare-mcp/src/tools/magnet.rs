//! The Magnet: bend the lines and shapes near a point, as a drag with the desktop app's Magnet.

use flare_core::LayerId;
use flare_document::Document;
use flare_tools::magnet::{self, DEFAULT_MAGNET_RADIUS, Pull};
use flare_tools::ops::SpanRef;
use glamx::DVec2;
use rmcp::{handler::server::wrapper::Parameters, tool, tool_router};
use serde_json::{Value, json};

use crate::ops::{apply, first_scene, parse_id};
use crate::params_magnet::MagnetPull;
use crate::server::{FlareServer, Outcome, State};

#[tool_router(router = magnet_tools, vis = "pub(super)")]
impl FlareServer {
    #[tool(
        description = "Magnet: bend line art by pulling it, as a drag with the desktop app's Magnet does, as one undo step. Every point of the lines and shapes within `radius` of `from` moves toward `to`: the point at `from` all the way, the rest less the farther they are, those at the edge of the circle not at all; a line with too few points to bend smoothly gets more first. `layer` keeps it to one layer; otherwise every unlocked layer at `frame`. A pull that reaches nothing is refused. `dry_run` only lists what it reaches and needs no write access."
    )]
    fn magnet(&self, Parameters(args): Parameters<MagnetPull>) -> Outcome {
        if args.dry_run.unwrap_or(false) {
            return self.read(|state| {
                let (_, targets, pull) = reach(state, &args)?;
                Ok(report(&targets, &pull, false))
            });
        }
        self.write(args.expected_revision, |state| {
            let (scene, targets, pull) = reach(state, &args)?;
            let commands = magnet::pull(state.session.document(), scene, &targets, &pull)
                .map_err(|e| format!("Magnet: {e}"))?;
            let revision = apply(state, commands)?;
            let mut out = report(&targets, &pull, true);
            out["revision"] = revision["revision"].clone();
            Ok(out)
        })
    }
}

/// The pull `args` asks for and the shapes it reaches.
fn reach(
    state: &State,
    args: &MagnetPull,
) -> Result<(flare_core::SceneId, Vec<SpanRef>, Pull), String> {
    let scene = first_scene(state)?;
    let pull = Pull::new(
        DVec2::from(args.from),
        DVec2::from(args.to),
        args.radius.unwrap_or(DEFAULT_MAGNET_RADIUS),
    )
    .map_err(|e| format!("Magnet: {e}"))?;
    let layer = args
        .layer
        .as_deref()
        .map(|id| parse_id(id, "layer").map(LayerId::from_uuid))
        .transpose()?;
    let document: &Document = state.session.document();
    let frame = args.frame.unwrap_or(0);
    Ok((
        scene,
        magnet::in_reach(document, scene, frame, layer, &pull),
        pull,
    ))
}

fn report(targets: &[SpanRef], pull: &Pull, pulled: bool) -> Value {
    let reached: Vec<Value> = targets
        .iter()
        .map(|at| json!({ "layer": at.layer.as_uuid(), "start": at.start, "element": at.element }))
        .collect();
    json!({ "reached": reached, "radius": pull.radius, "pulled": pulled })
}
