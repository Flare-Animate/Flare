//! Moving, aligning, transforming and converting the selection.

use egui::{Color32, Pos2, Rect, Vec2};
use flare_commands::Command;
use flare_core::{LayerId, SceneId};
use flare_document::Content;
use flare_tools::ops::{self, SpanRef, Stack};
use glamx::{DAffine2, DVec2};

use super::*;

impl Workspace {
    pub(super) fn move_selection(&mut self, delta: Vec2) {
        let Some(scene) = self.scene_id() else {
            return;
        };
        let delta = DVec2::new(f64::from(delta.x), f64::from(delta.y));
        if let Some(commands) =
            ops::translate_many(self.document(), scene, &self.selection_all(), delta)
        {
            self.run_batch(commands, "move");
        }
    }

    /// The arrow keys nudge the selection one screen pixel per press, Shift makes it ten,
    /// and the distance on the stage follows the zoom, as issue #213 spells out: at 200%
    /// zoom one press moves half a stage unit, Shift half of ten times that.
    pub(super) fn nudge_selection(&mut self, dx: i32, dy: i32, coarse: bool) {
        if dx == 0 && dy == 0 {
            return;
        }
        let steps = if coarse { 10.0 } else { 1.0 };
        let scale = self.stage_scale();
        if !scale.is_finite() || scale <= 0.0 {
            return;
        }
        self.move_selection(Vec2::new(
            steps * (dx as f32) / scale,
            steps * (dy as f32) / scale,
        ));
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
            self.report(tr!(self, "msg-distribute-needs-three"));
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
        let commands = ops::each_element(self.document(), spans, |document, at| {
            let d = deltas.get(spans.iter().position(|s| *s == at)?)?;
            ops::translate(
                document,
                scene,
                at,
                DVec2::new(f64::from(d.x), f64::from(d.y)),
            )
        });
        if let Some(commands) = commands {
            self.run_batch(commands, "align");
        }
    }

    /// Brings the selection to the front or sends it to the back: among the other elements of
    /// its keyframe, and for a keyframe that holds only it, among the layers. One undo step.
    pub fn arrange_selection(&mut self, front: bool) {
        self.stack_selection(if front { Stack::Front } else { Stack::Back });
    }

