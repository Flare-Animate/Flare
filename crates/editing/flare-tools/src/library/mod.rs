//! The library workflow commands of Animate's own toolbar scripts (issue #11, discussion #8):
//! Increment Instances, Add Instance to Symbol and Bake Transform. Each looks at the document
//! and returns the commands to run as one undo step, or says why it would do nothing, so the
//! window and the command line ask the same thing and get the same answer. Create New Symbol and
//! Convert to Symbol, with the name and type Animate's dialog asks for, are in `symbols`.

use flare_commands::Command;
use flare_core::{SceneId, SymbolId, Transform2D};
use flare_document::{
    BitmapInstance, Content, Document, FrameSpan, Instance, Playback, Timeline, Tween,
};
use glamx::DAffine2;
use std::num::NonZeroU32;
use thiserror::Error;

use crate::common::*;

mod symbols;
pub use symbols::{
    MAX_SYMBOL_NAME_CHARS, Registration, SymbolError, check_symbol_name, convert_to_symbol,
    create_symbol, create_symbol_with, free_symbol_name,
};

/// Why a library operation was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum LibraryError {
    #[error("there is no such symbol in the library")]
    NoSuchSymbol,
    #[error("there is nothing picked to add")]
    NothingSelected,
    #[error("the picked element is not a symbol instance")]
    NotAnInstance,
    #[error("that element cannot be placed in a symbol")]
    NotPlaceable,
    #[error("A symbol cannot contain an instance of itself")]
    Cycle,
    #[error("A symbol's timeline has no layer to place the frame in")]
    NoLayer,
    #[error("the timeline would not accept the change")]
    TimelineRefused,
    #[error("nothing would change")]
    NoChange,
    #[error("the shift would move a frame before the symbol's first")]
    BeforeFirst,
    #[error("the transform cannot be applied to the artwork")]
    Singular,
    #[error("A bake needs a timeline with no motion tween in it")]
    Tweened,
    #[error("A bake cannot carry the artwork's cameras and armatures")]
    NotSupported,
}

/// The same playback with its first frame moved by `by`. `None` says that leaves the frame
/// range, and the caller refuses the whole turn.
fn shifted(playback: Playback, by: i64) -> Option<Playback> {
    let first = i64::from(playback.first_frame).checked_add(by)?;
    u32::try_from(first).ok().map(|first_frame| Playback {
        mode: playback.mode,
        first_frame,
    })
}

/// One span whose contents map through `make`: the rebuilt span, or `None` when `make`
/// declined everything. `Err` is the first refusal.
fn rebuild_span(
    span: &FrameSpan,
    make: &mut impl FnMut(&Content) -> Result<Option<Content>, LibraryError>,
) -> Result<Option<FrameSpan>, LibraryError> {
    let mut moved = false;
    let mut elements = Vec::with_capacity(span.element_count());
    for content in span.elements() {
        if let Some(edited) = make(content)? {
            elements.push(edited);
            moved = true;
        } else {
            elements.push(content.clone());
        }
    }
    if !moved {
        return Ok(None);
    }
    Ok(Some(
        span.with_elements(elements)
            .map_err(|_| LibraryError::TimelineRefused)?
            .unwrap_or_else(|| span.clone()),
    ))
}

/// [`rebuild_span`] over a whole timeline: the rebuilt timeline, or `None` when nothing in it
/// would change. Unchanged layers are kept as they are.
fn rebuild_timeline(
    timeline: &Timeline,
    make: &mut impl FnMut(&Content) -> Result<Option<Content>, LibraryError>,
) -> Result<Option<Timeline>, LibraryError> {
    let mut moved = false;
    let mut layers = Vec::with_capacity(timeline.layers().len());
    for layer in timeline.layers() {
        let mut spans = Vec::with_capacity(layer.spans().len());
        for span in layer.spans() {
            if let Some(replacement) = rebuild_span(span, make)? {
                spans.push(replacement);
                moved = true;
            } else {
                spans.push(span.clone());
            }
        }
        if moved {
            layers.push(
                layer
                    .with_spans(spans)
                    .map_err(|_| LibraryError::TimelineRefused)?,
            );
        } else {
            layers.push(layer.clone());
        }
    }
    if !moved {
        return Ok(None);
    }
    Ok(Some(Timeline::new(layers)))
}

