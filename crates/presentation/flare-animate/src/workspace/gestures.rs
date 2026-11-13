//! Pointer gestures on the stage.

use egui::{Pos2, Rect};
use flare_core::{Rgba8, ShapeId};
use flare_document::{Shape, Stroke as ShapeStroke};
use flare_tools::free_transform::{Grab, handles};
use flare_tools::ops::{self};
use glamx::DAffine2;

use flare_input::{InputSample, PointerKind};

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
            | Tool::RotateView
            | Tool::Zoom
            | Tool::Eyedropper
            | Tool::Bucket
            | Tool::InkBottle
            | Tool::Eraser
            | Tool::Lasso
            | Tool::PolygonLasso
            | Tool::MagicWand
            | Tool::Camera
            | Tool::Bone
            | Tool::Pencil
            | Tool::Brush
            | Tool::Ruler
            | Tool::Mirror
            | Tool::Blur
            | Tool::Smudge => None,
            Tool::Subselection | Tool::ConvertAnchor => {
                self.begin_anchor_drag(point);
                None
            }
            // These two act on a click; see `anchor_click`.
            Tool::AddAnchor | Tool::DeleteAnchor => None,
            Tool::Select | Tool::FreeTransform => {
                // Only the free transform tool has a frame to grab; the hover pointer asks the
                // same question, so it always shows what a press here does.
                let grabbed = if additive {
                    None
                } else {
                    self.transform_grab(point)
                };
                let mode = match grabbed {
                    Some((Grab::Scale(handle), _)) => Some(DragMode::Scale(handle)),
                    Some((Grab::Skew(handle), _)) => Some(DragMode::Skew(handle)),
                    Some((Grab::Rotate, _)) => Some(DragMode::Rotate),
                    Some((Grab::Move, _)) | None => None,
                };
                if let (Some(mode), Some((_, bounds))) = (mode, grabbed) {
                    self.drag = Some(Drag {
                        bounds,
                        mode,
                        start: point,
                        current: point,
                    });
                    return;
                }
                // Pressing on something already selected drags the whole selection; with Free
                // Transform so does pressing anywhere inside its frame, unless the press is on
                // another shape, which is picked instead.
                let inside_frame = matches!(grabbed, Some((Grab::Move, _)));
                let on_selection = !additive
                    && match self.hit(point) {
                        Some(hit) => self.selection_all().contains(&hit),
                        None => inside_frame,
                    };
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
        // A rubbing eraser rubs out a dot wherever it is clicked, over a shape or not.
        if self.eraser_rubs() {
            self.erase_dot(point);
            return;
        }
        if self.blend_kind().is_some() {
            return self.blend_dot(point);
        }
        match self.tool {
            Tool::Bucket | Tool::InkBottle => self.pour_paint(point),
            Tool::Eyedropper => self.take_paint(point, shift),
            Tool::Eraser => {
                if let (Some(scene), Some(at)) = (self.scene_id(), self.hit(point)) {
                    let commands = ops::delete(self.document(), scene, at);
                    self.run_batch(commands, "paint");
                }
            }
            _ => {}
        }
    }

    /// The pressure and kind of the device drawing right now. A native pen source (the only one
    /// that works where the window system hands the pen over as a mouse) wins over the force the
    /// window reported for a touch; with neither it is a mouse, which counts as full pressure.
    pub(super) fn pointer_reading(&self, touch_force: Option<f32>) -> (f32, PointerKind) {
        self.pen_feed
            .reading()
            .or(touch_force)
            .map_or((1.0, PointerKind::Mouse), |pressure| {
                (pressure, PointerKind::Pen)
            })
    }

    /// Commits the pencil or brush stroke as one undo step. The pencil is a smoothed open path in
    /// the stroke color and width; the brush is a filled outline in the fill color whose width
    /// follows the pressure of the pen. A straight edge reduces the stroke to a line, and
    /// symmetry adds its mirror images, all in the same step.
    pub(super) fn finish_freehand(&mut self, scale: f32) {
        self.catch_up_freehand(scale);
        let samples = std::mem::take(&mut self.freehand);
        if self.tool == Tool::Eraser {
            if self.eraser_rubs() && !samples.is_empty() {
                self.finish_erase(samples);
            }
            self.live_erase = None;
            return;
        }
        if self.blend_kind().is_some() {
            return self.blend_release(samples);
        }
        let strokes = self.stroke_set(samples);
        let tolerance = self.smoothing / scale.max(f32::EPSILON);
        let mut shapes = Vec::new();
        for stroke in &strokes {
            let shape = match self.tool {
                Tool::Brush => self.brush_shape(stroke, f64::from(tolerance)),
                _ => self.pencil_shape(stroke, tolerance),
            };
            match shape {
                Ok(Some(shape)) => shapes.push(shape),
                Ok(None) => {} // a click, not a drag
                Err(error) => return self.report(error),
            }
        }
        let name = self.tr(if self.tool == Tool::Brush {
            "msg-brush-stroke"
        } else {
            "msg-pencil-line"
        });
        let preview = if shapes.is_empty() {
            Vec::new()
        } else {
            self.previews_of(&strokes, scale)
        };
        self.draw_shapes(shapes, &name);
        if self.status.is_none() && !preview.is_empty() {
            self.settle(preview);
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
        let points = flare_tools::pencil::simplify(&points, tolerance);
        let Some(path) = flare_tools::pencil::smooth_path(&points) else {
            return Ok(None);
        };
        let c = self.paint.stroke_color.to_array();
        let stroke = ShapeStroke::new(
            f64::from(self.paint.stroke_width),
            Rgba8::new(c[0], c[1], c[2], c[3]),
        )
        .map_err(|e| e.to_string())?
        .with_cap(self.paint.stroke_cap)
        .with_join(self.paint.stroke_join)
        .with_style(self.paint.stroke_style);
        Shape::new(ShapeId::new(), path, Rgba8::TRANSPARENT)
            .map(|shape| Some(shape.with_stroke(stroke)))
            .map_err(|e| e.to_string())
    }

    fn brush_shape(
        &self,
        samples: &[InputSample],
        tolerance: f64,
    ) -> Result<Option<Shape>, String> {
        let Some(path) = self
            .brush_shelf
            .stroke(f64::from(self.brush_size))
            .outline(samples, tolerance)
        else {
            return Ok(None);
        };
        let c = self.paint.fill.to_array();
        // Flow is how opaque the paint is at the first touch, so a low flow commits the stroke
        // partly see-through.
        let alpha = (f32::from(c[3]) * self.brush_shelf.flow()) as u8;
        Shape::new(ShapeId::new(), path, Rgba8::new(c[0], c[1], c[2], alpha))
            .map(Some)
            .map_err(|e| e.to_string())
    }

    /// Selects what the finished lasso loop encloses; with `additive` it adds to the selection.
    pub(super) fn finish_lasso(&mut self, additive: bool) {
        let polygon = std::mem::take(&mut self.lasso);
        let Some(scene) = self.scene_id() else { return };
        let found = match self.multi_frame_range() {
            Some(r) => flare_tools::multi_frame::spans_in_polygon_range(
                self.document(),
                scene,
                r.from(),
                r.to(),
                &polygon,
            ),
            None => ops::spans_in_polygon(self.document(), scene, self.frame, &polygon),
        };
        let all =
            flare_tools::lasso::freehand::selection_after(&self.selection_all(), found, additive);
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
            if drag.mode == DragMode::Draw {
                self.commit_shape(self.tool, drag.start, self.draw_end(&drag));
            } else if let Some(edit) = self.drag_edit(&drag) {
                // The same edit the stage was drawn with while the drag was held.
                self.commit_drag_edit(edit);
            }
        }
        self.drag_preview = None;
    }

    /// Where the shape being dragged out ends: at the pointer, except that Shift holds a line to
    /// the nearest of the eight directions 45 degrees apart. The preview and the commit both ask
    /// here, so what is let go is what was shown.
    pub(super) fn draw_end(&self, drag: &Drag) -> Pos2 {
        if self.tool == Tool::Line && self.uniform_scale {
            flare_tools::line::constrain(drag.start, drag.current)
        } else {
            drag.current
        }
    }

    /// Escape, tool switches and focus loss cancel a gesture without committing it.
    pub(super) fn cancel_drag(&mut self) {
        self.timeline_grab = None;
        self.bone_drag = None;
        self.lasso.clear();
        self.freehand.clear();
        self.stabilizer = None;
        self.cancel_erase();
        self.cancel_blend();
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

    /// Maps every selected span through `about` (stage coordinates), as one undo step.
    pub(super) fn apply_to_selection(&mut self, about: DAffine2) {
        let Some(scene) = self.scene_id() else {
            return;
        };
        match ops::transform_many(self.document(), scene, &self.selection_all(), about) {
            None => {
                if about != DAffine2::IDENTITY {
                    self.report(tr!(self, "msg-nothing-transformed"));
                }
            }
            Some(commands) => self.run_batch(commands, "transform"),
        }
    }
}
