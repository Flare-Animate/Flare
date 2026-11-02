//! Arrange: moving elements up and down the stacking order of their keyframe.

use flare_commands::Command;
use flare_core::SceneId;
use flare_document::{Content, Document};

use crate::common::*;

/// Where the chosen elements go among the others of their keyframe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stack {
    /// Above all the others.
    Front,
    /// One step up, past the element above.
    Forward,
    /// One step down, past the element below.
    Backward,
    /// Below all the others.
    Back,
}

/// The new order of `count` elements, as the old index now at each place, bottom first.
fn order(count: usize, chosen: &[bool], how: Stack) -> Vec<usize> {
    let mut order: Vec<usize> = (0..count).collect();
    match how {
        Stack::Front => order.sort_by_key(|i| chosen[*i]),
        Stack::Back => order.sort_by_key(|i| !chosen[*i]),
        Stack::Forward => {
            for place in (0..count.saturating_sub(1)).rev() {
                if chosen[order[place]] && !chosen[order[place + 1]] {
                    order.swap(place, place + 1);
                }
            }
        }
        Stack::Backward => {
            for place in 1..count {
                if chosen[order[place]] && !chosen[order[place - 1]] {
                    order.swap(place, place - 1);
                }
            }
        }
    }
    order
}

/// Moves the elements of `refs` within their keyframes, keeping the order they have among
/// themselves, as one list of commands, and returns where they are afterwards. `None` when
/// nothing would move (they are there already, or the references name nothing).
pub fn arrange_elements(
    document: &Document,
    scene: SceneId,
    refs: &[SpanRef],
    how: Stack,
) -> Option<(Vec<Command>, Vec<SpanRef>)> {
    let mut commands = Vec::new();
    let mut moved = Vec::new();
    for group in by_span(refs) {
        let Some(first) = group.first() else {
            continue;
        };
        let Some(span) = find_span(document, scene, *first) else {
            continue;
        };
        let count = span.element_count();
        let mut chosen = vec![false; count];
        for at in &group {
            if let Some(slot) = chosen.get_mut(at.element) {
                *slot = true;
            }
        }
        let order = order(count, &chosen, how);
        if order.iter().enumerate().all(|(place, old)| place == *old) {
            continue;
        }
        let elements: Vec<Content> = order
            .iter()
            .filter_map(|old| span.element(*old).cloned())
            .collect();
        let Ok(Some(rebuilt)) = span.with_elements(elements) else {
            continue;
        };
        commands.extend(replace_span(scene, *first, rebuilt));
        moved.extend(
            order
                .iter()
                .enumerate()
                .filter(|(_, old)| chosen[**old])
                .map(|(place, _)| first.with_element(place)),
        );
    }
    (!commands.is_empty()).then_some((commands, moved))
}

#[cfg(test)]
mod tests;
