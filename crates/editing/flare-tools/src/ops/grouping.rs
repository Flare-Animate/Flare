//! Grouping shapes into a symbol and breaking an instance back into its parts.

use std::num::NonZeroU32;

use flare_commands::Command;
use flare_core::{InstanceId, LayerId, SceneId, SymbolId, Transform2D};
use flare_document::{Content, Document, FrameSpan, Instance, Layer, Symbol, Timeline};

use super::*;

/// Groups shapes into one symbol instance: two or more shape spans become the layers of a new
/// Graphic symbol, and one instance of it takes the place of the topmost span. `None` when fewer
/// than two of the spans are shapes. The other spans are removed.
pub fn group(
    document: &Document,
    scene: SceneId,
    spans: &[SpanRef],
) -> Option<(Vec<Command>, SymbolId)> {
    let mut shapes = Vec::new();
    for at in spans {
        let span = find_span(document, scene, *at)?;
        match span.content() {
            Content::Shape(shape) => shapes.push((*at, span, shape)),
            _ => return None,
        }
    }
    if shapes.len() < 2 {
        return None;
    }
    let layers: Vec<Layer> = shapes
        .iter()
        .enumerate()
        .map(|(i, (_, _, shape))| {
            let inner = FrameSpan::new(0, NonZeroU32::MIN, Content::Shape((*shape).clone()));
            Layer::new(LayerId::new(), format!("Layer {}", i + 1), vec![inner]).ok()
        })
        .collect::<Option<_>>()?;
    let symbol_id = SymbolId::new();
    let name = format!("Group {}", document.symbols().len() + 1);
    let symbol = Symbol::new(symbol_id, name, Timeline::new(layers));
    let (top_at, top_span, _) = shapes[0];
    let instance = Instance::new(InstanceId::new(), symbol_id, Transform2D::IDENTITY);
    let mut commands: Vec<Command> = shapes
        .iter()
        .map(|(at, _, _)| Command::RemoveSpan {
            scene,
            layer: at.layer,
            start: at.start,
        })
        .collect();
    commands.push(Command::AddSymbol(symbol));
    commands.push(Command::InsertSpan {
        scene,
        layer: top_at.layer,
        span: FrameSpan::new(
            top_span.start(),
            top_span.duration(),
            Content::Instance(instance),
        ),
    });
    Some((commands, symbol_id))
}

/// Replaces an instance with what its symbol shows on its first frame, each layer's artwork on a
/// new layer of the scene with fresh identities, placed where the instance had it. `None` when
/// the span is not an instance or its symbol shows nothing.
pub fn break_apart(
    document: &Document,
    scene: SceneId,
    at: SpanRef,
) -> Option<(Vec<Command>, Vec<SpanRef>)> {
    let span = find_span(document, scene, at)?;
    let Content::Instance(instance) = span.content() else {
        return None;
    };
    let symbol = document.symbol(instance.symbol())?;
    let matrix = instance.transform().as_affine();
    let mut commands = vec![Command::RemoveSpan {
        scene,
        layer: at.layer,
        start: at.start,
    }];
    let mut placed = Vec::new();
    for layer in symbol.timeline().layers() {
        let Some(inner) = layer.span_at(0) else {
            continue;
        };
        let Some(content) = fresh_content(inner.content(), matrix) else {
            continue;
        };
        let part = FrameSpan::new(span.start(), span.duration(), content);
        let (batch, spot) = new_layer_with(scene, layer.name(), part)?;
        commands.extend(batch);
        placed.push(spot);
    }
    (!placed.is_empty()).then_some((commands, placed))
}
