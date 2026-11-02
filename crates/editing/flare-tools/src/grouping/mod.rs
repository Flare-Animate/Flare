//! Grouping shapes into a symbol and breaking an instance back into its parts.

use std::num::NonZeroU32;

use flare_commands::Command;
use flare_core::{InstanceId, LayerId, SceneId, SymbolId, Transform2D};
use flare_document::{Content, Document, FrameSpan, Instance, Layer, Symbol, Timeline, Tween};

use crate::common::*;

/// Groups shapes into one symbol instance: two or more shape elements become the layers of a new
/// Graphic symbol, and one instance of it takes the place of the first element of `spans` in its
/// keyframe, at the same height in the stacking order. `None` when fewer than two of them are
/// shapes. The other elements leave their keyframes; a keyframe left with nothing goes.
pub fn group(
    document: &Document,
    scene: SceneId,
    spans: &[SpanRef],
) -> Option<(Vec<Command>, SymbolId)> {
    let mut shapes = Vec::new();
    for at in spans {
        match find_span(document, scene, *at)?.element(at.element)? {
            Content::Shape(shape) => shapes.push((*at, shape)),
            _ => return None,
        }
    }
    if shapes.len() < 2 {
        return None;
    }
    // The symbol keeps the stacking the stage shows: the topmost layer first, and within a
    // keyframe the last element (drawn on top) first.
    let order: Vec<_> = scene_timeline(document, scene)?
        .layers()
        .iter()
        .map(|l| l.id())
        .collect();
    shapes.sort_by_key(|(at, _)| {
        (
            order.iter().position(|id| *id == at.layer),
            at.start,
            std::cmp::Reverse(at.element),
        )
    });
    let layers: Vec<Layer> = shapes
        .iter()
        .enumerate()
        .map(|(i, (_, shape))| {
            let inner = FrameSpan::new(0, NonZeroU32::MIN, Content::Shape((*shape).clone()));
            Layer::new(LayerId::new(), format!("Layer {}", i + 1), vec![inner]).ok()
        })
        .collect::<Option<_>>()?;
    let symbol_id = SymbolId::new();
    let name = format!("Group {}", document.symbols().len() + 1);
    let symbol = Symbol::new(symbol_id, name, Timeline::new(layers));
    let top = shapes[0].0;
    let instance = Instance::new(InstanceId::new(), symbol_id, Transform2D::IDENTITY);
    let mut commands = Vec::new();
    let mut instance = Some(Content::Instance(instance));
    for group in by_span(spans) {
        let Some(first) = group.first() else {
            continue;
        };
        let span = find_span(document, scene, *first)?;
        let holds_top = group.contains(&top);
        let mut kept = Vec::new();
        for (i, content) in span.elements().enumerate() {
            let chosen = group.iter().any(|at| at.element == i);
            if holds_top && i == top.element {
                kept.extend(instance.take());
            } else if !chosen {
                kept.push(content.clone());
            }
        }
        // A single element is replaced by the instance the way a span always was: no motion.
        let rebuilt = match kept.len() {
            0 => None,
            1 if span.element_count() == 1 => Some(FrameSpan::new(
                span.start(),
                span.duration(),
                kept.remove(0),
            )),
            _ => span.with_elements(kept).ok()?,
        };
        commands.push(Command::RemoveSpan {
            scene,
            layer: first.layer,
            start: first.start,
        });
        if let Some(rebuilt) = rebuilt {
            commands.push(Command::InsertSpan {
                scene,
                layer: first.layer,
                span: rebuilt,
            });
        }
    }
    // The symbol must exist before the instance of it is inserted.
    let at = commands
        .iter()
        .position(|c| matches!(c, Command::InsertSpan { .. }))
        .unwrap_or(commands.len());
    commands.insert(at, Command::AddSymbol(symbol));
    Some((commands, symbol_id))
}

