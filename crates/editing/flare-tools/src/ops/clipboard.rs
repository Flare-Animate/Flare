//! Copying, pasting, duplicating, converting to symbols and placing instances.

use flare_commands::Command;
use flare_core::{InstanceId, LayerId, SceneId, SymbolId, Transform2D};
use flare_document::{
    Content, Document, FrameSpan, Instance, Layer, Symbol, SymbolKind, Timeline, Tween,
};
use glamx::{DAffine2, DVec2};
use std::num::NonZeroU32;

use super::*;

/// A new layer holding one span, as the commands that add it.
pub(super) fn new_layer_with(
    scene: SceneId,
    name: &str,
    span: FrameSpan,
) -> Option<(Vec<Command>, SpanRef)> {
    let layer_id = LayerId::new();
    let layer = Layer::new(layer_id, name, vec![]).ok()?;
    let at = SpanRef {
        layer: layer_id,
        start: span.start(),
    };
    Some((
        vec![
            Command::AddLayer { scene, layer },
            Command::InsertSpan {
                scene,
                layer: layer_id,
                span,
            },
        ],
        at,
    ))
}

/// Copies a span onto a new layer above the others, moved by `offset`, with fresh shape and
/// instance identities so the copy is independent. Returns the commands and where the copy
/// lands. `None` when the span is missing, empty, or the result is not finite.
pub fn duplicate(
    document: &Document,
    scene: SceneId,
    at: SpanRef,
    offset: DVec2,
) -> Option<(Vec<Command>, SpanRef)> {
    duplicate_through(
        document,
        scene,
        at,
        DAffine2::from_translation(offset),
        "copy",
    )
}

/// [`duplicate`] through any `matrix` (in stage coordinates): a mirror image of a span is a
/// copy through a reflection. The new layer is named after the source and `suffix`.
pub fn duplicate_through(
    document: &Document,
    scene: SceneId,
    at: SpanRef,
    matrix: DAffine2,
    suffix: &str,
) -> Option<(Vec<Command>, SpanRef)> {
    if !matrix.is_finite() || matrix.matrix2.determinant().abs() < 1e-9 {
        return None;
    }
    let timeline = scene_timeline(document, scene)?;
    let source = timeline.layers().iter().find(|l| l.id() == at.layer)?;
    let span = find_span(document, scene, at)?;
    let content = fresh_content(span.content(), matrix)?;
    let copy = FrameSpan::new(span.start(), span.duration(), content).with_motion_of(span);
    new_layer_with(scene, &format!("{} {suffix}", source.name()), copy)
}

/// What Copy remembers about a span, so it can be pasted later or into another document.
#[derive(Clone, Debug, PartialEq)]
pub struct ClipItem {
    name: String,
    duration: NonZeroU32,
    tween: Tween,
    ease: i8,
    content: Content,
}

/// Remembers a span for pasting. `None` when it is missing or empty.
pub fn copy_span(document: &Document, scene: SceneId, at: SpanRef) -> Option<ClipItem> {
    let timeline = scene_timeline(document, scene)?;
    let source = timeline.layers().iter().find(|l| l.id() == at.layer)?;
    let span = find_span(document, scene, at)?;
    if matches!(span.content(), Content::Empty) {
        return None;
    }
    Some(ClipItem {
        name: source.name().to_owned(),
        duration: span.duration(),
        tween: span.tween(),
        ease: span.ease(),
        content: span.content().clone(),
    })
}

/// Pastes a remembered span in place on a new top layer, starting at `frame`, with fresh
/// identities. `None` when an instance's symbol is not in this document (pasting into a
/// document that lacks it would be invalid) or the frame range does not fit.
pub fn paste(
    document: &Document,
    scene: SceneId,
    item: &ClipItem,
    frame: u32,
) -> Option<(Vec<Command>, SpanRef)> {
    paste_offset(document, scene, item, frame, DVec2::ZERO)
}

/// Where a remembered span lies on the stage, for placing a paste.
pub fn clip_bounds(document: &Document, item: &ClipItem) -> Option<emath::Rect> {
    bounds::content_bounds(document, &item.content)
}

/// [`paste`] moved by `delta` stage units.
pub fn paste_offset(
    document: &Document,
    scene: SceneId,
    item: &ClipItem,
    frame: u32,
    delta: DVec2,
) -> Option<(Vec<Command>, SpanRef)> {
    if let Content::Instance(instance) = &item.content
        && document.symbol(instance.symbol()).is_none()
    {
        return None;
    }
    frame.checked_add(item.duration.get())?;
    let content = fresh_content(&item.content, DAffine2::from_translation(delta))?;
    let span = FrameSpan::new(frame, item.duration, content)
        .with_tween(item.tween)
        .with_ease(item.ease);
    new_layer_with(scene, &item.name, span)
}

/// Turns a shape span into a Graphic symbol plus an identity instance in its place.
/// Returns the commands (one atomic batch) and the new symbol's ID.
pub fn convert_to_symbol(
    document: &Document,
    scene: SceneId,
    at: SpanRef,
) -> Option<(Vec<Command>, SymbolId)> {
    convert_to_symbol_as(document, scene, at, SymbolKind::Graphic)
}

/// [`convert_to_symbol`] making a symbol of the given kind.
pub fn convert_to_symbol_as(
    document: &Document,
    scene: SceneId,
    at: SpanRef,
    kind: SymbolKind,
) -> Option<(Vec<Command>, SymbolId)> {
    let span = find_span(document, scene, at)?;
    let Content::Shape(shape) = span.content() else {
        return None;
    };
    let symbol_id = SymbolId::new();
    let inner = FrameSpan::new(0, NonZeroU32::MIN, Content::Shape(shape.clone()));
    let layer = Layer::new(LayerId::new(), "Layer 1", vec![inner]).ok()?;
    let name = format!("Symbol {}", document.symbols().len() + 1);
    let symbol = Symbol::new(symbol_id, name, Timeline::new(vec![layer])).with_kind(kind);
    let instance = Instance::new(InstanceId::new(), symbol_id, Transform2D::IDENTITY);
    let outer = FrameSpan::new(span.start(), span.duration(), Content::Instance(instance));
    Some((
        vec![
            Command::RemoveSpan {
                scene,
                layer: at.layer,
                start: at.start,
            },
            Command::AddSymbol(symbol),
            Command::InsertSpan {
                scene,
                layer: at.layer,
                span: outer,
            },
        ],
        symbol_id,
    ))
}

/// Places a symbol on a new layer, centered on `at`, for the whole scene length.
pub fn place_instance(
    document: &Document,
    scene: SceneId,
    symbol: SymbolId,
    at: DVec2,
) -> Option<Vec<Command>> {
    let timeline = scene_timeline(document, scene)?;
    let center = symbol_bounds(document, symbol).map_or(DVec2::ZERO, |b| {
        DVec2::new(f64::from(b.center().x), f64::from(b.center().y))
    });
    let transform = Transform2D::new(DAffine2::from_translation(at - center)).ok()?;
    let duration = u32::try_from(timeline.frame_count())
        .unwrap_or(u32::MAX)
        .max(24);
    let instance = Instance::new(InstanceId::new(), symbol, transform);
    let span = FrameSpan::new(0, NonZeroU32::new(duration)?, Content::Instance(instance));
    let layer_id = LayerId::new();
    let layer = Layer::new(layer_id, "Instance", vec![]).ok()?;
    Some(vec![
        Command::AddLayer { scene, layer },
        Command::InsertSpan {
            scene,
            layer: layer_id,
            span,
        },
    ])
}
