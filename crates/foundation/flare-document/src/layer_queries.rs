//! Questions about layers that more than one part of Flare asks, answered from the document alone.

use std::collections::HashSet;

use flare_core::{LayerId, SoundId};

use crate::{Document, Layer, Timeline};

impl Document {
    /// The sounds that only the cues of `layer` play, in any scene or symbol: what removing the
    /// layer would leave unused.
    pub fn sounds_only_played_by(&self, layer: LayerId) -> Vec<SoundId> {
        let timelines = self
            .scenes()
            .iter()
            .map(|s| s.timeline())
            .chain(self.symbols().iter().map(|s| s.timeline()));
        let (mut own, mut elsewhere) = (Vec::new(), HashSet::new());
        for timeline in timelines {
            for l in timeline.layers() {
                if l.id() == layer {
                    own.extend(l.cues().iter().map(|c| c.sound));
                } else {
                    elsewhere.extend(l.cues().iter().map(|c| c.sound));
                }
            }
        }
        own.sort_by_key(|id| *id.as_uuid());
        own.dedup();
        own.retain(|id| !elsewhere.contains(id));
        own
    }
}

impl Timeline {
    /// The folders `layer` could go into: those above it, other than the one it is in.
    pub fn folders_for(&self, layer: LayerId) -> Vec<&Layer> {
        let layers = self.layers();
        let Some(position) = layers.iter().position(|l| l.id() == layer) else {
            return Vec::new();
        };
        let current = layers[position].parent();
        layers[..position]
            .iter()
            .filter(|l| l.is_folder() && Some(l.id()) != current)
            .collect()
    }
}

#[cfg(test)]
#[path = "layer_queries_tests.rs"]
mod tests;
