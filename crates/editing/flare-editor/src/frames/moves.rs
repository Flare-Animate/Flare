//! Moving or copying a run of frames, several keyframes and the gaps between them, as a selected
//! range dragged in Animate's timeline goes (Alt held, it is copied).

use super::*;

/// Whether frames placed somewhere else also stay where they were.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FramePlacement {
    /// The frames left behind are empty.
    Move,
    /// The frames left behind keep what they had.
    Copy,
}

/// Places frames `first..=last` of `from` so they start at `at` on `onto`, the same layer or
/// another, replacing the frames they land on as Paste Frames does. Spans the range cuts through
/// are cut to it, and every placed span gets fresh identities. Moved, the frames left behind are
/// empty and the rest of `from` stays where it is. One batch, so it is one undo step.
///
/// `None` when the range is backwards or holds no span, a move would leave everything where it
/// is, `onto` is a folder, or the frames would run past the largest frame number.
pub fn place_frames(
    scene: SceneId,
    from: &Layer,
    (first, last): (u32, u32),
    onto: &Layer,
    at: u32,
    how: FramePlacement,
) -> Option<Vec<Command>> {
    let same = from.id() == onto.id();
    if onto.is_folder() || (same && first == at && how == FramePlacement::Move) {
        return None;
    }
    let clip = copy_frames(from, first, last)?;
    let stop = u64::from(at) + u64::from(clip.length());
    if stop > u64::from(u32::MAX) {
        return None;
    }
    let left = match how {
        FramePlacement::Move => cleared(from.spans(), first, u64::from(last) + 1)?,
        FramePlacement::Copy => from.spans().to_vec(),
    };
    let under = if same { &left[..] } else { onto.spans() };
    let mut placed = cleared(under, at, stop)?;
    for span in clip.spans() {
        placed.push(fresh(
            span,
            at.checked_add(span.start())?,
            span.duration().get(),
        )?);
    }
    placed.sort_by_key(FrameSpan::start);
    let mut commands = Vec::with_capacity((from.spans().len() + placed.len()) * 2);
    if !same && how == FramePlacement::Move {
        commands.extend(replace_spans(scene, from, left));
    }
    commands.extend(replace_spans(scene, onto, placed));
    Some(commands)
}
