//! Tools and the selection.

use egui::{Pos2, Rect};
use flare_commands::Command;
use flare_core::{LayerId, SceneId};
use flare_document::{Content, Tween};
use flare_tools::ops::{self, SpanRef};
use glamx::DVec2;

use super::*;

impl Workspace {
    // ---- tools and selection ---------------------------------------------------------

    pub fn tool(&self) -> Tool {
        self.tool
    }

    pub fn set_tool(&mut self, tool: Tool) {
        if self.tool != tool {
            self.cancel_drag();
            self.pen.clear();
            self.finish_text_for_tool_change();
            self.wand_drop();
            self.tool = tool;
            if tool == Tool::Camera {
                self.ensure_camera();
            }
        }
    }

    pub fn has_selection(&self) -> bool {
        self.selected.is_some()
    }

    pub(super) fn scene_id(&self) -> Option<SceneId> {
        self.document().scenes().first().map(|scene| scene.id())
    }

    /// Drops a selection whose span no longer exists (after undo, delete or open).
    pub(super) fn validate_selection(&mut self) {
        self.selection_checked = Some((self.context_serial, self.active().revision()));
        if self
            .frame_range
            .is_some_and(|r| self.layer(r.layer).is_none())
        {
            self.frame_range = None;
        }
        if let Some(scene) = self.scene_id() {
            // One pass over the layers answers for every selected span.
            let editable: std::collections::HashMap<(LayerId, u32), usize> = self
                .document()
                .scenes()
                .iter()
                .find(|s| s.id() == scene)
                .into_iter()
                .flat_map(|s| s.timeline().layers())
                .filter(|l| l.is_visible() && !l.is_locked())
                .flat_map(|l| {
                    l.spans()
                        .iter()
                        .map(|span| ((l.id(), span.start()), span.element_count()))
                })
                .collect();
            let mut all: Vec<SpanRef> = self.selection_all();
            all.retain(|span| {
                editable
                    .get(&(span.layer, span.start))
                    .is_some_and(|count| span.element < *count)
            });
            self.selected = all.first().copied();
            self.also_selected = all.into_iter().skip(1).collect();
        } else {
            self.selected = None;
            self.also_selected.clear();
        }
        if let Some(symbol) = self.library.selected
            && self.document().symbol(symbol).is_none()
        {
            self.library.selected = None;
        }
    }

    /// The scene-level element of the topmost shape under `point` at the current frame,
    /// looking through locked layers.
    pub(super) fn hit(&self, point: Pos2) -> Option<SpanRef> {
        // Text is picked anywhere in its box, not only on the ink of its letters.
        self.glyph_hit(point).or_else(|| self.text_box_hit(point))
    }

    pub(super) fn glyph_hit(&self, point: Pos2) -> Option<SpanRef> {
        let scene = self.scene_id()?;
        let frame =
            flare_eval::evaluate_frame_without_camera(self.document(), scene, self.frame).ok()?;
        let at = DVec2::new(f64::from(point.x), f64::from(point.y));
        flare_render::hit_test_all(&frame, at)
            .into_iter()
            .filter_map(|index| {
                ops::locate_hit(self.document(), scene, self.frame, &frame.shapes[index])
            })
            .find(|span| self.on_unlocked_layer(*span))
    }

    /// Whether `span` is on a layer that can be changed: neither it nor a folder it is in is
    /// locked.
    pub(super) fn on_unlocked_layer(&self, span: SpanRef) -> bool {
        let Some(scene) = self.document().scenes().first() else {
            return false;
        };
        let layers = scene.timeline().layers();
        layers
            .iter()
            .position(|l| l.id() == span.layer)
            .is_some_and(|index| !locked_in_folders(layers, index))
    }

    /// Every selected span, the primary one first.
    pub(super) fn selection_all(&self) -> Vec<SpanRef> {
        self.selected
            .into_iter()
            .chain(self.also_selected.iter().copied())
            .collect()
    }

    /// Selects what is at `point`. With `additive` (Shift) the span is toggled in or out of
    /// the selection; without it the selection becomes just that span, or empty.
    pub(super) fn select_at(&mut self, point: Pos2, additive: bool) {
        let hit = self.hit(point);
        if !additive {
            self.selected = hit;
            self.also_selected.clear();
            return;
        }
        let Some(hit) = hit else { return };
        let mut all = self.selection_all();
        match all.iter().position(|s| *s == hit) {
            Some(i) => {
                all.remove(i);
            }
            None => all.push(hit),
        }
        self.selected = all.first().copied();
        self.also_selected = all.into_iter().skip(1).collect();
    }