/// Increment Instances (issue #11): every instance of `symbol`, on every timeline of the
/// document, whose playback begins at frame `from` or later begins `by` frames further on, so
/// a new run of frames appended to the symbol is not part of what instances already show.
/// `by` may be negative.
pub fn increment_instances(
    document: &Document,
    symbol: SymbolId,
    from: u32,
    by: i64,
) -> Result<Vec<Command>, LibraryError> {
    if by == 0 {
        return Err(LibraryError::NoChange);
    }
    document.symbol(symbol).ok_or(LibraryError::NoSuchSymbol)?;
    let qualifies = |instance: &Instance| {
        instance.symbol() == symbol && instance.playback().first_frame >= from
    };
    let mut commands = Vec::new();
    for scene in document.scenes() {
        for layer in scene.timeline().layers() {
            for span in layer.spans() {
                for element in span.elements() {
                    let Content::Instance(instance) = element else {
                        continue;
                    };
                    if !qualifies(instance) {
                        continue;
                    }
                    let Some(playback) = shifted(instance.playback(), by) else {
                        return Err(LibraryError::BeforeFirst);
                    };
                    commands.push(Command::SetInstancePlayback {
                        scene: scene.id(),
                        layer: layer.id(),
                        start: span.start(),
                        playback,
                    });
                }
            }
        }
    }
    // Instances inside other symbols read the same frames, so their timelines take the same
    // shift, as one replaced timeline rather than one command per span. A symbol cannot hold
    // an instance of itself, so its own timeline never changes.
    let mut bump = |content: &Content| {
        if let Content::Instance(instance) = content
            && qualifies(instance)
        {
            let Some(playback) = shifted(instance.playback(), by) else {
                return Err(LibraryError::BeforeFirst);
            };
            return Ok(Some(Content::Instance(
                Instance::new(instance.id(), instance.symbol(), instance.transform())
                    .with_playback(playback),
            )));
        }
        Ok(None)
    };
    for other in document.symbols() {
        if let Some(timeline) = rebuild_timeline(other.timeline(), &mut bump)? {
            commands.push(Command::ReplaceSymbolTimeline {
                symbol: other.id(),
                timeline,
            });
        }
    }
    (!commands.is_empty())
        .then_some(commands)
        .ok_or(LibraryError::NoChange)
}

/// Add Instance to Symbol (issue #11): the element the pick names becomes one frame at the end
/// of `symbol`'s timeline, as a copy: what is on the scene stays where it is. The copy lines
/// up in the symbol's own coordinates, so it is drawn where its pivot stood.
pub fn add_instance_to_symbol(
    document: &Document,
    symbol: SymbolId,
    scene: SceneId,
    at: SpanRef,
) -> Result<Vec<Command>, LibraryError> {
    let target = document.symbol(symbol).ok_or(LibraryError::NoSuchSymbol)?;
    let content = find_element(document, scene, at).ok_or(LibraryError::NothingSelected)?;
    match content {
        Content::Empty | Content::Camera(_) | Content::Armature(_) => {
            return Err(LibraryError::NotPlaceable);
        }
        Content::Instance(instance) if instance.symbol() == symbol => {
            return Err(LibraryError::Cycle);
        }
        _ => {}
    }
    let first = target
        .timeline()
        .layers()
        .first()
        .ok_or(LibraryError::NoLayer)?;
    let start = first.spans().last().map_or(0, |s| s.end() as u32);
    // A copy: the shape keeps its look under a fresh identity, as a paste does, since a
    // document lets a shape id name one shape only.
    let copy = fresh_content(content, DAffine2::IDENTITY).ok_or(LibraryError::NotPlaceable)?;
    let mut spans = first.spans().to_vec();
    spans.push(FrameSpan::new(start, NonZeroU32::MIN, copy));
    let placed = first
        .with_spans(spans)
        .map_err(|_| LibraryError::TimelineRefused)?;
    let mut layers = target.timeline().layers().to_vec();
    layers[0] = placed;
    let timeline = Timeline::new(layers);
    Ok(vec![Command::ReplaceSymbolTimeline { symbol, timeline }])
}

