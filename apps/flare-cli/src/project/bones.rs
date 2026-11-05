//! Bones: linking instances into an armature and posing it, as Animate's Bone tool does.

use anyhow::{Context, Result, bail};
use flare_core::{DocumentPoint, InstanceId, LayerId, SceneId};
use flare_document::Document;
use flare_tools::bones::{self, Bond};
use glamx::DVec2;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use uuid::Uuid;

use super::*;

/// What to do with bones.
#[derive(Debug, clap::Subcommand)]
pub enum BoneAction {
    /// Link two instances with a bone, as a drag of the Bone tool from one to the other does. Two
    /// instances on plain layers make a new armature layer; from an instance in an armature, the
    /// other joins it below that joint. Instance ids are in `flare eval`.
    Add {
        path: PathBuf,
        #[arg(long)]
        from: Uuid,
        #[arg(long)]
        to: Uuid,
        #[arg(long, default_value_t = 0)]
        frame: u32,
        /// Where on `from` the joint goes (default: its registration point).
        #[arg(long, num_args = 2, value_names = ["X", "Y"], allow_hyphen_values = true)]
        from_at: Option<Vec<f64>>,
        /// Where on `to` the joint goes (default: its registration point).
        #[arg(long, num_args = 2, value_names = ["X", "Y"], allow_hyphen_values = true)]
        to_at: Option<Vec<f64>>,
    },
    /// Pose the armature at a frame: pull a joint to a point (inverse kinematics; a root moves
    /// the whole armature), turn it, or move a root by an offset. Between two poses this makes a
    /// pose of its own, which the one before tweens into.
    Pose {
        path: PathBuf,
        /// The armature layer (default: the scene's only one).
        #[arg(long)]
        layer: Option<Uuid>,
        #[arg(long)]
        joint: usize,
        #[arg(long, default_value_t = 0)]
        frame: u32,
        /// Pull the joint toward this point.
        #[arg(long, num_args = 2, value_names = ["X", "Y"], allow_hyphen_values = true,
              conflicts_with_all = ["turn", "by"])]
        to: Option<Vec<f64>>,
        /// Turn the joint to this many degrees from rest, clockwise.
        #[arg(long, allow_hyphen_values = true, conflicts_with = "by")]
        turn: Option<f64>,
        /// Move a root joint, and the armature with it, by this much.
        #[arg(long, num_args = 2, value_names = ["DX", "DY"], allow_hyphen_values = true)]
        by: Option<Vec<f64>>,
    },
    /// Let a joint turn only between two angles from rest, in degrees, in every pose; with
    /// neither it turns freely.
    Limit {
        path: PathBuf,
        #[arg(long)]
        layer: Option<Uuid>,
        #[arg(long)]
        joint: usize,
        #[arg(long, allow_hyphen_values = true, requires = "max")]
        min: Option<f64>,
        #[arg(long, allow_hyphen_values = true, requires = "min")]
        max: Option<f64>,
    },
    /// Take away the bone into a joint, and every bone below it; the instances stay where they are.
    Detach {
        path: PathBuf,
        #[arg(long)]
        layer: Option<Uuid>,
        #[arg(long)]
        joint: usize,
    },
    /// Turn the armature back into plain keyframes of its instances, posed as they are.
    Release {
        path: PathBuf,
        #[arg(long)]
        layer: Option<Uuid>,
    },
}

fn first_scene(document: &Document) -> Result<SceneId> {
    Ok(document
        .scenes()
        .first()
        .context("the document has no scene")?
        .id())
}

fn is_armature(layer: &flare_document::Layer) -> bool {
    bones::is_armature_layer(layer)
}

/// The armature layer named, or the first scene's only one.
fn armature_layer(document: &Document, layer: Option<Uuid>) -> Result<LayerId> {
    let layers = document.scenes()[0].timeline().layers();
    match layer {
        Some(id) => layers
            .iter()
            .find(|l| *l.id().as_uuid() == id)
            .filter(|l| is_armature(l))
            .map(|l| l.id())
            .with_context(|| format!("bones: no armature layer {id} in the first scene")),
        None => {
            let mut found = layers.iter().filter(|l| is_armature(l));
            let first = found
                .next()
                .context("bones: the first scene has no armature; make one with `bone add`")?;
            if found.next().is_some() {
                bail!("bones: the first scene has several armatures; name one with --layer");
            }
            Ok(first.id())
        }
    }
}

