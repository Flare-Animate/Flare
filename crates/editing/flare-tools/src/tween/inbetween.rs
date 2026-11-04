//! Keyframe interpolation and in-betweens: bake the frames between two keyframes from the curve.

use std::num::NonZeroU32;

use flare_commands::Command;
use flare_core::SceneId;
use flare_document::{Document, FrameSpan, SpanInterp, Tween};
use flare_eval::{inbetween_content, span_fraction};
use thiserror::Error;

use crate::common::*;

/// The most in-betweens one request makes, so a typo cannot make a million spans.
pub const MAX_INBETWEENS: u32 = 1000;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum InbetweenError {
    #[error("no keyframe starts at frame {0}")]
    NoSuchKeyframe(u32),
    #[error("in-betweens need between 1 and {MAX_INBETWEENS}, not {0}")]
    Count(u32),
    #[error("the keyframe at frame {0} has no keyframe right after it to blend toward")]
    NoNext(u32),
    #[error("the keyframe lasts {have} frames, which holds at most {} in-betweens, not {want}", have - 1)]
    TooShort { have: u32, want: u32 },
    #[error(
        "the two keyframes hold content that cannot be blended (a shape needs a shape, an instance the same symbol, a camera a camera)"
    )]
    Incompatible,
}

/// Sets the interpolation of the tween starting at a keyframe, or clears it (`None`).
/// `None` back when the span is missing or already has it.
pub fn set_interpolation(
    document: &Document,
    scene: SceneId,
    at: SpanRef,
    interp: Option<SpanInterp>,
) -> Option<Vec<Command>> {
    let span = find_span(document, scene, at)?;
    if span.interp() == interp {
        return None;
    }
    Some(vec![Command::SetKeyframeInterpolation {
        scene,
        layer: at.layer,
        start: at.start,
        interp,
    }])
}

/// `count` new keyframes inside the keyframe at `at`, blended toward the keyframe right after it
/// along the span's own curve (its interpolation, else its ease). The span's `duration` frames are
/// shared out as evenly as they go, the original keeping the first part. The result is one batch,
/// so one undo step, and nothing changes if any piece cannot be made.
///
/// # Errors
/// See `InbetweenError`.
pub fn create_inbetweens(
    document: &Document,
    scene: SceneId,
    at: SpanRef,
    count: u32,
) -> Result<Vec<Command>, InbetweenError> {
    if count == 0 || count > MAX_INBETWEENS {
        return Err(InbetweenError::Count(count));
    }
    let span = find_span(document, scene, at).ok_or(InbetweenError::NoSuchKeyframe(at.start))?;
    let layer = scene_timeline(document, scene)
        .and_then(|t| t.layers().iter().find(|l| l.id() == at.layer))
        .ok_or(InbetweenError::NoSuchKeyframe(at.start))?;
    let next = layer
        .span_at_start(span.end())
        .ok_or(InbetweenError::NoNext(at.start))?;
    let total = span.duration().get();
    if count >= total {
        return Err(InbetweenError::TooShort {
            have: total,
            want: count,
        });
    }
    let parts = count + 1;
    let (base, extra) = (total / parts, total % parts);
    let length = |i: u32| base + u32::from(i < extra);
    let mut commands = vec![Command::RemoveSpan {
        scene,
        layer: at.layer,
        start: at.start,
    }];
    let first = length(0);
    let rebuilt = FrameSpan::new(
        span.start(),
        NonZeroU32::new(first).ok_or(InbetweenError::Incompatible)?,
        span.content().clone(),
    )
    .with_extras(span.extras().to_vec())
    .map_err(|_| InbetweenError::Incompatible)?;
    commands.push(Command::InsertSpan {
        scene,
        layer: at.layer,
        span: rebuilt,
    });
    let mut offset = first;
    for i in 1..parts {
        let t = span_fraction(span, f64::from(offset) / f64::from(total));
        let content = inbetween_content(span.content(), next.content(), t)
            .ok_or(InbetweenError::Incompatible)?;
        let len = NonZeroU32::new(length(i)).ok_or(InbetweenError::Incompatible)?;
        commands.push(Command::InsertSpan {
            scene,
            layer: at.layer,
            span: FrameSpan::new(span.start() + offset, len, content).with_tween(Tween::None),
        });
        offset += len.get();
    }
    Ok(commands)
}

#[cfg(test)]
mod tests;