/// What artwork content is once `matrix` is part of it.
fn bake_content(content: &Content, matrix: DAffine2) -> Result<Option<Content>, LibraryError> {
    match content {
        Content::Empty => Ok(None),
        Content::Camera(_) | Content::Armature(_) => Err(LibraryError::NotSupported),
        Content::Shape(shape) => {
            let shape = transform_shape(shape, matrix).ok_or(LibraryError::Singular)?;
            Ok(Some(Content::Shape(shape)))
        }
        Content::Bitmap(placed) => {
            let transform = Transform2D::new(matrix * placed.transform().as_affine())
                .map_err(|_| LibraryError::Singular)?;
            Ok(Some(Content::Bitmap(
                BitmapInstance::new(placed.id(), placed.bitmap(), transform)
                    .with_opacity(placed.opacity()),
            )))
        }
        Content::Instance(instance) => {
            let transform = Transform2D::new(matrix * instance.transform().as_affine())
                .map_err(|_| LibraryError::Singular)?;
            Ok(Some(Content::Instance(
                Instance::new(instance.id(), instance.symbol(), transform)
                    .with_playback(instance.playback()),
            )))
        }
    }
}

/// Bake Transform (issue #11): the picked instance's transform becomes part of its symbol's
/// artwork, and every instance of that symbol everywhere takes the reverse transform, so
/// nothing seen changes; the picked one ends with no transform of its own. Needs artwork with
/// no motion tween in it.
pub fn bake_transform(
    document: &Document,
    scene: SceneId,
    at: SpanRef,
) -> Result<Vec<Command>, LibraryError> {
    let Some(Content::Instance(picked)) = find_element(document, scene, at) else {
        return Err(LibraryError::NotAnInstance);
    };
    let symbol = picked.symbol();
    let target = document.symbol(symbol).ok_or(LibraryError::NoSuchSymbol)?;
    let matrix = picked.transform().as_affine();
    if matrix == DAffine2::IDENTITY {
        return Err(LibraryError::NoChange);
    }
    // A singular matrix's inverse is not finite; nothing sensible can be baked then.
    let inverse = matrix.inverse();
    if !inverse.is_finite() {
        return Err(LibraryError::Singular);
    }
    for layer in target.timeline().layers() {
        for span in layer.spans() {
            if span.tween() != Tween::None {
                return Err(LibraryError::Tweened);
            }
        }
    }
    let mut bake = |content: &Content| bake_content(content, matrix);
    let baked = rebuild_timeline(target.timeline(), &mut bake)?.ok_or(LibraryError::NoChange)?;
    let mut commands = vec![Command::ReplaceSymbolTimeline {
        symbol,
        timeline: baked,
    }];
    // Every other place the symbol is shown takes the reverse transform. The picked one's
    // product with it is the identity, so it ends with no transform of its own.
    let mut let_loose = |content: &Content| match content {
        Content::Instance(instance) if instance.symbol() == symbol => {
            let transform = Transform2D::new(instance.transform().as_affine() * inverse)
                .map_err(|_| LibraryError::Singular)?;
            Ok(Some(Content::Instance(
                Instance::new(instance.id(), instance.symbol(), transform)
                    .with_playback(instance.playback()),
            )))
        }
        _ => Ok(None),
    };
    for each in document.scenes() {
        for layer in each.timeline().layers() {
            for span in layer.spans() {
                if let Some(replacement) = rebuild_span(span, &mut let_loose)? {
                    let at = SpanRef {
                        layer: layer.id(),
                        start: span.start(),
                        element: 0,
                    };
                    commands.extend(replace_span(each.id(), at, replacement));
                }
            }
        }
    }
    for other in document.symbols() {
        if let Some(timeline) = rebuild_timeline(other.timeline(), &mut let_loose)? {
            commands.push(Command::ReplaceSymbolTimeline {
                symbol: other.id(),
                timeline,
            });
        }
    }
    Ok(commands)
}

#[cfg(test)]
mod tests;
