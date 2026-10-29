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
            self.text_edit = None;
            self.tool = tool;
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
            let editable: std::collections::HashSet<(LayerId, u32)> = self
                .document()
                .scenes()
                .iter()
                .find(|s| s.id() == scene)
                .into_iter()
                .flat_map(|s| s.timeline().layers())
                .filter(|l| l.is_visible() && !l.is_locked())
                .flat_map(|l| l.spans().iter().map(|span| (l.id(), span.start())))
                .collect();
            let mut all: Vec<SpanRef> = self.selection_all();
            all.retain(|span| editable.contains(&(span.layer, span.start)));
            self.selected = all.first().copied();
            self.also_selected = all.into_iter().skip(1).collect();
        } else {
            self.selected = None;
            self.also_selected.clear();
        }
        if let Some(symbol) = self.selected_symbol
            && self.document().symbol(symbol).is_none()
        {
            self.selected_symbol = None;
        }
    }

    /// The scene-level span of the topmost shape under `point` at the current frame,
    /// looking through locked layers.
    pub(super) fn hit(&self, point: Pos2) -> Option<SpanRef> {
        let scene = self.scene_id()?;
        let frame = flare_eval::evaluate_frame(self.document(), scene, self.frame).ok()?;
        let at = DVec2::new(f64::from(point.x), f64::from(point.y));
        let timeline = self.document().scenes().first()?.timeline();
        flare_render::hit_test_all(&frame, at)
            .into_iter()
            .filter_map(|index| {
                ops::locate_hit(self.document(), scene, self.frame, &frame.shapes[index])
            })
            .find(|span| {
                timeline
                    .layers()
                    .iter()
                    .find(|l| l.id() == span.layer)
                    .is_some_and(|l| !l.is_locked())
            })
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
        match self.active_mut().apply(commands) {
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
        }
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
            .map(|scene| ops::spans_bounds(self.document(), scene, &spans))
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

    /// Pastes the remembered spans in place at the current frame, each on its own new layer,
    /// and selects them.
    pub fn paste(&mut self) {
        let Some(scene) = self.scene_id() else {
            return;
        };
        let mut commands = Vec::new();
        let mut pasted = Vec::new();
        for item in &self.clipboard {
            match ops::paste(self.document(), scene, item, self.frame) {
                Some((batch, at)) => {
                    commands.extend(batch);
                    pasted.push(at);
                }
                None => {
                    self.report("Cannot paste: its symbol is not in this document");
                    return;
                }
            }
        }
        if commands.is_empty() {
            return;
        }
        self.run_batch(commands, "paste");
        if self.status.is_none() {
            self.selected = pasted.first().copied();
            self.also_selected = pasted.iter().skip(1).copied().collect();
            self.current_layer = self.selected.map(|s| s.layer);
        }
    }

    pub fn delete_selection(&mut self) {
        let Some(scene) = self.scene_id() else {
            return;
        };
        let commands: Vec<Command> = self
            .selection_all()
            .into_iter()
            .flat_map(|span| ops::delete(scene, span))
            .collect();
        if commands.is_empty() {
            return;
        }
        self.run_batch(commands, "delete");
        self.validate_selection();
    }

    /// The selected span's tween, when it is an instance (the only thing that can tween).
    pub fn selection_tween(&self) -> Option<Tween> {
        let (selected, scene) = (self.selected?, self.scene_id()?);
        let span = ops::find_span(self.document(), scene, selected)?;
        matches!(span.content(), Content::Instance(_)).then(|| span.tween())
    }

    /// Turns the motion tween on the selected instance span on or off.
    pub fn set_selection_tween(&mut self, tween: Tween) {
        let (Some(selected), Some(scene)) = (self.selected, self.scene_id()) else {
            return;
        };
        match ops::set_tween(self.document(), scene, selected, tween) {
            Some(commands) => self.run_batch(commands, "change the tween"),
            None => self.report("Select an instance to change its motion tween"),
        }
    }
}
