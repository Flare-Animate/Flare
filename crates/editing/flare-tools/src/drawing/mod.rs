//! Drawing into the layer: where a new shape goes, as Animate keeps everything drawn on a layer in
//! a keyframe of that layer.

use std::fmt;
use std::num::NonZeroU32;

use flare_commands::Command;
use flare_core::{LayerId, SceneId};
use flare_document::{Content, Document, FrameSpan, Layer, Shape};

use glamx::DAffine2;

use crate::common::{SpanRef, fresh_content, scene_timeline};

mod merge;
pub use merge::{Merged, merge_into, merged_count};

/// Which layer a drawing goes to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// This layer.
    Layer(LayerId),
    /// The topmost layer that can take art (not a folder or the camera); with none, a new one.
    Default,
}

/// Why a drawing was not placed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DrawRefusal {
    NoSuchScene,
    NoSuchLayer,
    /// The layer, or a folder around it, is locked.
    Locked,
    Hidden,
    /// A folder, mask, guide or camera layer holds no drawings.
    NotArt,
    /// The keyframe at the frame holds a tween or a camera and cannot share itself.
    Held,
    /// More elements than a keyframe may hold, or a layer or span that cannot be built.
    TooMany,
}

impl fmt::Display for DrawRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NoSuchScene => "no such scene",
            Self::NoSuchLayer => "no such layer",
            Self::Locked => "the layer is locked",
            Self::Hidden => "the layer is hidden",
            Self::NotArt => "a folder, mask, guide or camera layer holds no drawings",
            Self::Held => {
                "the keyframe holds a tween or a camera, which cannot share it with other drawings"
            }
            Self::TooMany => "a keyframe cannot hold that many elements",
        })
    }
}

impl std::error::Error for DrawRefusal {}

/// Where the shapes went.
#[derive(Debug, Clone, PartialEq)]
pub struct DrawPlan {
    /// One batch: the layer if it had to be made, and the keyframe.
    pub commands: Vec<Command>,
    pub layer: LayerId,
    /// The new elements, bottom first.
    pub placed: Vec<SpanRef>,
}

/// The commands that draw `shapes`, in order and above what is there, into `target` at `frame`.
///
/// With a span of the layer at `frame` the shapes join it as more elements. With none, a keyframe
/// of their own starts at `frame` and lasts `duration` frames, or by default to the end of the
/// scene (one frame on a layer with nothing else) but not into the layer's next keyframe; the
/// empty frames between it and the keyframe before it are held by that keyframe, as Animate fills
/// them when a keyframe is made past them. With no layer
/// to draw on at all, a layer named `new_layer` is made first. A layer that is locked (itself or
/// by a folder), hidden, a folder, mask, guide or camera, and a keyframe that cannot hold several
/// elements, is refused.
///
/// Shapes marked as merged drawing ([`Shape::is_merged`]) are drawn by Merge Drawing's rules
/// ([`merge_into`]): they cut and join the merged drawing already in the keyframe, and `placed`
/// names the pieces they left.
pub fn draw_shapes(
    document: &Document,
    scene: SceneId,
    target: Target,
    frame: u32,
    duration: Option<NonZeroU32>,
    shapes: Vec<Shape>,
    new_layer: &str,
) -> Result<DrawPlan, DrawRefusal> {
    let merging = shapes.iter().any(Shape::is_merged);
    let contents = shapes.into_iter().map(Content::Shape).collect();
    let plan = place(
        document, scene, target, frame, duration, contents, new_layer, true,
    )?;
    if merging { merge_plan(plan) } else { Ok(plan) }
}

/// `plan` with the shapes it adds merged into the keyframe it adds them to. The keyframe is the
/// last span the plan inserts, whichever way `place` found or made it, so the rules about where a
/// drawing may go are the ones every drawing follows.
fn merge_plan(mut plan: DrawPlan) -> Result<DrawPlan, DrawRefusal> {
    let Some(Command::InsertSpan { span, .. }) = plan.commands.last_mut() else {
        return Ok(plan);
    };
    let fresh: Vec<usize> = plan.placed.iter().map(|at| at.element).collect();
    let mut kept = Vec::new();
    let mut drawn = Vec::new();
    for (index, element) in span.elements().enumerate() {
        match element {
            Content::Shape(shape) if fresh.contains(&index) => drawn.push(shape.clone()),
            other => kept.push(other.clone()),
        }
    }
    let merged = merge_into(kept, drawn);
    *span = span
        .with_elements(merged.elements)
        .ok()
        .flatten()
        .ok_or(DrawRefusal::TooMany)?;
    let start = span.start();
    plan.placed = merged
        .placed
        .into_iter()
        .map(|element| SpanRef {
            layer: plan.layer,
            start,
            element,
        })
        .collect();
    Ok(plan)
}

