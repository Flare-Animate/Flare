//! Where an armature's joints are at a frame, and where that puts the instances they carry.

use std::borrow::Cow;

use flare_document::{Armature, Content, FrameSpan, Layer, Tween};

use super::morph;

/// The pose an armature layer's keyframe holds at `frame`: its own, or between it and the next
/// keyframe when a motion tween runs from one to the next and both are one rig.
pub fn armature_at<'a>(
    layer: &'a Layer,
    span: &'a FrameSpan,
    frame: u32,
) -> Option<Cow<'a, Armature>> {
    let Content::Armature(armature) = span.content() else {
        return None;
    };
    if span.tween() == Tween::Linear
        && frame > span.start()
        && let Some(Content::Armature(next)) = layer.span_at_start(span.end()).map(|n| n.content())
    {
        let t = morph::eased(
            f64::from(frame - span.start()) / f64::from(span.duration().get()),
            span.ease(),
        );
        if let Some(between) = armature.lerp(next, t) {
            return Some(Cow::Owned(between));
        }
    }
    Some(Cow::Borrowed(armature))
}

/// The frame the run of poses `span` is part of began: a new pose is the same instances moved,
/// so what they show goes on from where it was instead of starting its symbols again.
pub(crate) fn run_start(layer: &Layer, span: &FrameSpan) -> u32 {
    let Content::Armature(rig) = span.content() else {
        return span.start();
    };
    let spans = layer.spans();
    let mut at = spans.partition_point(|s| s.start() < span.start());
    // Spans are sorted and do not overlap, so this walks back over the keyframes that run into
    // each other, at most once over the layer.
    while let Some(earlier) = at.checked_sub(1).map(|i| &spans[i]) {
        let joined = earlier.end() == u64::from(spans[at].start());
        match earlier.content() {
            Content::Armature(other) if joined && other.same_rig(rig) => at -= 1,
            _ => break,
        }
    }
    spans.get(at).map_or(span.start(), FrameSpan::start)
}