    pub(super) fn run_batch(&mut self, commands: Vec<Command>, what: &str) {
        match self.active_mut().apply_labeled(commands, &step_label(what)) {
            Ok(()) => self.status = None,
            Err(error) => self.report(format!("Cannot {what}: {error}")),
        }
    }

    /// The union of the bounds of every selected span.
    pub(super) fn selection_bounds(&self) -> Option<Rect> {
        self.selection_rects().into_iter().reduce(|a, b| a.union(b))
    }

    /// The bounds of every selected span. Kept until the document or the selection changes, so
    /// painting a big selection each frame costs nothing after the first.
    pub(super) fn selection_rects(&self) -> Vec<Rect> {
        use std::hash::{Hash, Hasher};
        let spans = self.selection_all();
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        for span in &spans {
            span.layer.hash(&mut hasher);
            span.start.hash(&mut hasher);
            span.element.hash(&mut hasher);
        }
        // Mid-tween the box follows the playhead.
        self.frame.hash(&mut hasher);
        let stamp = (
            self.context_serial,
            self.active().revision(),
            hasher.finish(),
        );
        if let Some((cached, rects)) = &*self.selection_rects_cache.borrow()
            && *cached == stamp
        {
            return rects.clone();
        }
        let rects = self
            .scene_id()
            .map(|scene| ops::spans_bounds_at(self.document(), scene, &spans, self.frame))
            .unwrap_or_default();
        *self.selection_rects_cache.borrow_mut() = Some((stamp, rects.clone()));
        rects
    }

    /// Remembers every selected span for Paste.
    pub fn copy_selection(&mut self) {
        let Some(scene) = self.scene_id() else {
            return;
        };
        let items: Vec<ops::ClipItem> = self
            .selection_all()
            .into_iter()
            .filter_map(|span| ops::copy_span(self.document(), scene, span))
            .collect();
        if !items.is_empty() {
            self.clipboard = items;
        }
    }

    /// Copy, then delete: one undo step restores what was cut.
    pub fn cut_selection(&mut self) {
        if !self.has_selection() {
            return;
        }
        self.copy_selection();
        self.delete_selection();
    }

    /// Selects everything on unlocked layers at the current frame.
    pub fn select_all(&mut self) {
        let Some(scene) = self.scene_id() else { return };
        let all = ops::spans_at_frame(self.document(), scene, self.frame);
        self.selected = all.first().copied();
        self.also_selected = all.into_iter().skip(1).collect();
    }

    pub fn clear_selection(&mut self) {
        self.selected = None;
        self.also_selected.clear();
    }

    /// Pastes the remembered things at the playhead into the active layer's keyframe, together
    /// centered on the stage, and selects them.
    pub fn paste(&mut self) {
        self.paste_at(true);
    }

    /// Pastes the remembered spans where they were copied from.
    pub fn paste_in_place(&mut self) {
        self.paste_at(false);
    }

    fn paste_at(&mut self, centered: bool) {
        let Some(scene) = self.scene_id() else {
            return;
        };
        // The same move for every item, so a group keeps its arrangement.
        let delta = if centered {
            let bounds = self
                .clipboard
                .iter()
                .filter_map(|item| ops::clip_bounds(self.document(), item))
                .reduce(|a, b| a.union(b));
            let stage = self.document().stage();
            bounds.map_or(glamx::DVec2::ZERO, |b| {
                glamx::DVec2::new(
                    f64::from(stage.width()) / 2.0 - f64::from(b.center().x),
                    f64::from(stage.height()) / 2.0 - f64::from(b.center().y),
                )
            })
        } else {
            glamx::DVec2::ZERO
        };
        // Into the layer chosen in the timeline, at the playhead, as a drawing goes; with none
        // chosen (or a deleted one) the topmost that can take art.
        let target = self
            .current_layer
            .filter(|id| self.layer(*id).is_some())
            .map_or(ops::Target::Default, ops::Target::Layer);
        let plan = ops::paste_into(
            self.document(),
            scene,
            target,
            &self.clipboard,
            self.frame,
            delta,
            &tr!(self, "name-new-layer", &[("n", "1")]),
        );
        let plan = match plan {
            Ok(plan) => plan,
            Err(ops::PasteRefusal::MissingSymbol) => {
                self.report(tr!(self, "msg-paste-symbol-missing"));
                return;
            }
            Err(refusal) => {
                let name = self
                    .active_layer()
                    .map(|l| l.name().to_owned())
                    .unwrap_or_default();
                let reason = refusal.to_string();
                self.report(tr!(
                    self,
                    "msg-paste-refused",
                    &[("name", &name), ("reason", &reason)]
                ));
                return;
            }
        };
        if plan.commands.is_empty() {
            return;
        }
        self.run_batch(plan.commands, "paste");
        if self.status.is_none() {
            self.selected = plan.placed.first().copied();
            self.also_selected = plan.placed.iter().skip(1).copied().collect();
            self.current_layer = Some(plan.layer);
        }
    }

