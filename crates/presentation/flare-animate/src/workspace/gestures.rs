//! Pointer gestures on the stage.

use egui::{Color32, Pos2, Rect};
use flare_commands::Command;
use flare_core::{Rgba8, ShapeId};
use flare_document::{Content, Shape, Stroke as ShapeStroke};
use flare_tools::handles::{self, Handle};
use flare_tools::ops::{self};
use glamx::DAffine2;

use flare_input::{InputSample, PressureCurve};

use super::*;

impl Workspace {
    #[cfg(test)]
    pub(super) fn begin_drag(&mut self, point: Pos2) {
        self.begin_drag_with(point, false);
    }

    pub(super) fn begin_drag_with(&mut self, point: Pos2, additive: bool) {
        self.drag_cancelled = false;
        let mode = match self.tool {
            Tool::Rectangle | Tool::Ellipse | Tool::Line | Tool::PolyStar => Some(DragMode::Draw),
            // The pen builds a path point by point; see `pen_press`.
            Tool::Pen | Tool::Text => None,
            Tool::Hand
            | Tool::Zoom
            | Tool::Eyedropper
            | Tool::Bucket
            | Tool::InkBottle
            | Tool::Eraser
            | Tool::Lasso
            | Tool::Pencil
            | Tool::Brush => None,
            Tool::Subselection => {
                self.begin_anchor_drag(point);
                None
            }
            Tool::Select => {
                let grabbed = if additive {
                    None
                } else {
                    self.selection_bounds()
                        .and_then(|b| handles::pick(b, point, self.handle_radius).map(|h| (h, b)))
                };
                if let Some((handle, bounds)) = grabbed {
                    // Alt turns an edge handle from scaling into skewing.
                    let mode = if self.skew_modifier && handle.is_edge() {
                        DragMode::Skew(handle)
                    } else {
                        DragMode::Scale(handle)
                    };
                    self.drag = Some(Drag {
                        bounds,
                        mode,
                        start: point,
                        current: point,
                    });
                    return;
                }
                if !additive
                    && let Some(bounds) = self.selection_bounds()
                    && handles::in_rotate_zone(bounds, point, self.handle_radius)
                {
                    self.drag = Some(Drag {
                        bounds,
                        mode: DragMode::Rotate,
                        start: point,
                        current: point,
                    });
                    return;
                }
                // Pressing on something already selected drags the whole selection.
                let on_selection = !additive
                    && self
                        .hit(point)
                        .is_some_and(|hit| self.selection_all().contains(&hit));
                if !on_selection {
                    self.select_at(point, additive);
                }
                // Shift only edits the selection; it never starts a move.
                (!additive && self.selected.is_some()).then_some(DragMode::Move)
            }
        };
        self.drag = mode.map(|mode| Drag {
            bounds: Rect::NOTHING,
            mode,
            start: point,
            current: point,
        });
    }

