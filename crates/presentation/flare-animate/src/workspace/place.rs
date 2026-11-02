//! Putting drawn shapes into the active layer, as Animate keeps everything drawn on a layer.

use flare_document::Shape;
use flare_tools::ops::{self, DrawRefusal, Target};

use super::*;

impl Workspace {
    /// Adds `shape` to the active layer's keyframe at the playhead, as one undo step.
    pub(super) fn draw_shape(&mut self, shape: Shape, name: &str) {
        self.draw_shapes(vec![shape], name);
    }

    /// Adds every shape to the active layer's keyframe at the playhead, in order and on top of
    /// what is there, as one undo step (`flare_tools::ops::draw_shapes` decides where they go).
    /// A layer that is locked, hidden, a folder, mask, guide or camera, and a keyframe that
    /// cannot hold several elements, is refused with a message saying why.
    pub(super) fn draw_shapes(&mut self, shapes: Vec<Shape>, name: &str) {
        if shapes.is_empty() {
            return;
        }
        let Some(scene) = self.scene_id() else {
            let message = tr!(self, "tools-no-scene");
            self.report(message);
            return;
        };
        // German nouns keep their capital letter inside a sentence; the other languages that have
        // capitals write a common noun in lower case.
        let noun = if self.language() == flare_i18n::Language::German {
            name.to_owned()
        } else {
            name.to_lowercase()
        };
        let draw_label = tr!(self, "tools-draw-label", &[("name", &noun)]);
        // The layer chosen in the timeline; with none chosen (or a deleted one) the topmost that
        // can take art.
        let target = self
            .current_layer
            .filter(|id| self.layer(*id).is_some())
            .map_or(Target::Default, Target::Layer);
        let plan = ops::draw_shapes(
            self.document(),
            scene,
            target,
            self.frame,
            None,
            shapes,
            &tr!(self, "name-new-layer", &[("n", "1")]),
        );
        let plan = match plan {
            Ok(plan) => plan,
            Err(refusal) => {
                let message = self.drawing_refusal(&refusal);
                self.report(message);
                return;
            }
        };
        match self.active_mut().apply_labeled(plan.commands, &draw_label) {
            Ok(()) => {
                self.status = None;
                self.current_layer = Some(plan.layer);
            }
            Err(error) => {
                let message = tr!(self, "tools-draw-error", &[("error", &error.to_string())]);
                self.report(message);
            }
        }
    }

    fn drawing_refusal(&self, refusal: &DrawRefusal) -> String {
        let name = self
            .active_layer()
            .map(|l| l.name().to_owned())
            .unwrap_or_default();
        let args = [("name", name.as_str())];
        match refusal {
            DrawRefusal::Locked => tr!(self, "tools-draw-locked", &args),
            DrawRefusal::Hidden => tr!(self, "tools-draw-hidden", &args),
            DrawRefusal::Held | DrawRefusal::TooMany => tr!(self, "tools-draw-held", &args),
            DrawRefusal::NotArt | DrawRefusal::NoSuchLayer | DrawRefusal::NoSuchScene => {
                tr!(self, "tools-draw-not-art", &args)
            }
        }
    }
}
