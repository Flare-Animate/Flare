//! The Eraser changes the frame it is on and no other. On a frame a keyframe holds from before,
//! that frame becomes a keyframe of its own first, so the keyframe and the rest of its frames keep
//! their drawing as it was.

use std::num::NonZeroU32;

use flare_commands::Command;
use flare_core::{LayerId, SceneId};
use flare_document::{Content, FrameSpan};
use glamx::DAffine2;

use crate::common::{SpanRef, fresh_content, replace_span};

/// The commands that show `kept` in place of what `span` held, on `frame` alone when the span
/// only holds that frame from an earlier keyframe, and on the whole keyframe when `frame` is its
/// first. Empty when a span could not be built.
pub(super) fn keep_on_frame(
    scene: SceneId,
    layer: LayerId,
    span: &FrameSpan,
    frame: u32,
    kept: Vec<Content>,
) -> Vec<Command> {
    let at = SpanRef {
        layer,
        start: span.start(),
        element: 0,
    };
    if frame <= span.start() || u64::from(frame) >= span.end() {
        return match span.with_elements(kept) {
            Ok(Some(rest)) => replace_span(scene, at, rest),
            Ok(None) => vec![Command::RemoveSpan {
                scene,
                layer,
                start: span.start(),
            }],
            Err(_) => Vec::new(),
        };
    }
    split_out(scene, layer, span, frame, kept).unwrap_or_default()
}

/// `span` cut into the frames before `frame`, `frame` itself showing `kept`, and the frames after
/// it. The frame and the frames after it are copies, every element with an identity of its own.
fn split_out(
    scene: SceneId,
    layer: LayerId,
    span: &FrameSpan,
    frame: u32,
    kept: Vec<Content>,
) -> Option<Vec<Command>> {
    let before = span.retimed(span.start(), NonZeroU32::new(frame - span.start())?);
    let one = span.retimed(frame, NonZeroU32::MIN);
    let renewed = kept.iter().map(renew).collect::<Option<Vec<_>>>()?;
    // Everything rubbed out leaves the frame blank, still a keyframe of its own.
    let here = match one.with_elements(renewed).ok()? {
        Some(here) => here,
        None => FrameSpan::new(frame, NonZeroU32::MIN, Content::Empty),
    };
    let mut commands = vec![
        Command::RemoveSpan {
            scene,
            layer,
            start: span.start(),
        },
        Command::InsertSpan {
            scene,
            layer,
            span: before,
        },
        Command::InsertSpan {
            scene,
            layer,
            span: here,
        },
    ];
    let next = frame.checked_add(1)?;
    if let Some(rest) = u32::try_from(span.end() - u64::from(next))
        .ok()
        .and_then(NonZeroU32::new)
    {
        let copies = span.elements().map(renew).collect::<Option<Vec<_>>>()?;
        let after = span.retimed(next, rest).with_elements(copies).ok()??;
        commands.push(Command::InsertSpan {
            scene,
            layer,
            span: after,
        });
    }
    Some(commands)
}

/// `content` with a new identity, so a copy of a frame can sit beside the original. What has no
/// identity of its own (nothing, a camera, the pose of a rig shared by every keyframe) is kept.
fn renew(content: &Content) -> Option<Content> {
    match content {
        Content::Empty | Content::Camera(_) | Content::Armature(_) => Some(content.clone()),
        _ => fresh_content(content, DAffine2::IDENTITY),
    }
}
