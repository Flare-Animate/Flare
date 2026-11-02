//! One timeline's own content at a frame, without descending into the instances on it.
//!
//! [`evaluate_frame`](crate::evaluate_frame) flattens a scene into shapes; this keeps the
//! structure, which a format with symbols needs: each layer contributes its span's elements, bottom first (each a
//! shape, or an instance of a symbol with its own transform), and what the symbol shows is that
//! symbol's own timeline, evaluated the same way.

use std::borrow::Cow;

use flare_core::{InstanceId, SymbolId};
use flare_document::{Content, Document, Playback, Shape, Timeline};
use glamx::DAffine2;

use super::{ClipShape, EvalError, Frame, Steer, hidden_by_folder, shape_at, tweened, walk_layer};

/// What one layer shows.
#[derive(Debug, Clone, PartialEq)]
pub enum LevelContent<'a> {
    /// A shape, or a step of a shape tween (see [`DrawnShape::phase`](crate::DrawnShape::phase)).
    Shape { shape: Cow<'a, Shape>, phase: u64 },
    /// An instance of a symbol, placed by `transform` in this timeline's space.
    Instance {
        id: InstanceId,
        symbol: SymbolId,
        transform: DAffine2,
        /// The frame its span began at: a new span is a new instance, which starts the symbol
        /// again.
        span_start: u32,
        playback: Playback,
    },
}

/// One layer's content and the mask over it.
#[derive(Debug, Clone, PartialEq)]
pub struct LevelItem<'a> {
    /// The layer's place in the timeline's layers, counting from the top.
    pub layer: usize,
    /// Which element of the layer's span at the frame this is, counting from the bottom one.
    pub element: usize,
    pub content: LevelContent<'a>,
    /// The mask region this lies inside, as an index into [`Level::clips`].
    pub clip: Option<usize>,
}

/// A timeline at one frame: bottom layer first, so later items draw on top.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Level<'a> {
    pub items: Vec<LevelItem<'a>>,
    /// The mask regions the items refer to, in this timeline's space. A mask that holds an
    /// instance is flattened into the shapes it draws.
    pub clips: Vec<Vec<ClipShape<'a>>>,
    /// What the camera does to everything on this level: the identity unless the timeline has
    /// a camera layer.
    pub view: DAffine2,
}

/// `timeline` at `frame`, one level deep.
pub fn evaluate_level<'a>(
    document: &'a Document,
    timeline: &'a Timeline,
    frame: u32,
) -> Result<Level<'a>, EvalError> {
    let layers = timeline.layers();
    let mut level = Level {
        view: super::camera_view(document, timeline, frame),
        ..Level::default()
    };
    for (index, layer) in layers.iter().enumerate().rev() {
        if !layer.is_visible()
            || layer.is_mask()
            || layer.is_guide()
            || layer.is_camera()
            || layer.is_folder()
            || hidden_by_folder(layers, index)
        {
            continue;
        }
        let clip = match index
            .checked_sub(1)
            .map(|above| &layers[above])
            .filter(|above| above.is_mask() && above.is_visible())
        {
            Some(mask) => {
                let mut scratch = Frame::default();
                walk_layer(
                    document,
                    mask,
                    frame,
                    DAffine2::IDENTITY,
                    &mut Vec::new(),
                    Steer::default(),
                    &mut scratch,
                )?;
                level.clips.push(
                    scratch
                        .shapes
                        .into_iter()
                        .map(|drawn| ClipShape {
                            shape: drawn.shape,
                            phase: drawn.phase,
                            transform: drawn.transform,
                        })
                        .collect(),
                );
                Some(level.clips.len() - 1)
            }
            None => None,
        };
        let guide = index
            .checked_sub(1)
            .map(|above| &layers[above])
            .filter(|above| above.is_guide() && above.is_visible())
            .and_then(|above| above.span_at(frame))
            .and_then(|span| match span.content() {
                Content::Shape(shape) => Some(shape),
                _ => None,
            });
        let Some(span) = layer.span_at(frame) else {
            continue;
        };
        for (element, content) in span.elements().enumerate() {
            let content = match content {
                Content::Empty | Content::Camera(_) => continue,
                Content::Shape(shape) => {
                    // A tween is the first element's; the others stay as they are.
                    let (shape, phase) = if element == 0 {
                        shape_at(layer, span, frame, shape)
                    } else {
                        (std::borrow::Cow::Borrowed(shape), 0)
                    };
                    LevelContent::Shape { shape, phase }
                }
                Content::Instance(instance) => LevelContent::Instance {
                    id: instance.id(),
                    symbol: instance.symbol(),
                    transform: if element == 0 {
                        tweened(layer, frame, instance, guide)?
                    } else {
                        instance.transform().as_affine()
                    },
                    span_start: span.start(),
                    playback: instance.playback(),
                },
            };
            level.items.push(LevelItem {
                layer: index,
                element,
                content,
                clip,
            });
            if level.items.len() >= super::MAX_NODES {
                return Err(EvalError::TooManyNodes);
            }
        }
    }
    Ok(level)
}

/// The frame of a symbol an instance shows `elapsed` frames after its span began, as
/// [`evaluate_frame`](crate::evaluate_frame) counts it. A publisher that keeps symbols uses this
/// to know what a symbol's own clock has to do.
pub fn symbol_frame(
    kind: flare_document::SymbolKind,
    playback: Playback,
    elapsed: u64,
    length: u64,
) -> u32 {
    super::child_frame(kind, playback, elapsed, length)
}
