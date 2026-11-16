//! `flare lip-sync`: keys a mouth Graphic to a voice. The work is `flare_runtime::lip_sync`'s; this
//! reads the arguments, finds Rhubarb, writes the project and prints what was made.

use std::path::PathBuf;

use anyhow::{Result, anyhow};
use clap::Args;
use flare_runtime::{Analysis, LipSyncRequest, lip_sync, locate_rhubarb, pick_sound};
use flare_tools::lipsync::MouthMap;
use flare_tools::ops::SpanRef;
use serde_json::{Value, json};
use uuid::Uuid;

/// Key a mouth to a voice: the keyframe at --frame, one instance of a Graphic holding a mouth
/// drawing on each frame, becomes Single Frame keys of those drawings following the sound
/// (Rhubarb Lip Sync when it is installed, else the built-in analysis). One undoable edit.
#[derive(Args)]
pub struct LipSyncArgs {
    path: PathBuf,
    /// The frame the mouth keyframe starts on (zero based): it must hold one instance of a Graphic
    /// symbol with a mouth drawing on each of its frames.
    #[arg(long)]
    frame: u32,
    /// The layer of the mouth keyframe; the first layer that can be drawn on by default.
    #[arg(long)]
    layer: Option<Uuid>,
    /// The sound to follow, by name or id; the one sound on the timeline by default.
    #[arg(long)]
    sound: Option<String>,
    /// The symbol's frame (zero based) for each mouth shape, as `mbp=3,o=5`. Shapes not named
    /// keep their place in the order neutral, mbp, ai, e, o, u, fv, l, wq, etc (frames 0 to 9).
    #[arg(long, default_value = "")]
    map: String,
    /// auto (Rhubarb Lip Sync when it is installed, else the built-in analysis), built-in or
    /// rhubarb.
    #[arg(long, default_value = "auto", value_parser = analysis_arg)]
    analysis: Analysis,
}

fn analysis_arg(text: &str) -> Result<Analysis, String> {
    Analysis::parse(text).ok_or_else(|| format!("`{text}` is not auto, built-in or rhubarb"))
}

pub fn run(args: LipSyncArgs) -> Result<Value> {
    let map = MouthMap::parse(&args.map)?;
    let mut session = crate::project::open_session(&args.path)?;
    let (scene, layer) = crate::project::first_scene_layer(&session, args.layer)?;
    let sound = pick_sound(session.document(), scene, args.sound.as_deref())?;
    let request = LipSyncRequest {
        scene,
        at: SpanRef {
            layer,
            start: args.frame,
            element: 0,
        },
        sound,
        map,
        analysis: args.analysis,
    };
    let rhubarb = locate_rhubarb();
    let made = lip_sync(session.document(), &request, rhubarb.as_deref())
        .map_err(|error| anyhow!("cannot lip sync: {error}"))?;
    let shapes: Vec<&str> = made.shapes.iter().map(|s| s.name()).collect();
    let keyframes = made.keyframes;
    session.apply(made.commands)?;
    crate::project::commit_session(&session, &args.path)?;
    Ok(json!({
        "sound": sound.as_uuid(),
        "layer": layer.as_uuid(),
        "start": made.start,
        "frames": made.frames,
        "keyframes": keyframes,
        "analysis": made.used.name(),
        "fell_back": made.fell_back,
        "shapes": shapes,
        "map": map
            .pairs()
            .map(|(v, f)| (v.name().to_owned(), json!(f)))
            .collect::<serde_json::Map<_, _>>(),
    }))
}
