//! Finding a span of a scene's timeline.

use flare_commands::Command;
use flare_core::{LayerId, SceneId};
use flare_document::{Content, Document, FrameSpan, Timeline};

/// A selected element, by its layer, the start frame of its span (spans on a layer never overlap,
/// so this is unique) and its place among the span's elements (`FrameSpan::element`, bottom
/// first). A span of one element is `element: 0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpanRef {
    pub layer: LayerId,
    pub start: u32,
    pub element: usize,
}

impl SpanRef {
    /// The same span's element at `element`.
    #[must_use]
    pub fn with_element(self, element: usize) -> Self {
        Self { element, ..self }
    }
}

pub(crate) fn scene_timeline(document: &Document, scene: SceneId) -> Option<&Timeline> {
    document
        .scenes()
        .iter()
        .find(|s| s.id() == scene)
        .map(|s| s.timeline())
}

pub fn find_span(document: &Document, scene: SceneId, at: SpanRef) -> Option<&FrameSpan> {
    scene_timeline(document, scene)?
        .layers()
        .iter()
        .find(|layer| layer.id() == at.layer)?
        .spans()
        .iter()
        .find(|span| span.start() == at.start)
}

/// What the selection names: the element of the span at `at`.
pub fn find_element(document: &Document, scene: SceneId, at: SpanRef) -> Option<&Content> {
    find_span(document, scene, at)?.element(at.element)
}

/// The commands that put `span` in place of the one at `at` (same layer, same start).
pub(crate) fn replace_span(scene: SceneId, at: SpanRef, span: FrameSpan) -> Vec<Command> {
    vec![
        Command::RemoveSpan {
            scene,
            layer: at.layer,
            start: at.start,
        },
        Command::InsertSpan {
            scene,
            layer: at.layer,
            span,
        },
    ]
}

/// The commands that give the element `at` names the content `content`, in the span it is in,
/// keeping the other elements, the duration and the motion. `None` when the element is missing
/// or the span cannot hold the content.
pub(crate) fn replace_element(
    document: &Document,
    scene: SceneId,
    at: SpanRef,
    content: Content,
) -> Option<Vec<Command>> {
    let span = find_span(document, scene, at)?.with_element(at.element, content)?;
    Some(replace_span(scene, at, span))
}

/// The commands that take the element `at` names out of its span: the span goes when it was the
/// only element, and otherwise stays with the others. `None` when the element is missing.
pub(crate) fn remove_element(
    document: &Document,
    scene: SceneId,
    at: SpanRef,
) -> Option<Vec<Command>> {
    let span = find_span(document, scene, at)?;
    span.element(at.element)?;
    Some(match span.without_element(at.element) {
        Some(rest) => replace_span(scene, at, rest),
        None => vec![Command::RemoveSpan {
            scene,
            layer: at.layer,
            start: at.start,
        }],
    })
}
