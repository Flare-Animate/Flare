//! The Eraser tool and the Delete key: taking elements out of their spans.

use flare_commands::Command;
use flare_core::SceneId;
use flare_document::Document;

use crate::common::{SpanRef, by_span, find_span, remove_element, replace_span};

/// Takes the element `at` names out of its span: the span goes when it was the only element, and
/// stays with the others when not. Empty when the element is not there.
pub fn delete(document: &Document, scene: SceneId, at: SpanRef) -> Vec<Command> {
    remove_element(document, scene, at).unwrap_or_default()
}

/// [`delete`] for every element of `refs` as one list of commands: the elements of one span leave
/// together, and a span left with nothing goes.
pub fn delete_many(document: &Document, scene: SceneId, refs: &[SpanRef]) -> Vec<Command> {
    let mut out = Vec::new();
    for group in by_span(refs) {
        let Some(first) = group.first() else {
            continue;
        };
        let Some(span) = find_span(document, scene, *first) else {
            continue;
        };
        let kept: Vec<_> = span
            .elements()
            .enumerate()
            .filter(|(i, _)| !group.iter().any(|at| at.element == *i))
            .map(|(_, content)| content.clone())
            .collect();
        if kept.len() == span.element_count() {
            continue;
        }
        match span.with_elements(kept) {
            Ok(Some(rest)) => out.extend(replace_span(scene, *first, rest)),
            Ok(None) => out.push(Command::RemoveSpan {
                scene,
                layer: first.layer,
                start: first.start,
            }),
            Err(_) => {}
        }
    }
    out
}

#[cfg(test)]
mod tests;
