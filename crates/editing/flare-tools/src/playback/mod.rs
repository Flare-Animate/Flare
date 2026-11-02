//! How a Graphic instance plays its symbol: Animate's Loop, Play Once and Single Frame, and the
//! frame it starts on.

use flare_commands::Command;
use flare_core::SceneId;
use flare_document::{Content, Document, Playback, SymbolKind};

use crate::common::*;

/// Gives the Graphic instance in a span another frame setting, keeping its identity, place,
/// duration and tween. The first frame is held inside the symbol's length. `None` when the span
/// is missing, is not an instance of a Graphic (a Movie Clip and a Button ignore the setting),
/// or already has it.
pub fn set_playback(
    document: &Document,
    scene: SceneId,
    at: SpanRef,
    playback: Playback,
) -> Option<Vec<Command>> {
    let span = find_span(document, scene, at)?;
    let Some(Content::Instance(instance)) = span.element(at.element) else {
        return None;
    };
    let symbol = document.symbol(instance.symbol())?;
    if symbol.kind() != SymbolKind::Graphic {
        return None;
    }
    let last = u32::try_from(symbol.timeline().frame_count().saturating_sub(1)).unwrap_or(u32::MAX);
    let playback = Playback {
        first_frame: playback.first_frame.min(last),
        ..playback
    };
    if playback == instance.playback() {
        return None;
    }
    let changed = instance.clone().with_playback(playback);
    replace_element(document, scene, at, Content::Instance(changed))
}

#[cfg(test)]
mod tests;