fn point(values: Option<Vec<f64>>, what: &str) -> Result<Option<DVec2>> {
    match values.as_deref() {
        None => Ok(None),
        Some([x, y]) if x.is_finite() && y.is_finite() => Ok(Some(DVec2::new(*x, *y))),
        Some(_) => bail!("bones: {what} must be two finite numbers"),
    }
}

/// Every armature of the first scene at `frame`, for printing.
pub fn bones_report(document: &Document, frame: u32) -> Result<Value> {
    let scene = first_scene(document)?;
    Ok(json!({ "frame": frame, "armatures": bones::report(document, scene, frame) }))
}

/// `flare bones`: the armatures at a frame.
pub fn bones_list(path: &Path, frame: u32) -> Result<Value> {
    let session = open(path)?;
    bones_report(session.document(), frame)
}

/// `flare bone ...`.
pub fn bone(action: BoneAction) -> Result<Value> {
    let (path, frame) = match &action {
        BoneAction::Add { path, frame, .. } | BoneAction::Pose { path, frame, .. } => {
            (path.clone(), *frame)
        }
        BoneAction::Limit { path, .. }
        | BoneAction::Detach { path, .. }
        | BoneAction::Release { path, .. } => (path.clone(), 0),
    };
    let mut session = open(&path)?;
    let document = session.document();
    let scene = first_scene(document)?;
    let commands = match action {
        BoneAction::Add {
            from,
            to,
            frame,
            from_at,
            to_at,
            ..
        } => {
            let at = |p: Option<DVec2>| p.and_then(|p| DocumentPoint::new(p.x, p.y).ok());
            let from = Bond {
                instance: InstanceId::from_uuid(from),
                at: at(point(from_at, "--from-at")?),
            };
            let to = Bond {
                instance: InstanceId::from_uuid(to),
                at: at(point(to_at, "--to-at")?),
            };
            let name = format!("Armature_{}", armature_count(document) + 1);
            bones::connect(document, scene, frame, from, to, &name)
                .context("bones: cannot add the bone")?
        }
        BoneAction::Pose {
            layer,
            joint,
            frame,
            to,
            turn,
            by,
            ..
        } => {
            let layer = armature_layer(document, layer)?;
            let made = match (point(to, "--to")?, turn, point(by, "--by")?) {
                (Some(target), None, None) => {
                    bones::pose_joint(document, scene, layer, frame, joint, target)
                }
                (None, Some(degrees), None) => {
                    if !degrees.is_finite() {
                        bail!("bones: --turn must be a finite number");
                    }
                    bones::turn_joint(document, scene, layer, frame, joint, degrees.to_radians())
                }
                (None, None, Some(by)) => {
                    bones::shift_root(document, scene, layer, frame, joint, by)
                }
                _ => bail!("bones: say one of --to, --turn or --by"),
            };
            made.context("bones: cannot pose the armature")?
        }
        BoneAction::Limit {
            layer,
            joint,
            min,
            max,
            ..
        } => {
            let layer = armature_layer(document, layer)?;
            let limit = min.zip(max).map(|(a, b)| (a.to_radians(), b.to_radians()));
            bones::set_limit(document, scene, layer, joint, limit)
                .context("bones: cannot limit the joint")?
        }
        BoneAction::Detach { layer, joint, .. } => {
            let layer = armature_layer(document, layer)?;
            bones::detach(document, scene, layer, joint).context("bones: cannot detach the bone")?
        }
        BoneAction::Release { layer, .. } => {
            let layer = armature_layer(document, layer)?;
            bones::release(document, scene, layer).context("bones: cannot release the armature")?
        }
    };
    session.apply(commands)?;
    commit(&session, &path)?;
    let mut report = bones_report(session.document(), frame)?;
    report["revision"] = json!(session.revision());
    Ok(report)
}

fn armature_count(document: &Document) -> usize {
    document.scenes()[0]
        .timeline()
        .layers()
        .iter()
        .filter(|l| is_armature(l))
        .count()
}