/// [`draw_shapes`] for anything a keyframe can hold: shapes and symbol instances, as a paste
/// brings them. The same rules decide where they go and when they are refused.
pub fn place_contents(
    document: &Document,
    scene: SceneId,
    target: Target,
    frame: u32,
    duration: Option<NonZeroU32>,
    contents: Vec<Content>,
    new_layer: &str,
) -> Result<DrawPlan, DrawRefusal> {
    place(
        document, scene, target, frame, duration, contents, new_layer, false,
    )
}

/// `own_keyframe` is for drawing: on a frame that only shows what an earlier keyframe holds, it
/// starts a keyframe there (the held drawing copied, the new elements beside it) and leaves the
/// frames before as they were. Without it the elements join the keyframe that shows there, as a
/// paste or a placed picture is meant to.
#[allow(clippy::too_many_arguments)]
fn place(
    document: &Document,
    scene: SceneId,
    target: Target,
    frame: u32,
    duration: Option<NonZeroU32>,
    contents: Vec<Content>,
    new_layer: &str,
    own_keyframe: bool,
) -> Result<DrawPlan, DrawRefusal> {
    let timeline = scene_timeline(document, scene).ok_or(DrawRefusal::NoSuchScene)?;
    let layers = timeline.layers();
    let found = match target {
        Target::Layer(id) => Some(
            layers
                .iter()
                .position(|l| l.id() == id)
                .ok_or(DrawRefusal::NoSuchLayer)?,
        ),
        Target::Default => layers.iter().position(|l| !l.is_folder() && !l.is_camera()),
    };
    let mut commands = Vec::new();
    let (layer_id, existing) = match found {
        Some(index) => {
            let layer = &layers[index];
            if layer.is_locked() || locked_by_folder(layers, index) {
                return Err(DrawRefusal::Locked);
            }
            if !layer.is_visible() {
                return Err(DrawRefusal::Hidden);
            }
            if layer.is_mask() || layer.is_guide() || layer.is_camera() || layer.is_folder() {
                return Err(DrawRefusal::NotArt);
            }
            (layer.id(), Some(layer))
        }
        None => {
            let id = LayerId::new();
            let layer = Layer::new(id, new_layer, vec![])
                .map_err(|_| DrawRefusal::TooMany)?
                .with_hold(true);
            commands.push(Command::AddLayer { scene, layer });
            (id, None)
        }
    };
    let mut contents = contents.into_iter();
    let Some(first) = contents.next() else {
        return Ok(DrawPlan {
            commands: Vec::new(),
            layer: layer_id,
            placed: Vec::new(),
        });
    };
    let span = match existing.and_then(|l| l.span_at(frame)) {
        Some(span)
            if own_keyframe
                && span.start() < frame
                && !matches!(span.content(), Content::Empty) =>
        {
            let layer = existing.ok_or(DrawRefusal::NoSuchLayer)?;
            return draw_beside_held(
                scene,
                timeline.frame_count(),
                duration,
                layer,
                span,
                frame,
                first,
                contents.collect(),
                commands,
            );
        }
        Some(span) => {
            let before = span.element_count();
            let mut grown = span.with_appended(first).ok_or(DrawRefusal::Held)?;
            for content in contents {
                grown = grown.with_appended(content).ok_or(DrawRefusal::Held)?;
            }
            // An empty keyframe takes the first shape as its content, not as an extra.
            let skipped = usize::from(matches!(span.content(), Content::Empty) && before == 1);
            commands.push(Command::RemoveSpan {
                scene,
                layer: layer_id,
                start: span.start(),
            });
            let placed = (before - skipped..grown.element_count())
                .map(|element| SpanRef {
                    layer: layer_id,
                    start: span.start(),
                    element,
                })
                .collect();
            commands.push(Command::InsertSpan {
                scene,
                layer: layer_id,
                span: grown,
            });
            return Ok(DrawPlan {
                commands,
                layer: layer_id,
                placed,
            });
        }
        None => {
            let to_end = u32::try_from(timeline.frame_count())
                .unwrap_or(u32::MAX)
                .saturating_sub(frame);
            let room = existing
                .and_then(|l| l.spans().iter().map(FrameSpan::start).find(|s| *s > frame))
                .map_or(u32::MAX, |next| next - frame);
            let length = duration
                .unwrap_or_else(|| NonZeroU32::new(to_end.min(room)).unwrap_or(NonZeroU32::MIN));
            if let Some(held) = existing.and_then(|l| hold_up_to(scene, l, frame)) {
                commands.extend(held);
            }
            FrameSpan::new(frame, length, first)
                .with_extras(contents.collect())
                .map_err(|_| DrawRefusal::TooMany)?
        }
    };
    let placed = (0..span.element_count())
        .map(|element| SpanRef {
            layer: layer_id,
            start: frame,
            element,
        })
        .collect();
    commands.push(Command::InsertSpan {
        scene,
        layer: layer_id,
        span,
    });
    Ok(DrawPlan {
        commands,
        layer: layer_id,
        placed,
    })
}