    pub fn delete_selection(&mut self) {
        let Some(scene) = self.scene_id() else {
            return;
        };
        let commands = ops::delete_many(self.document(), scene, &self.selection_all());
        if commands.is_empty() {
            return;
        }
        self.run_batch(commands, "delete");
        self.validate_selection();
    }

    /// The selected span, when it holds an instance or a shape (the two things that can tween).
    fn selected_tweenable(&self) -> Option<&flare_document::FrameSpan> {
        let (selected, scene) = (self.selected?, self.scene_id()?);
        let span = ops::find_span(self.document(), scene, selected)?;
        // In a keyframe of several elements the tween is the first element's.
        matches!(span.content(), Content::Instance(_) | Content::Shape(_)).then_some(span)
    }

    /// The selected span's tween, when it holds something that can tween.
    pub fn selection_tween(&self) -> Option<Tween> {
        self.selected_tweenable().map(|span| span.tween())
    }

    /// The easing of the selected span's tween, when it holds something that can tween.
    pub fn selection_ease(&self) -> Option<i8> {
        self.selected_tweenable().map(|span| span.ease())
    }

    /// The flare-tween interpolation of the selected span's tween, if it has chosen one.
    pub fn selection_interp(&self) -> Option<flare_document::SpanInterp> {
        self.selected_tweenable().and_then(|span| span.interp())
    }

    /// Sets (or with `None` clears) the selected span's interpolation, as one undo step.
    pub fn set_selection_interp(&mut self, interp: Option<flare_document::SpanInterp>) {
        let (Some(selected), Some(scene)) = (self.selected, self.scene_id()) else {
            return;
        };
        if let Some(commands) = ops::set_interpolation(self.document(), scene, selected, interp) {
            self.run_batch(commands, "change the tween's interpolation");
        }
    }

    /// The selected span is an instance, which a motion tween moves.
    pub fn selection_holds_instance(&self) -> bool {
        self.selected_tweenable()
            .is_some_and(|span| matches!(span.content(), Content::Instance(_)))
    }

    /// The selected span is a shape, which a shape tween changes.
    pub fn selection_holds_shape(&self) -> bool {
        self.selected_tweenable()
            .is_some_and(|span| matches!(span.content(), Content::Shape(_)))
    }

    /// Turns the tween on the selected span on or off: a motion tween on an instance, a shape
    /// tween on a shape.
    pub fn set_selection_tween(&mut self, tween: Tween) {
        let (Some(selected), Some(scene)) = (self.selected, self.scene_id()) else {
            return;
        };
        match ops::set_tween(self.document(), scene, selected, tween) {
            Some(commands) => self.run_batch(commands, "change the tween"),
            None => self.report(tr!(self, "msg-tween-needs-target")),
        }
    }

    /// Sets the easing of the selected span's tween, from -100 (starts slow) to 100 (ends slow).
    pub fn set_selection_ease(&mut self, ease: i8) {
        let (Some(selected), Some(scene)) = (self.selected, self.scene_id()) else {
            return;
        };
        if let Some(commands) = ops::set_ease(self.document(), scene, selected, ease) {
            self.run_batch(commands, "change the tween's ease");
        }
    }
}

/// Whether `layers[index]` is locked, itself or by a locked folder around it.
pub(super) fn locked_in_folders(layers: &[Layer], index: usize) -> bool {
    let mut current = Some(index);
    while let Some(i) = current {
        if layers[i].is_locked() {
            return true;
        }
        current = layers[i]
            .parent()
            .and_then(|parent| layers[..i].iter().position(|l| l.id() == parent));
    }
    false
}

/// The name the history list gives a step: what was asked, with a capital.
pub(super) fn step_label(what: &str) -> String {
    let mut label = what.to_owned();
    if let Some(first) = label.get_mut(0..1) {
        first.make_ascii_uppercase();
    }
    label
}