    /// Moves the selected elements up or down the stacking order of their keyframes, and
    /// selects them where they land. Elements alone in their keyframe take their layer to the
    /// top or the bottom of the layers for Front and Back, as shapes on layers of their own did.
    pub fn stack_selection(&mut self, how: Stack) {
        let Some(scene) = self.scene_id() else { return };
        let Some(timeline) = self.document().scenes().first().map(|s| s.timeline()) else {
            return;
        };
        let count = timeline.layers().len();
        let (together, alone): (Vec<SpanRef>, Vec<SpanRef>) =
            self.selection_all().into_iter().partition(|at| {
                ops::find_span(self.document(), scene, *at).is_some_and(|s| s.element_count() > 1)
            });
        let mut commands = Vec::new();
        let mut landed = alone.clone();
        if let Some((batch, now)) = ops::arrange_elements(self.document(), scene, &together, how) {
            commands.extend(batch);
            landed = now.into_iter().chain(alone.iter().copied()).collect();
        } else {
            landed.extend(together.iter().copied());
        }
        let front = matches!(how, Stack::Front | Stack::Forward);
        let mut positions: Vec<(usize, LayerId)> = alone
            .iter()
            .filter(|_| matches!(how, Stack::Front | Stack::Back))
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
        commands.extend(positions.into_iter().map(|(_, layer)| Command::MoveLayer {
            scene,
            layer,
            index: if front { 0 } else { count.saturating_sub(1) },
        }));
        if commands.is_empty() {
            return;
        }
        self.run_batch(commands, "arrange");
        if self.status.is_none() {
            self.selected = landed.first().copied();
            self.also_selected = landed.into_iter().skip(1).collect();
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

    /// Empties the swatch that is selected (stroke or fill), so the next shape has none of it.
    pub fn no_color(&mut self) {
        match self.color_target {
            ColorTarget::Fill => self.paint.fill = Color32::TRANSPARENT,
            ColorTarget::Stroke => self.paint.stroke_color = Color32::TRANSPARENT,
            ColorTarget::GradientEnd => self.paint.gradient_end = Color32::TRANSPARENT,
        }
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

    /// Copies the selection into the same keyframes, nudged so it can be seen, and selects the copies.
    pub fn duplicate_selection(&mut self) {
        let Some(scene) = self.scene_id() else {
            return;
        };
        let offset = DVec2::new(DUPLICATE_OFFSET, DUPLICATE_OFFSET);
        let Some((commands, copies)) =
            ops::duplicate_many(self.document(), scene, &self.selection_all(), offset)
        else {
            return;
        };
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
                ops::convert_to_symbol_as(self.document(), scene, selected, self.library.new_kind)
                    .is_some()
            }
            _ => false,
        }
    }

    /// Convert to Symbol: the selected shape becomes a library symbol with one instance.
    pub fn convert_selection_to_symbol(&mut self) {
        let (Some(selected), Some(scene)) = (self.selected, self.scene_id()) else {
            return;
        };
        let Some((commands, symbol)) =
            ops::convert_to_symbol_as(self.document(), scene, selected, self.library.new_kind)
        else {
            self.report(tr!(self, "msg-select-shape-for-symbol"));
            return;
        };
        self.run_batch(commands, "convert to symbol");
        if self.status.is_none() {
            self.library.selected = Some(symbol);
        }
    }

    /// Whether any selected span is a symbol instance.
    /// Whether anything selected can be broken apart: a symbol instance, or text.
    pub(super) fn selection_can_break_apart(&self) -> bool {
        self.selection_has_instance()
            || self
                .selection_all()
                .into_iter()
                .any(|at| self.text_at(at).is_some())
    }

    pub(super) fn selection_has_instance(&self) -> bool {
        let Some(scene) = self.scene_id() else {
            return false;
        };
        self.selection_all().into_iter().any(|at| {
            matches!(
                ops::find_element(self.document(), scene, at),
                Some(Content::Instance(_))
            )
        })
    }

    /// Groups the selected shapes into one symbol instance, as one undo step.
    pub(super) fn group_selection(&mut self) {
        let Some(scene) = self.scene_id() else { return };
        let selection = self.selection_all();
        let Some((commands, symbol)) = ops::group(self.document(), scene, &selection) else {
            self.report(tr!(self, "msg-select-shapes-to-group"));
            return;
        };
        self.run_batch(commands, "group");
        if self.status.is_none() {
            // The group takes the topmost shape's place; select it there.
            self.also_selected.clear();
            self.selected = self.find_instance_of(scene, symbol).or(self.selected);
            self.library.selected = Some(symbol);
        }
    }

    /// The element at the current frame that is an instance of `symbol`, topmost layer first.
    fn find_instance_of(&self, scene: SceneId, symbol: flare_core::SymbolId) -> Option<SpanRef> {
        let timeline = self
            .document()
            .scenes()
            .iter()
            .find(|s| s.id() == scene)?
            .timeline();
        timeline.layers().iter().find_map(|layer| {
            let span = layer.span_at(self.frame)?;
            let element = span
                .elements()
                .position(|c| matches!(c, Content::Instance(i) if i.symbol() == symbol))?;
            Some(SpanRef {
                layer: layer.id(),
                start: span.start(),
                element,
            })
        })
    }

    /// Replaces each selected instance with the artwork of its symbol, as one undo step, and
    /// selects the parts.
    pub(super) fn break_apart_selection(&mut self) {
        let Some(scene) = self.scene_id() else { return };
        // Text becomes plain outlines in the same place, still selected.
        let Some((commands, parts)) =
            ops::break_apart_many(self.document(), scene, &self.selection_all())
        else {
            self.report(tr!(self, "msg-select-to-break-apart"));
            return;
        };
        self.run_batch(commands, "break apart");
        if self.status.is_none() {
            self.selected = parts.first().copied();
            self.also_selected = parts.into_iter().skip(1).collect();
        }
    }
}