/// Drawing on a frame that only shows what an earlier keyframe holds: the frames before it keep
/// what they show, and a keyframe of its own starts at `frame` with a copy of everything held
/// and the new elements beside it, up to where the held span stopped (or, past its end, as far
/// as the scene goes). The held drawing stays; nothing is lost from the frames before.
#[allow(clippy::too_many_arguments)]
fn draw_beside_held(
    scene: SceneId,
    scene_frames: u64,
    duration: Option<NonZeroU32>,
    layer: &Layer,
    held: &FrameSpan,
    frame: u32,
    first: Content,
    more: Vec<Content>,
    mut commands: Vec<Command>,
) -> Result<DrawPlan, DrawRefusal> {
    if held.tween() != flare_document::Tween::None {
        return Err(DrawRefusal::Held);
    }
    let before = NonZeroU32::new(frame - held.start()).ok_or(DrawRefusal::TooMany)?;
    // Inside the held keyframe the new one lasts to where it ended. Past its end it lasts as
    // long as asked, else to the end of the scene.
    let length = if u64::from(frame) < held.end() {
        NonZeroU32::new(
            u32::try_from(held.end() - u64::from(frame)).map_err(|_| DrawRefusal::TooMany)?,
        )
    } else {
        duration.or_else(|| {
            NonZeroU32::new(
                u32::try_from(scene_frames)
                    .unwrap_or(u32::MAX)
                    .saturating_sub(frame),
            )
        })
    }
    .unwrap_or(NonZeroU32::MIN);
    let mut elements: Vec<Content> = held
        .elements()
        .filter(|element| !matches!(element, Content::Empty))
        .map(|element| fresh_content(element, DAffine2::IDENTITY))
        .collect::<Option<_>>()
        .ok_or(DrawRefusal::TooMany)?;
    let carried = elements.len();
    elements.push(first);
    elements.extend(more);
    let mut grown = FrameSpan::new(frame, length, Content::Empty).with_motion_of(held);
    grown = grown
        .with_elements(elements)
        .ok()
        .flatten()
        .ok_or(DrawRefusal::TooMany)?;
    let placed = (carried..grown.element_count())
        .map(|element| SpanRef {
            layer: layer.id(),
            start: frame,
            element,
        })
        .collect();
    commands.push(Command::RemoveSpan {
        scene,
        layer: layer.id(),
        start: held.start(),
    });
    commands.push(Command::InsertSpan {
        scene,
        layer: layer.id(),
        span: held.retimed(held.start(), before),
    });
    commands.push(Command::InsertSpan {
        scene,
        layer: layer.id(),
        span: grown,
    });
    Ok(DrawPlan {
        commands,
        layer: layer.id(),
        placed,
    })
}

/// The commands that make the last keyframe of `layer` before `frame` last up to it, when empty
/// frames lie between: the frames a new keyframe at `frame` leaves behind show what came before,
/// not nothing. `None` when no keyframe comes before `frame` or it already reaches it.
pub(crate) fn hold_up_to(scene: SceneId, layer: &Layer, frame: u32) -> Option<Vec<Command>> {
    let before = layer
        .spans()
        .iter()
        .rev()
        .find(|s| s.end() <= u64::from(frame))?;
    if before.end() == u64::from(frame) {
        return None;
    }
    let length = NonZeroU32::new(frame.checked_sub(before.start())?)?;
    Some(vec![
        Command::RemoveSpan {
            scene,
            layer: layer.id(),
            start: before.start(),
        },
        Command::InsertSpan {
            scene,
            layer: layer.id(),
            span: before.retimed(before.start(), length),
        },
    ])
}

/// Whether a folder around `layers[index]`, however deep, is locked.
fn locked_by_folder(layers: &[Layer], index: usize) -> bool {
    let mut parent = layers[index].parent();
    while let Some(id) = parent {
        let Some(folder) = layers[..index].iter().find(|l| l.id() == id) else {
            return false;
        };
        if folder.is_locked() {
            return true;
        }
        parent = folder.parent();
    }
    false
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod hold_tests;

#[cfg(test)]
mod merge_tests;
