//! The chosen rig drawn on the stage while editing (issue #49): its bones at the playhead's
//! frame, so a pose can be judged where the artwork is. Drawn only over the symbol being edited
//! or over a selected instance of it, since a rig lives in its symbol's own space.

use egui::{Color32, Painter, Pos2, Rect, Stroke};
use flare_document::Content;
use flare_tools::ops;
use glamx::DVec2;

use super::*;

impl Workspace {
    /// The rig's bones as stage-space segments, root to tip, when the rig of the Rigs panel's
    /// symbol is in view; `None` when it is not, there is no rig, or it fails to evaluate.
    pub(super) fn rig_bones_on_stage(&self) -> Option<Vec<(Pos2, Pos2)>> {
        let symbol = self.rig_symbol()?;
        let root = self.session.document();
        let rig = flare_editor::rig_library::find(root, symbol).ok()??;
        let to_stage = if self.contexts.last().is_some_and(|c| c.symbol == symbol) {
            glamx::DAffine2::IDENTITY
        } else {
            let (selected, scene) = (self.selected?, self.scene_id()?);
            match ops::find_element(self.document(), scene, selected)? {
                Content::Instance(instance) if instance.symbol() == symbol => {
                    instance.transform().as_affine()
                }
                _ => return None,
            }
        };
        let bones = flare_rig::evaluate_rig(&rig, self.frame).ok()?;
        Some(
            bones
                .iter()
                .map(|bone| {
                    let tail = DVec2::new(bone.origin[0], bone.origin[1])
                        + DVec2::new(bone.angle.cos(), bone.angle.sin()) * bone.length;
                    let at = |p: DVec2| {
                        let p = to_stage.transform_point2(p);
                        Pos2::new(p.x as f32, p.y as f32)
                    };
                    (at(DVec2::new(bone.origin[0], bone.origin[1])), at(tail))
                })
                .collect(),
        )
    }

    /// Draws the bones over the stage: a dark line under a light one reads on any color.
    pub(super) fn rig_overlay(&self, painter: &Painter, image: Rect, scale: f32) {
        let Some(bones) = self.rig_bones_on_stage() else {
            return;
        };
        let to_screen = |p: Pos2| image.min + p.to_vec2() * scale;
        for (from, to) in bones {
            let (a, b) = (to_screen(from), to_screen(to));
            painter.line_segment([a, b], Stroke::new(4.0, Color32::from_black_alpha(160)));
            painter.line_segment([a, b], Stroke::new(2.0, Color32::from_rgb(255, 170, 40)));
            painter.circle_filled(a, 3.5, Color32::WHITE);
        }
    }
}
