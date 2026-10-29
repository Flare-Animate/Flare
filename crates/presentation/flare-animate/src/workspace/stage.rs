//! The stage panel.

use egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};
use flare_tools::handles::{self, Handle};
use glamx::DVec2;

use flare_input::{InputSample, PointerKind};

use super::*;

impl Workspace {
    pub(super) fn stage(&mut self, ui: &mut egui::Ui) {
        self.document_tabs(ui);
        self.stage_bar(ui);
        let available = ui.available_size();
        let (response, painter) = ui.allocate_painter(available, Sense::click_and_drag());
        painter.rect_filled(response.rect, 0.0, flare_ui::theme::STAGE_SURROUND);
        let extent = self.stage_extent();
        self.stage_panel = response.rect;
        if response.hovered() {
            // Pinch or Ctrl/Cmd+scroll zooms about the cursor; plain scroll and middle-drag pan.
            let (zoom, scroll, cursor) =
                ui.input(|i| (i.zoom_delta(), i.smooth_scroll_delta, i.pointer.hover_pos()));
            if zoom != 1.0 {
                self.stage_view.zoom_about(
                    cursor.unwrap_or(response.rect.center()),
                    zoom,
                    response.rect,
                    extent,
                );
            } else if scroll != Vec2::ZERO {
                self.stage_view.pan_by(scroll);
            }
        }
        if response.dragged_by(egui::PointerButton::Middle) {
            self.stage_view.pan_by(response.drag_delta());
        }
        let scale = self.stage_view.scale(response.rect, extent);
        let image_rect = self.stage_view.image_rect(response.rect, extent);
        if let Some(texture) = &self.texture {
            // Until the texture catches up with a zoom or pan it is drawn where it belongs, never
            // stretched over the stage; the stage color shows through the rest.
            let bg = self.document().stage().background().to_array();
            painter.rect_filled(
                image_rect,
                0.0,
                Color32::from_rgba_unmultiplied(bg[0], bg[1], bg[2], bg[3]),
            );
            let region = texture.view.region;
            let shown = Rect::from_min_max(
                image_rect.min + region.min.to_vec2() * scale,
                image_rect.min + region.max.to_vec2() * scale,
            );
            painter.image(
                texture.handle.id(),
                shown,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
            if self.desired_texture_view(ui.ctx().pixels_per_point(), texture_side_limit(ui.ctx()))
                != Some(texture.view)
            {
                ui.ctx().request_repaint();
            }
        } else {
            painter.text(
                image_rect.center(),
                Align2::CENTER_CENTER,
                "Nothing to show",
                FontId::proportional(16.0),
                Color32::GRAY,
            );
        }
        painter.rect_stroke(
            image_rect,
            0.0,
            Stroke::new(1.0, Color32::DARK_GRAY),
            StrokeKind::Outside,
        );

        if self.show_grid {
            self.stage_grid(&painter, response.rect, image_rect, extent, scale);
        }
        if self.show_rulers {
            self.stage_rulers(&painter, response.rect, image_rect, scale);
        }
        self.stage_guides(ui, response.rect, image_rect, scale);
        self.text_editor(ui.ctx(), image_rect, scale);
        if !self.stage_view.is_fit() {
            painter.text(
                response.rect.right_bottom() - Vec2::new(8.0, 8.0),
                Align2::RIGHT_BOTTOM,
                format!("{:.0}%", scale / fit_scale(response.rect, extent) * 100.0),
                FontId::proportional(12.0),
                Color32::GRAY,
            );
        }
        self.handle_radius = 8.0 / scale.max(f32::EPSILON);
        self.uniform_scale = ui.input(|i| i.modifiers.shift);
        self.skew_modifier = ui.input(|i| i.modifiers.alt);
        // Tools that place points snap them to the grid; tools that find things do not, or a
        // thin shape could not be clicked.
        let snap = self
            .snap_to_grid
            .then_some(self.grid_size)
            .filter(|_| tool_places_points(self.tool));
        let guides = if self.show_guides {
            self.guides.clone()
        } else {
            Vec::new()
        };
        let to_doc = |p: Pos2| {
            let at = screen_to_stage(p, image_rect, extent);
            snap.map_or(at, |grid| {
                let on_grid = snap_point(at, grid);
                guides::snap_to_guides(on_grid, &guides, 6.0 / scale.max(f32::EPSILON))
            })
        };
        self.pointer_stage = response.hover_pos().map(to_doc);
        let to_screen = |p: Pos2| image_rect.min + p.to_vec2() * scale;
        if self.tool == Tool::Text
            && response.clicked()
            && let Some(p) = response.interact_pointer_pos()
        {
            self.text_edit = Some(PendingText {
                at: to_doc(p),
                buffer: String::new(),
                focused: false,
            });
        }
        if self.tool == Tool::Hand && response.dragged_by(egui::PointerButton::Primary) {
            self.stage_view.pan_by(response.drag_delta());
        }
        if matches!(self.tool, Tool::Pencil | Tool::Brush) {
            let primary = egui::PointerButton::Primary;
            if let Some(p) = response.interact_pointer_pos()
                && response.dragged_by(primary)
            {
                let at = to_doc(p);
                // A pen reports how hard it presses; a mouse does not, and counts as full.
                let (pressure, kind) = ui
                    .input(|i| {
                        i.events.iter().rev().find_map(|e| match e {
                            egui::Event::Touch {
                                force: Some(force), ..
                            } => Some((*force, PointerKind::Pen)),
                            _ => None,
                        })
                    })
                    .unwrap_or((1.0, PointerKind::Mouse));
                let moved = self
                    .freehand
                    .last()
                    .is_none_or(|last| (last.x as f32, last.y as f32) != (at.x, at.y));
                if moved
                    && let Some(sample) =
                        InputSample::new(f64::from(at.x), f64::from(at.y), pressure, kind)
                {
                    self.freehand.push(sample);
                }
            }
            if response.drag_stopped_by(primary) {
                self.finish_freehand(scale);
            }
            if self.freehand.len() > 1 {
                let (color, width) = match self.tool {
                    Tool::Brush => (self.fill, self.brush_size),
                    _ => (self.stroke_color, self.stroke_width),
                };
                let points: Vec<Pos2> = self
                    .freehand
                    .iter()
                    .map(|s| to_screen(Pos2::new(s.x as f32, s.y as f32)))
                    .collect();
                painter.add(egui::Shape::line(points, Stroke::new(width * scale, color)));
            }
        }
        if self.tool == Tool::Lasso {
            let primary = egui::PointerButton::Primary;
            if let Some(p) = response.interact_pointer_pos()
                && response.dragged_by(primary)
            {
                let at = to_doc(p);
                let step = 2.0 / scale.max(f32::EPSILON);
                if self
                    .lasso
                    .last()
                    .is_none_or(|last| (*last - at).length() >= step)
                {
                    self.lasso.push(at);
                }
            }
            if response.drag_stopped_by(primary) {
                self.finish_lasso(ui.input(|i| i.modifiers.shift));
            }
            if self.lasso.len() > 1 {
                let mut points: Vec<Pos2> = self.lasso.iter().map(|p| to_screen(*p)).collect();
                points.push(points[0]);
                painter.add(egui::Shape::line(
                    points,
                    Stroke::new(1.5, flare_ui::theme::ACCENT),
                ));
            }
        }
        if self.tool == Tool::Zoom
            && response.clicked()
            && let Some(p) = response.interact_pointer_pos()
        {
            let factor = if ui.input(|i| i.modifiers.alt) {
                1.0 / 1.5
            } else {
                1.5
            };
            self.stage_view.zoom_about(p, factor, response.rect, extent);
        }
        if response.clicked()
            && let Some(p) = response.interact_pointer_pos()
        {
            let shift = ui.input(|i| i.modifiers.shift);
            self.click_with_tool(to_doc(p), shift);
        }
        if self.tool == Tool::Pen {
            // A press that turns into a drag starts where the button went down, not where
            // egui noticed the drag; a plain click adds a corner point.
            let origin = ui.input(|i| i.pointer.press_origin());
            let here = response.interact_pointer_pos();
            if response.drag_started_by(egui::PointerButton::Primary)
                && let Some(p) = origin.or(here)
            {
                self.pen_press(to_doc(p));
            }
            if response.dragged_by(egui::PointerButton::Primary)
                && let Some(p) = here
            {
                self.pen_drag(to_doc(p));
            }
            if response.drag_stopped_by(egui::PointerButton::Primary) {
                self.pen_release();
            }
            if response.double_clicked() {
                self.pen_finish();
            } else if response.clicked()
                && let Some(p) = here
            {
                self.pen_press(to_doc(p));
                self.pen_release();
            }
        }
        let additive = ui.input(|i| i.modifiers.shift);
        let primary = egui::PointerButton::Primary;
        if response.drag_started_by(primary)
            && let Some(p) = response.interact_pointer_pos()
        {
            self.begin_drag_with(to_doc(p), additive);
        }
        if response.dragged_by(primary)
            && let Some(p) = response.interact_pointer_pos()
        {
            self.update_drag(to_doc(p));
        }
        if response.drag_stopped_by(primary) {
            self.end_drag();
        }
        // A plain click with the select tool also selects (or clears the selection).
        if response.clicked()
            && matches!(
                self.tool,
                Tool::Select | Tool::Subselection | Tool::FreeTransform
            )
            && let Some(p) = response.interact_pointer_pos()
        {
            self.select_at(to_doc(p), additive);
        }

        // Double-click an instance to edit its symbol; double-click empty stage to go back.
        if response.double_clicked()
            && self.tool == Tool::Select
            && let Some(p) = response.interact_pointer_pos()
        {
            self.select_at(to_doc(p), false);
            match self.selected {
                Some(_) => {
                    if let Some(symbol) = self.selected_instance_symbol() {
                        self.enter_symbol(symbol);
                    }
                }
                None => {
                    self.exit_symbol();
                }
            }
        }

        // Motion guides are never drawn into the animation, but their paths show on the stage
        // so the path a layer follows can be seen and edited.
        if let Some(scene) = self.document().scenes().first() {
            let ink = Stroke::new(1.5, Color32::from_rgb(230, 80, 200));
            for layer in scene
                .timeline()
                .layers()
                .iter()
                .filter(|l| l.is_guide() && l.is_visible())
            {
                if let Some(span) = layer.span_at(self.frame)
                    && let flare_document::Content::Shape(shape) = span.content()
                {
                    for line in flare_eval::flatten(shape.path()) {
                        let points: Vec<Pos2> = line
                            .iter()
                            .map(|p| to_screen(Pos2::new(p.x as f32, p.y as f32)))
                            .collect();
                        painter.add(egui::Shape::line(points, ink));
                    }
                }
            }
        }
        // The pen path being built: the curve so far, its points, and the dragged handles.
        if self.pen.is_active() {
            let ink = Stroke::new(1.5, Color32::from_rgb(40, 120, 255));
            let handle = Stroke::new(1.0, Color32::from_rgb(120, 120, 120));
            let points = self.pen.points();
            for pair in points.windows(2) {
                let (a, b) = (pair[0], pair[1]);
                let c1 = a.out.unwrap_or(a.anchor);
                let c2 = b.out.map_or(b.anchor, |o| b.anchor + (b.anchor - o));
                let steps = 20;
                let sample = |t: f32| {
                    let u = 1.0 - t;
                    to_screen(Pos2::new(
                        u * u * u * a.anchor.x
                            + 3.0 * u * u * t * c1.x
                            + 3.0 * u * t * t * c2.x
                            + t * t * t * b.anchor.x,
                        u * u * u * a.anchor.y
                            + 3.0 * u * u * t * c1.y
                            + 3.0 * u * t * t * c2.y
                            + t * t * t * b.anchor.y,
                    ))
                };
                let line: Vec<Pos2> = (0..=steps)
                    .map(|i| sample(i as f32 / steps as f32))
                    .collect();
                painter.add(egui::Shape::line(line, ink));
            }
            for point in points {
                if let Some(out) = point.out {
                    let mirror = point.anchor + (point.anchor - out);
                    painter.line_segment([to_screen(mirror), to_screen(out)], handle);
                    painter.circle_filled(to_screen(out), 3.0, Color32::from_rgb(120, 120, 120));
                }
                let at = to_screen(point.anchor);
                painter.rect_filled(
                    Rect::from_center_size(at, Vec2::splat(7.0)),
                    0.0,
                    Color32::WHITE,
                );
                painter.rect_stroke(
                    Rect::from_center_size(at, Vec2::splat(7.0)),
                    0.0,
                    ink,
                    StrokeKind::Outside,
                );
            }
        }

        let outline = Stroke::new(1.5, Color32::from_rgb(40, 120, 255));
        let moving = self
            .drag
            .filter(|d| d.mode == DragMode::Move)
            .map_or(Vec2::ZERO, |d| d.current - d.start);
        for bounds in self.selection_rects() {
            let shown = bounds.translate(moving);
            let rect = Rect::from_min_max(to_screen(shown.min), to_screen(shown.max));
            painter.rect_stroke(rect, 0.0, outline, StrokeKind::Outside);
        }
        if self.tool == Tool::Subselection {
            let ink = Stroke::new(1.5, crate::workspace::ANCHOR_INK);
            for (segment, point) in self.anchor_points() {
                let at = self
                    .anchor_drag
                    .filter(|d| d.segment == segment)
                    .map_or(point, |d| d.to);
                let centre = to_screen(at);
                let square = Rect::from_center_size(centre, Vec2::splat(8.0));
                painter.rect_filled(square, 1.0, Color32::WHITE);
                painter.rect_stroke(square, 1.0, ink, StrokeKind::Outside);
            }
        }
        if self.tool == Tool::FreeTransform
            && let Some(bounds) = self.selection_bounds()
        {
            // The handles follow a move in progress, or show the scaled result of a scale.
            let shown = match self.drag {
                Some(Drag {
                    mode: DragMode::Scale(handle),
                    bounds: grabbed,
                    current,
                    ..
                }) => handles::scale_for(grabbed, handle, current, self.uniform_scale).map_or(
                    grabbed,
                    |scale| {
                        let m = handles::matrix(grabbed, handle, scale);
                        let corner = |p: Pos2| {
                            let q = m.transform_point2(DVec2::new(f64::from(p.x), f64::from(p.y)));
                            Pos2::new(q.x as f32, q.y as f32)
                        };
                        Rect::from_two_pos(corner(grabbed.min), corner(grabbed.max))
                    },
                ),
                _ => bounds.translate(moving),
            };
            // A rotate or skew in progress shows the turned or sheared frame.
            if let Some(drag) = self.drag
                && let Some(m) = self.drag_matrix(&drag)
            {
                let corner = |p: Pos2| {
                    let q = m.transform_point2(DVec2::new(f64::from(p.x), f64::from(p.y)));
                    to_screen(Pos2::new(q.x as f32, q.y as f32))
                };
                let b = drag.bounds;
                let outline_points = vec![
                    corner(b.left_top()),
                    corner(b.right_top()),
                    corner(b.right_bottom()),
                    corner(b.left_bottom()),
                ];
                painter.add(egui::Shape::closed_line(outline_points, outline));
            }
            let frame = Rect::from_min_max(to_screen(shown.min), to_screen(shown.max));
            painter.rect_stroke(
                frame,
                0.0,
                Stroke::new(1.0, Color32::from_rgb(40, 120, 255)),
                StrokeKind::Outside,
            );
            for handle in Handle::ALL {
                let at = to_screen(handle.position(shown));
                let square = Rect::from_center_size(at, Vec2::splat(7.0));
                painter.rect_filled(square, 0.0, Color32::WHITE);
                painter.rect_stroke(square, 0.0, outline, StrokeKind::Outside);
            }
        }
        if let Some(drag) = self.drag.filter(|d| d.mode == DragMode::Draw) {
            let (from, to) = (to_screen(drag.start), to_screen(drag.current));
            let preview = Rect::from_two_pos(from, to);
            let ink = Stroke::new(1.0, Color32::BLACK);
            match self.tool {
                Tool::Line => {
                    painter.line_segment(
                        [from, to],
                        Stroke::new(self.stroke_width * scale, self.stroke_color),
                    );
                }
                Tool::Ellipse => {
                    let radius = preview.size() / 2.0;
                    painter.add(egui::epaint::EllipseShape::filled(
                        preview.center(),
                        radius,
                        self.fill.gamma_multiply(0.5),
                    ));
                    painter.rect_stroke(preview, 0.0, ink, StrokeKind::Outside);
                }
                _ => {
                    painter.rect_filled(preview, 0.0, self.fill.gamma_multiply(0.5));
                    painter.rect_stroke(preview, 0.0, ink, StrokeKind::Outside);
                }
            }
        }
    }
}
