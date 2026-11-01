//! The Eraser tool and the Delete key: taking a span out of its layer.

use flare_commands::Command;
use flare_core::SceneId;

use crate::common::SpanRef;

pub fn delete(scene: SceneId, at: SpanRef) -> Vec<Command> {
    vec![Command::RemoveSpan {
        scene,
        layer: at.layer,
        start: at.start,
    }]
}

#[cfg(test)]
mod tests;