/// Replaces an instance with what its symbol shows on its first frame: every element of every
/// layer's artwork, with fresh identities, placed where the instance had it. The parts go into
/// the instance's own keyframe in the stacking order the symbol had, in place of the instance.
/// `None` when the element is not an instance or its symbol shows nothing. When the keyframe
/// cannot hold several elements (it tweens), each part goes on a new layer.
pub fn break_apart(
    document: &Document,
    scene: SceneId,
    at: SpanRef,
) -> Option<(Vec<Command>, Vec<SpanRef>)> {
    let span = find_span(document, scene, at)?;
    let Content::Instance(instance) = span.element(at.element)? else {
        return None;
    };
    let symbol = document.symbol(instance.symbol())?;
    let matrix = instance.transform().as_affine();
    // (layer name, content) bottom layer first, each layer's own elements bottom first.
    let mut parts: Vec<(&str, Content)> = Vec::new();
    for layer in symbol.timeline().layers().iter().rev() {
        let Some(inner) = layer.span_at(0) else {
            continue;
        };
        for element in inner.elements() {
            if let Some(content) = fresh_content(element, matrix) {
                parts.push((layer.name(), content));
            }
        }
    }
    if parts.is_empty() {
        return None;
    }
    let mut elements: Vec<Content> = span.elements().cloned().collect();
    let tail = elements.split_off(at.element + 1);
    elements.pop();
    let first_new = elements.len();
    let count = parts.len();
    // A tweening keyframe is one object; its parts cannot share it.
    let direct = (span.tween() == Tween::None)
        .then(|| {
            span.with_elements(
                elements
                    .iter()
                    .cloned()
                    .chain(parts.iter().map(|(_, c)| c.clone()))
                    .chain(tail.iter().cloned())
                    .collect(),
            )
        })
        .and_then(|rebuilt| rebuilt.ok().flatten());
    if let Some(rebuilt) = direct.filter(|r| r.element_count() == span.element_count() - 1 + count)
    {
        let placed = (first_new..first_new + count)
            .map(|element| at.with_element(element))
            .collect();
        return Some((replace_span(scene, at, rebuilt), placed));
    }
    // The keyframe tweens, so its parts cannot share it: one new layer each, as before.
    let mut commands = remove_element(document, scene, at)?;
    let mut placed = Vec::new();
    for (name, content) in parts {
        let part = FrameSpan::new(span.start(), span.duration(), content);
        let (batch, spot) = new_layer_with(scene, name, part)?;
        commands.extend(batch);
        placed.push(spot);
    }
    Some((commands, placed))
}

/// Breaks apart every instance of `refs`, and takes apart every text of `refs` that is not one,
/// as one list of commands. Returns where the parts are afterwards: an instance's parts take its
/// place in the stack, so the elements above it move up by one less than the parts it made.
/// `None` when nothing could be broken apart.
pub fn break_apart_many(
    document: &Document,
    scene: SceneId,
    refs: &[SpanRef],
) -> Option<(Vec<Command>, Vec<SpanRef>)> {
    let (commands, results) = each_element_with(document, refs, |doc, at| {
        if let Some((commands, parts)) = break_apart(doc, scene, at) {
            return Some((commands, (at, parts)));
        }
        // Text becomes plain outlines in the same place.
        let commands = crate::text::break_text_apart(doc, scene, at)?;
        Some((commands, (at, vec![at])))
    })?;
    // Elements went from the highest index of a span to the lowest, so each break shifted
    // the ones already placed above it.
    let mut placed: Vec<(SpanRef, Vec<SpanRef>)> = Vec::new();
    for (at, parts) in results {
        let grew = parts.len().saturating_sub(1);
        for (_, earlier) in placed.iter_mut() {
            for part in earlier {
                if part.layer == at.layer && part.start == at.start && part.element > at.element {
                    part.element += grew;
                }
            }
        }
        placed.push((at, parts));
    }
    Some((commands, placed.into_iter().flat_map(|(_, p)| p).collect()))
}

#[cfg(test)]
mod tests;
