//! Acting on several selected elements at once, when some of them share a span.
//!
//! Every edit replaces the span an element lives in, so two edits built from the same document
//! would each start from the span as it was and the second would undo the first. Elements that
//! share a span are therefore edited one after another on a working copy; elements alone in
//! their span are edited straight from the document.

use std::collections::HashMap;

use flare_commands::{Command, apply_all};
use flare_core::LayerId;
use flare_document::Document;

use super::span::SpanRef;

/// `refs` grouped by span, each group's elements from the highest index down (so removing one
/// does not move the next), groups in the order their span first appears.
pub(crate) fn by_span(refs: &[SpanRef]) -> Vec<Vec<SpanRef>> {
    let mut slots: HashMap<(LayerId, u32), usize> = HashMap::new();
    let mut groups: Vec<Vec<SpanRef>> = Vec::new();
    for at in refs {
        let slot = *slots.entry((at.layer, at.start)).or_insert_with(|| {
            groups.push(Vec::new());
            groups.len() - 1
        });
        if !groups[slot].iter().any(|seen| seen.element == at.element) {
            groups[slot].push(*at);
        }
    }
    for group in &mut groups {
        group.sort_by_key(|at| std::cmp::Reverse(at.element));
    }
    groups
}

/// The commands of `edit` for each of `refs`, as one list that applies in order, with what each
/// edit also gave back (where a copy landed, say). `edit` gets the document the earlier edits of
/// the same span have been applied to. An element `edit` has nothing to do for is skipped. `None`
/// when no element gave any command.
pub fn each_element_with<T>(
    document: &Document,
    refs: &[SpanRef],
    mut edit: impl FnMut(&Document, SpanRef) -> Option<(Vec<Command>, T)>,
) -> Option<(Vec<Command>, Vec<T>)> {
    let mut out = Vec::new();
    let mut extra = Vec::new();
    for group in by_span(refs) {
        let mut working: Option<Document> = None;
        for (i, at) in group.iter().enumerate() {
            let basis = working.as_ref().unwrap_or(document);
            let Some((commands, value)) = edit(basis, *at) else {
                continue;
            };
            if commands.is_empty() {
                continue;
            }
            // A single element needs no working copy; a second one in the span does.
            if i + 1 < group.len() {
                working = apply_all(basis, commands.iter().cloned()).ok().flatten();
                working.as_ref()?;
            }
            out.extend(commands);
            extra.push(value);
        }
    }
    (!out.is_empty()).then_some((out, extra))
}

/// [`each_element_with`] for edits with nothing to give back.
pub fn each_element(
    document: &Document,
    refs: &[SpanRef],
    mut edit: impl FnMut(&Document, SpanRef) -> Option<Vec<Command>>,
) -> Option<Vec<Command>> {
    each_element_with(document, refs, |doc, at| Some((edit(doc, at)?, ()))).map(|(c, _)| c)
}