    /// A click with one of the tools that act on a single shape: it changes that shape's paint,
    /// takes its color, or deletes it, as one undo step.
    pub(super) fn click_with_tool(&mut self, point: Pos2, shift: bool) {
        if !matches!(
            self.tool,
            Tool::Eyedropper | Tool::Bucket | Tool::InkBottle | Tool::Eraser
        ) {
            return;
        }
        let (Some(scene), Some(at)) = (self.scene_id(), self.hit(point)) else {
            return;
        };
        let to_rgba = |c: Color32| {
            let c = c.to_array();
            Rgba8::new(c[0], c[1], c[2], c[3])
        };
        let commands = match self.tool {
            Tool::Eraser => Some(ops::delete(scene, at)),
            Tool::Bucket => {
                ops::restyle(self.document(), scene, at, Some(to_rgba(self.fill)), None)
            }
            Tool::InkBottle => {
                match ShapeStroke::new(f64::from(self.stroke_width), to_rgba(self.stroke_color)) {
                    Ok(stroke) => {
                        ops::restyle(self.document(), scene, at, None, Some(Some(stroke)))
                    }
                    Err(error) => return self.report(error.to_string()),
                }
            }
            _ => {
                let Some(Content::Shape(shape)) =
                    ops::find_span(self.document(), scene, at).map(|s| s.content().clone())
                else {
                    return;
                };
                let from = |c: Rgba8| Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a);
                match (shift, shape.stroke()) {
                    (true, Some(stroke)) => self.stroke_color = from(stroke.color()),
                    (true, None) => {}
                    (false, _) => self.fill = from(shape.flat_fill()),
                }
                None
            }
        };
        if let Some(commands) = commands {
            self.run_batch(commands, "paint");
        }
    }

    /// Commits the pencil or brush stroke as one undo step. The pencil is a smoothed open path in
    /// the stroke color and width; the brush is a filled outline in the fill color whose width
    /// follows the pressure of the pen.
    pub(super) fn finish_freehand(&mut self, scale: f32) {
        let samples = std::mem::take(&mut self.freehand);
        let tolerance = self.smoothing / scale.max(f32::EPSILON);
        let shape = match self.tool {
            Tool::Brush => self.brush_shape(&samples, f64::from(tolerance)),
            _ => self.pencil_shape(&samples, tolerance),
        };
        let name = if self.tool == Tool::Brush {
            "Brush stroke"
        } else {
            "Pencil line"
        };
        match shape {
            Ok(Some(shape)) => self.insert_shape_layer(shape, name),
            Ok(None) => {} // a click, not a drag
            Err(error) => self.report(error),
        }
    }

    fn pencil_shape(
        &self,
        samples: &[InputSample],
        tolerance: f32,
    ) -> Result<Option<Shape>, String> {
        let points: Vec<Pos2> = samples
            .iter()
            .map(|s| Pos2::new(s.x as f32, s.y as f32))
            .collect();
        let points = flare_tools::freehand::simplify(&points, tolerance);
        let Some(path) = flare_tools::freehand::smooth_path(&points) else {
            return Ok(None);
        };
        let c = self.stroke_color.to_array();
        let stroke = ShapeStroke::new(
            f64::from(self.stroke_width),
            Rgba8::new(c[0], c[1], c[2], c[3]),
        )
        .map_err(|e| e.to_string())?;
        Shape::new(ShapeId::new(), path, Rgba8::TRANSPARENT)
            .map(|shape| Some(shape.with_stroke(stroke)))
            .map_err(|e| e.to_string())
    }

    fn brush_shape(
        &self,
        samples: &[InputSample],
        tolerance: f64,
    ) -> Result<Option<Shape>, String> {
        let curve = PressureCurve::new(self.brush_light, 1.0);
        let Some(path) =
            flare_tools::brush::outline(samples, f64::from(self.brush_size), curve, tolerance)
        else {
            return Ok(None);
        };
        let c = self.fill.to_array();
        Shape::new(ShapeId::new(), path, Rgba8::new(c[0], c[1], c[2], c[3]))
            .map(Some)
            .map_err(|e| e.to_string())
    }

    /// Selects what the finished lasso loop encloses; with `additive` it adds to the selection.
    pub(super) fn finish_lasso(&mut self, additive: bool) {
        let polygon = std::mem::take(&mut self.lasso);
        let Some(scene) = self.scene_id() else { return };
        let found = ops::spans_in_polygon(self.document(), scene, self.frame, &polygon);
        let mut all = if additive {
            self.selection_all()
        } else {
            Vec::new()
        };
        for span in found {
            if !all.contains(&span) {
                all.push(span);
            }
        }
        self.selected = all.first().copied();
        self.also_selected = all.into_iter().skip(1).collect();
    }

    pub(super) fn update_drag(&mut self, point: Pos2) {
        self.update_anchor_drag(point);
        if let Some(drag) = &mut self.drag {
            drag.current = point;
        }
    }

    pub(super) fn end_drag(&mut self) {
        let cancelled = std::mem::take(&mut self.drag_cancelled);
        self.end_anchor_drag(cancelled);
        if let Some(drag) = self.drag.take()
            && !cancelled
        {
            match drag.mode {
                DragMode::Draw => self.commit_shape(self.tool, drag.start, drag.current),
                DragMode::Move => {
                    let mut delta = drag.current - drag.start;
                    if self.snap_to_grid {
                        delta = snap_point(delta.to_pos2(), self.grid_size).to_vec2();
                    }
                    self.move_selection(delta);
                }
                DragMode::Scale(handle) => {
                    self.scale_selection(handle, drag.bounds, drag.current);
                }
                DragMode::Rotate | DragMode::Skew(_) => {
                    if let Some(matrix) = self.drag_matrix(&drag) {
                        self.apply_to_selection(matrix);
                    }
                }
            }
        }
    }

    /// Escape, tool switches and focus loss cancel a gesture without committing it.
    pub(super) fn cancel_drag(&mut self) {
        self.span_drag = None;
        self.lasso.clear();
        self.freehand.clear();
        if self.anchor_drag.take().is_some() {
            self.drag_cancelled = true;
        }
        if self.drag.take().is_some() {
            self.drag_cancelled = true;
        }
    }

    /// The transform a rotate or skew drag stands for right now, if it stands for one.
    pub(super) fn drag_matrix(&self, drag: &Drag) -> Option<DAffine2> {
        match drag.mode {
            DragMode::Rotate => {
                let center = drag.bounds.center();
                let angle = handles::rotation_between(
                    center,
                    drag.start,
                    drag.current,
                    self.uniform_scale,
                )?;
                (angle != 0.0).then(|| handles::rotation_matrix(center, angle))
            }
            DragMode::Skew(handle) => {
                let k = handles::skew_for(drag.bounds, handle, drag.current)?;
                (k != 0.0).then(|| handles::skew_matrix(drag.bounds, handle, k))
            }
            DragMode::Draw | DragMode::Move | DragMode::Scale(_) => None,
        }
    }

    /// Scales the selection so `handle` of `bounds` moves to `pointer`, as one undo step.
    pub(super) fn scale_selection(&mut self, handle: Handle, bounds: Rect, pointer: Pos2) {
        if let Some(scale) = handles::scale_for(bounds, handle, pointer, self.uniform_scale) {
            if scale == (1.0, 1.0) {
                return;
            }
            self.apply_to_selection(handles::matrix(bounds, handle, scale));
        }
    }

    /// Maps every selected span through `about` (stage coordinates), as one undo step.
    pub(super) fn apply_to_selection(&mut self, about: DAffine2) {
        let Some(scene) = self.scene_id() else {
            return;
        };
        let spans = self.selection_all();
        let commands: Vec<Command> = spans
            .into_iter()
            .filter_map(|span| ops::transform(self.document(), scene, span, about))
            .flatten()
            .collect();
        if commands.is_empty() {
            if about != DAffine2::IDENTITY {
                self.report(
                    "Nothing could be transformed (an instance in a motion tween cannot be skewed or mirrored)",
                );
            }
        } else {
            self.run_batch(commands, "transform");
        }
    }
}
