//! Drawing into the layer: where a new shape goes, as Animate keeps everything drawn on a layer in
//! a keyframe of that layer.

use std::fmt;
use std::num::NonZeroU32;

use flare_commands::Command;
use flare_core::{LayerId, SceneId};
use flare_document::{Content, Document, FrameSpan, Layer, Shape};

use crate::common::{SpanRef, scene_timeline};

/// How many frames a keyframe made by drawing lasts at least, when it has no explicit length.
pub const MIN_DRAWN_FRAMES: u32 = 24;

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
/// scene (at least [`MIN_DRAWN_FRAMES`]) but not into the layer's next keyframe. With no layer
/// to draw on at all, a layer named `new_layer` is made first. A layer that is locked (itself or
/// by a folder), hidden, a folder, mask, guide or camera, and a keyframe that cannot hold several
/// elements, is refused.
pub fn draw_shapes(
    document: &Document,
    scene: SceneId,
    target: Target,
    frame: u32,
    duration: Option<NonZeroU32>,
    shapes: Vec<Shape>,
    new_layer: &str,
) -> Result<DrawPlan, DrawRefusal> {
    let contents = shapes.into_iter().map(Content::Shape).collect();
    place_contents(
        document, scene, target, frame, duration, contents, new_layer,
    )
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
            let layer = Layer::new(id, new_layer, vec![]).map_err(|_| DrawRefusal::TooMany)?;
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
                .saturating_sub(frame)
                .max(MIN_DRAWN_FRAMES);
            let room = existing
                .and_then(|l| l.spans().iter().map(FrameSpan::start).find(|s| *s > frame))
                .map_or(u32::MAX, |next| next - frame);
            let length = duration
                .unwrap_or_else(|| NonZeroU32::new(to_end.min(room)).unwrap_or(NonZeroU32::MIN));
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
