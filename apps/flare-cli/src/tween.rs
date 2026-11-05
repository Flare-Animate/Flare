//! `flare tween`: the interpolation library from the command line. The behaviour is in
//! `flare_tools::tween`; this only parses arguments and prints.

use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use clap::Subcommand;
use flare_tools::ops::{self, SpanRef};
use flare_tools::tween::spec;
use serde_json::{Value, json};
use uuid::Uuid;

#[derive(Subcommand)]
pub enum TweenCommand {
    /// List the interpolation families, easings and handle types.
    ListEasings,
    /// Evaluate a curve: the fraction of the change made at `--t` (0 to 1) of the time.
    Eval {
        #[arg(long)]
        family: String,
        #[arg(long)]
        easing: Option<String>,
        #[arg(long)]
        handle: Option<String>,
        /// Bezier handles: right.dt right.dv left.dt left.dv, in the unit square.
        #[arg(long, num_args = 4, allow_hyphen_values = true)]
        handles: Option<Vec<f64>>,
        #[arg(long, allow_hyphen_values = true)]
        t: f64,
    },
    /// Set the interpolation of the tween that starts at the keyframe at `--at`; `--clear` goes
    /// back to its plain ease.
    SetInterpolation {
        path: PathBuf,
        #[arg(long)]
        at: u32,
        #[arg(long)]
        layer: Option<Uuid>,
        #[arg(long, required_unless_present = "clear")]
        family: Option<String>,
        #[arg(long)]
        easing: Option<String>,
        #[arg(long)]
        handle: Option<String>,
        #[arg(long, num_args = 4, allow_hyphen_values = true)]
        handles: Option<Vec<f64>>,
        #[arg(long, conflicts_with = "family")]
        clear: bool,
    },
    /// Bake `--count` in-between keyframes inside the keyframe at `--at`, blended toward the next.
    Inbetween {
        path: PathBuf,
        #[arg(long)]
        at: u32,
        #[arg(long)]
        count: u32,
        #[arg(long)]
        layer: Option<Uuid>,
    },
}

fn four(handles: Option<Vec<f64>>) -> Option<[f64; 4]> {
    handles.and_then(|h| <[f64; 4]>::try_from(h).ok())
}

pub fn run(op: TweenCommand) -> Result<Value> {
    match op {
        TweenCommand::ListEasings => Ok(json!({
            "families": spec::families().iter()
                .map(|f| json!({ "name": f.name, "eased": f.eased })).collect::<Vec<_>>(),
            "easings": spec::easing_names(),
            "handle_types": spec::handle_names(),
        })),
        TweenCommand::Eval {
            family,
            easing,
            handle,
            handles,
            t,
        } => {
            let value = spec::eval(
                &family,
                easing.as_deref(),
                handle.as_deref(),
                four(handles),
                t,
            )
            .map_err(|e| anyhow!(e))?;
            Ok(json!({ "t": t, "value": value }))
        }
        TweenCommand::SetInterpolation {
            path,
            at,
            layer,
            family,
            easing,
            handle,
            handles,
            clear,
        } => {
            let interp = match (&family, clear) {
                (Some(family), false) => Some(
                    spec::build_interp(family, easing.as_deref(), handle.as_deref(), four(handles))
                        .map_err(|e| anyhow!(e))?,
                ),
                _ => None,
            };
            let mut session = crate::project::open_session(&path)?;
            let (scene, layer) = crate::project::first_scene_layer(&session, layer)?;
            let span = SpanRef {
                layer,
                start: at,
                element: 0,
            };
            let commands = ops::set_interpolation(session.document(), scene, span, interp)
                .with_context(|| {
                    format!("no keyframe at frame {at} or it already has that interpolation")
                })?;
            session.apply(commands)?;
            crate::project::commit_session(&session, &path)?;
            Ok(json!({ "frame": at, "revision": session.revision() }))
        }
        TweenCommand::Inbetween {
            path,
            at,
            count,
            layer,
        } => {
            let mut session = crate::project::open_session(&path)?;
            let (scene, layer) = crate::project::first_scene_layer(&session, layer)?;
            let span = SpanRef {
                layer,
                start: at,
                element: 0,
            };
            let commands = ops::create_inbetweens(session.document(), scene, span, count)?;
            session.apply(commands)?;
            crate::project::commit_session(&session, &path)?;
            Ok(json!({ "frame": at, "inbetweens": count, "revision": session.revision() }))
        }
    }
}
