//! Moving, aligning, transforming and converting the selection.

use egui::{Color32, Pos2, Rect, Vec2};
use flare_commands::Command;
use flare_core::{LayerId, SceneId};
use flare_document::Content;
use flare_tools::ops::{self, SpanRef};
use glamx::{DAffine2, DVec2};

use super::*;

impl Workspace {
    pub(super) fn move_selection(&mut self, delta: Vec2) {
        let Some(scene) = self.scene_id() else {
            return;
        };
        let delta = DVec2::new(f64::from(delta.x), f64::from(delta.y));
        let commands: Vec<Command> = self
            .selection_all()
            .into_iter()
            .filter_map(|span| ops::translate(self.document(), scene, span, delta))
            .flatten()
            .collect();
        if !commands.is_empty() {
            self.run_batch(commands, "move");
        }
    }

    /// Moves the selection so its shapes line up on one another's extent, or on the stage when
    /// "Align to stage" is on, as one undo step.
    pub fn align_selection(&mut self, how: ops::Align) {
        let Some((scene, spans, rects)) = self.selection_with_bounds() else {
            return;
        };
        let target = if self.align_to_stage {
            Rect::from_min_size(Pos2::ZERO, self.stage_extent())
        } else {
            rects
                .iter()
                .copied()
                .reduce(|a, b| a.union(b))
                .unwrap_or(Rect::NOTHING)
        };
        let deltas = ops::align_deltas(&rects, how, target);
        self.move_each(scene, &spans, &deltas);
    }

    /// Spaces three or more selected shapes evenly along an axis.
    pub fn distribute_selection(&mut self, horizontal: bool) {
        let Some((scene, spans, rects)) = self.selection_with_bounds() else {
            return;
        };
        if spans.len() < 3 {
            self.report("Distributing needs at least three shapes selected");
            return;
        }
        let deltas = ops::distribute_deltas(&rects, horizontal);
        self.move_each(scene, &spans, &deltas);
    }

    /// Selected spans that have bounds, with those bounds, in selection order.
    pub(super) fn selection_with_bounds(&self) -> Option<(SceneId, Vec<SpanRef>, Vec<Rect>)> {
        let scene = self.scene_id()?;
        let (spans, rects): (Vec<_>, Vec<_>) = self
            .selection_all()
            .into_iter()
            .filter_map(|at| Some((at, ops::span_bounds(self.document(), scene, at)?)))
            .unzip();
        (!spans.is_empty()).then_some((scene, spans, rects))
    }

    pub(super) fn move_each(&mut self, scene: SceneId, spans: &[SpanRef], deltas: &[Vec2]) {
        let commands: Vec<Command> = spans
            .iter()
            .zip(deltas)
            .filter_map(|(at, d)| {
                ops::translate(
                    self.document(),
                    scene,
                    *at,
                    DVec2::new(f64::from(d.x), f64::from(d.y)),
                )
            })
            .flatten()
            .collect();
        if !commands.is_empty() {
            self.run_batch(commands, "align");
        }
    }

    /// Moves the selected shapes' layers to the top (`front`) or the bottom of the stack,
    /// keeping their order among themselves, as one undo step.
    pub fn arrange_selection(&mut self, front: bool) {
        let Some(scene) = self.scene_id() else { return };
        let Some(timeline) = self.document().scenes().first().map(|s| s.timeline()) else {
            return;
        };
        let count = timeline.layers().len();
        let mut positions: Vec<(usize, LayerId)> = self
            .selection_all()
            .iter()
            .filter_map(|at| {
                let p = timeline.layers().iter().position(|l| l.id() == at.layer)?;
                Some((p, at.layer))
            })
            .collect();
        positions.sort();
        positions.dedup();
        if front {
            positions.reverse();
        }
        let commands: Vec<Command> = positions
            .into_iter()
            .map(|(_, layer)| Command::MoveLayer {
                scene,
                layer,
                index: if front { 0 } else { count.saturating_sub(1) },
            })
            .collect();
        if !commands.is_empty() {
            self.run_batch(commands, "arrange");
        }
    }

    /// Applies the Transform panel's numbers to the selection about the center of its bounds
    /// and resets them, as one undo step.
    pub fn apply_transform_edit(&mut self) {
        let Some(bounds) = self.selection_bounds() else {
            return;
        };
        if let Some(matrix) = self.transform_edit.matrix(bounds.center()) {
            self.apply_to_selection(matrix);
        }
        self.transform_edit = TransformEdit::default();
    }

