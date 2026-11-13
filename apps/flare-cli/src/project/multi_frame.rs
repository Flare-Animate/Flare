//! `flare transform-frames`: Edit Multiple Frames from the command line. Everything in a range of
//! frames, on the layers that can be changed, moved, scaled and turned together about the middle
//! of all of it, as one undo step. The rules are `flare_tools::multi_frame`'s.

use super::*;
use flare_tools::multi_frame::{RangeTransform, transform_range};

pub fn transform_frames(path: &Path, from: u32, to: u32, how: RangeTransform) -> Result<Value> {
    let mut session = open(path)?;
    let (scene, _) = first_scene_layer(&session, None)?;
    let commands = transform_range(session.document(), scene, from, to, how).context(
        "nothing to transform: the frames hold nothing on a shown, unlocked layer, or the \
         transform changes nothing (or scales to zero)",
    )?;
    let count = commands.len();
    session.apply(commands)?;
    commit(&session, path)?;
    Ok(json!({ "from": from, "to": to, "commands": count }))
}
