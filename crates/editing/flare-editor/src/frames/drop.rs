//! Dropping a dragged keyframe anywhere on the timeline: onto empty frames, or over frames that
//! hold something, which it replaces, as a keyframe dragged in Animate's timeline does.

use super::*;

/// Moves the span of `from` starting at `start` so it starts at `to` on `onto`, the same layer or
/// another, replacing the frames it lands on. A span it covers goes; one it cuts keeps the parts
/// outside it, the far part with fresh identities. The frames it leaves are empty, and it keeps
/// its own content and tween. One batch, so the drop is one undo step.
///
/// `None` when `from` has no span starting at `start`, the span would not move, `onto` is a
/// folder, or the span would run past the largest frame number.
pub fn drop_span(
    scene: SceneId,
    from: &Layer,
    start: u32,
    onto: &Layer,
    to: u32,
) -> Option<Vec<Command>> {
    let same = from.id() == onto.id();
    if (same && start == to) || onto.is_folder() {
        return None;
    }
    let moved = from.spans().iter().find(|s| s.start() == start)?;
    let stop = u64::from(to) + u64::from(moved.duration().get());
    if stop > u64::from(u32::MAX) {
        return None;
    }
    let others: Vec<FrameSpan> = onto
        .spans()
        .iter()
        .filter(|s| !same || s.start() != start)
        .cloned()
        .collect();
    let mut spans = cleared(&others, to, stop)?;
    spans.push(shifted(moved, to)?);
    spans.sort_by_key(FrameSpan::start);
    let mut commands = Vec::with_capacity(spans.len() * 2 + 1);
    if !same {
        commands.push(Command::RemoveSpan {
            scene,
            layer: from.id(),
            start,
        });
    }
    commands.extend(replace_spans(scene, onto, spans));
    Some(commands)
}