    /// Moves and resizes the selection so its bounds become `target`, as one undo step.
    pub fn set_selection_bounds(&mut self, target: Rect) {
        let Some(bounds) = self.selection_bounds() else {
            return;
        };
        if bounds.width() < 1e-3
            || bounds.height() < 1e-3
            || target.width() < 1.0
            || target.height() < 1.0
        {
            return;
        }
        let scale = DVec2::new(
            f64::from(target.width() / bounds.width()),
            f64::from(target.height() / bounds.height()),
        );
        let (from, to) = (
            DVec2::new(f64::from(bounds.min.x), f64::from(bounds.min.y)),
            DVec2::new(f64::from(target.min.x), f64::from(target.min.y)),
        );
        let matrix = DAffine2::from_translation(to)
            * DAffine2::from_scale(scale)
            * DAffine2::from_translation(-from);
        if matrix != DAffine2::IDENTITY {
            self.apply_to_selection(matrix);
        }
    }

    /// Swaps the fill and stroke colors.
    pub fn swap_colors(&mut self) {
        std::mem::swap(&mut self.paint.fill, &mut self.paint.stroke_color);
    }

    /// Black stroke, white fill.
    pub fn default_colors(&mut self) {
        self.paint.stroke_color = Color32::BLACK;
        self.paint.fill = Color32::WHITE;
    }

    /// Applies `matrix` to every selected span about the center of the selection's bounds, as
    /// one undo step. The y axis points down, so a positive angle turns clockwise on screen.
    pub fn transform_selection(&mut self, matrix: DAffine2) {
        let Some(bounds) = self.selection_bounds() else {
            return;
        };
        let center = DVec2::new(f64::from(bounds.center().x), f64::from(bounds.center().y));
        let about =
            DAffine2::from_translation(center) * matrix * DAffine2::from_translation(-center);
        self.apply_to_selection(about);
    }

    /// Copies the selection onto a new layer, nudged so it can be seen, and selects the copy.
    pub fn duplicate_selection(&mut self) {
        let Some(scene) = self.scene_id() else {
            return;
        };
        let offset = DVec2::new(DUPLICATE_OFFSET, DUPLICATE_OFFSET);
        let mut commands = Vec::new();
        let mut copies = Vec::new();
        for span in self.selection_all() {
            if let Some((batch, copy)) = ops::duplicate(self.document(), scene, span, offset) {
                commands.extend(batch);
                copies.push(copy);
            }
        }
        if commands.is_empty() {
            return;
        }
        self.run_batch(commands, "duplicate");
        if self.status.is_none() {
            self.selected = copies.first().copied();
            self.also_selected = copies.iter().skip(1).copied().collect();
            self.current_layer = self.selected.map(|s| s.layer);
        }
    }

    pub fn can_convert_selection(&self) -> bool {
        match (self.selected, self.scene_id()) {
            (Some(selected), Some(scene)) => {
                ops::convert_to_symbol(self.document(), scene, selected).is_some()
            }
            _ => false,
        }
    }

    /// Convert to Symbol: the selected shape becomes a library symbol with one instance.
    pub fn convert_selection_to_symbol(&mut self) {
        let (Some(selected), Some(scene)) = (self.selected, self.scene_id()) else {
            return;
        };
        let Some((commands, symbol)) = ops::convert_to_symbol(self.document(), scene, selected)
        else {
            self.report("Select a shape to convert to a symbol");
            return;
        };
        self.run_batch(commands, "convert to symbol");
        if self.status.is_none() {
            self.selected_symbol = Some(symbol);
        }
    }

    /// Whether any selected span is a symbol instance.
    pub(super) fn selection_has_instance(&self) -> bool {
        let Some(scene) = self.scene_id() else {
            return false;
        };
        self.selection_all().into_iter().any(|at| {
            matches!(
                ops::find_span(self.document(), scene, at).map(|s| s.content()),
                Some(Content::Instance(_))
            )
        })
    }

    /// Groups the selected shapes into one symbol instance, as one undo step.
    pub(super) fn group_selection(&mut self) {
        let Some(scene) = self.scene_id() else { return };
        let selection = self.selection_all();
        let Some((commands, symbol)) = ops::group(self.document(), scene, &selection) else {
            self.report("Select two or more shapes to group");
            return;
        };
        self.run_batch(commands, "group");
        if self.status.is_none() {
            // The group takes the topmost shape's place, so that span stays selected.
            self.also_selected.clear();
            self.selected_symbol = Some(symbol);
        }
    }

    /// Replaces each selected instance with the artwork of its symbol, as one undo step, and
    /// selects the parts.
    pub(super) fn break_apart_selection(&mut self) {
        let Some(scene) = self.scene_id() else { return };
        let mut commands = Vec::new();
        let mut parts = Vec::new();
        for at in self.selection_all() {
            if let Some((batch, placed)) = ops::break_apart(self.document(), scene, at) {
                commands.extend(batch);
                parts.extend(placed);
            }
        }
        if commands.is_empty() {
            self.report("Select a symbol instance to break apart");
            return;
        }
        self.run_batch(commands, "break apart");
        if self.status.is_none() {
            self.selected = parts.first().copied();
            self.also_selected = parts.into_iter().skip(1).collect();
        }
    }
}
