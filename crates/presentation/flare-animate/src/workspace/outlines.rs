//! The color each layer is marked with in the timeline (and drawn in by outline mode).

use super::Workspace;
use flare_commands::{Command, LayerChange};
use flare_core::LayerId;
use flare_document::OUTLINE_PALETTE;

impl Workspace {
    /// Gives the layer the next color of the palette: one undo step.
    pub fn cycle_layer_outline(&mut self, layer: LayerId) {
        let Some(scene) = self.scene_id() else { return };
        let Some(next) = self
            .document()
            .scenes()
            .first()
            .and_then(|s| {
                s.timeline()
                    .layers()
                    .iter()
                    .enumerate()
                    .find(|(_, l)| l.id() == layer)
            })
            .map(|(index, l)| {
                let current = l.outline_or_default(index);
                let at = OUTLINE_PALETTE.iter().position(|c| *c == current);
                OUTLINE_PALETTE[at.map_or(0, |i| i + 1) % OUTLINE_PALETTE.len()]
            })
        else {
            return;
        };
        self.run_batch(
            vec![Command::UpdateLayer {
                scene,
                layer,
                change: LayerChange::Outline(Some(next)),
            }],
            "change the outline color",
        );
    }
}
