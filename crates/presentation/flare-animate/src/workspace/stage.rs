//! The stage panel.

use egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};
use flare_input::InputSample;
use flare_tools::free_transform::handles::{self, Handle};
use glamx::DVec2;

use super::*;

impl Workspace {
    pub(super) fn stage(&mut self, ui: &mut egui::Ui) {
        self.document_tabs(ui);
        self.stage_bar(ui);
        let available = ui.available_size();
        let (response, painter) = ui.allocate_painter(available, Sense::click_and_drag());
        painter.rect_filled(response.rect, 0.0, flare_ui::theme::stage_surround());
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
        // The stage is painted upright and turned afterwards, all of it at once; the rulers and
        // the pointer stay as the screen has them.
        let layer = painter.layer_id();
        let turned_from = flare_ui::view::turn::painted_so_far(ui.ctx(), layer);
        self.stage_body(ui, &response, &painter, extent);
        flare_ui::view::turn::turn_painted(
            ui.ctx(),
            layer,
            turned_from,
            response.rect.center(),
            self.stage_view.rotation(),
        );
        if self.display.show_rulers {
            let scale = self.stage_view.scale(response.rect, extent);
            let image_rect = self.stage_view.image_rect(response.rect, extent);
            self.stage_rulers(&painter, response.rect, image_rect, scale);
        }
        self.rotation_dial(ui, &response);
        self.transform_pointer(ui, &response);
        self.tool_pointer(ui, &response);
    }

    /// Everything on the stage that turns with it: the picture, the furniture over it, and what
    /// the tools do and draw. Points come in from the screen through the view, so a tool never
    /// sees the turn.
    fn stage_body(
        &mut self,
        ui: &mut egui::Ui,
        response: &egui::Response,
        painter: &egui::Painter,
        extent: Vec2,
    ) {
        let response = response.clone();
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

        if self.display.show_grid {
            let reach = self.stage_view.upright_reach(response.rect);
            self.stage_grid(painter, reach, image_rect, extent, scale);
        }
        self.stage_guides(ui, response.rect, image_rect, scale);
        let reach = self.stage_view.upright_reach(response.rect);
        self.assist_overlay(painter, reach, image_rect, scale);
        self.text_editor(ui.ctx(), image_rect, scale);
        self.handle_radius = 8.0 / scale.max(f32::EPSILON);
        self.uniform_scale = ui.input(|i| i.modifiers.shift);
        self.skew_modifier = ui.input(|i| i.modifiers.alt);
        // Tools that place points snap them to the grid; tools that find things do not, or a
        // thin shape could not be clicked.
        let places = tool_places_points(self.tool);
        let snap = self
            .display
            .snap_to_grid
            .then_some(self.display.grid_size)
            .filter(|_| places);
        let guides = if self.display.show_guides && places {
            self.display.guides.clone()
        } else {
            Vec::new()
        };
        // Guides pull points to them whether or not the grid does.
        let (view, panel) = (self.stage_view, response.rect);
        let to_doc = |p: Pos2| {
            let at = screen_to_stage(&view, panel, extent, p);
            guides::snap_for_drawing(at, snap, &guides, 6.0 / scale.max(f32::EPSILON))
        };
        self.pointer_stage = response.hover_pos().map(to_doc);
        let to_screen = |p: Pos2| image_rect.min + p.to_vec2() * scale;
        // A second button tapped while a gesture is held is not a request for the menu.
        if self.drag.is_none() && self.anchor_drag.is_none() {
            if response.secondary_clicked()
                && let Some(p) = response.interact_pointer_pos()
            {
                self.pick_for_stage_menu(to_doc(p));
            }
            response.context_menu(|ui| self.stage_menu(ui));
        }
        if self.camera_stage(&response, painter, scale, &to_screen)
            || self.bone_stage(&response, painter, &to_screen, &to_doc)
        {
            return;
        }
        if self.tool == Tool::Text
            && response.clicked()
            && let Some(p) = response.interact_pointer_pos()
        {
            self.text_click(to_doc(p));
        }
        self.rotate_view_stage(ui, &response);
        if self.tool == Tool::Hand && response.dragged_by(egui::PointerButton::Primary) {
            self.stage_view.pan_by(response.drag_delta());
        }
        if matches!(self.tool, Tool::Ruler | Tool::Mirror) {
            let primary = egui::PointerButton::Primary;
            let origin = ui.input(|i| i.pointer.press_origin());
            if let Some(p) = response.interact_pointer_pos()
                && (response.dragged_by(primary) || response.clicked())
            {
                self.assist_drag(origin.map_or_else(|| to_doc(p), to_doc), to_doc(p));
            }
        }
        if matches!(
            self.tool,
            Tool::Pencil | Tool::Brush | Tool::Blur | Tool::Smudge
        ) || self.eraser_rubs()
        {
            let primary = egui::PointerButton::Primary;
            self.paint.straight_held = ui.input(|i| i.modifiers.shift);
            if let Some(p) = response.interact_pointer_pos()
                && response.dragged_by(primary)
            {
                let at = to_doc(p);
                // A pen reports how hard it presses; a mouse does not, and counts as full.
                let touch = ui.input(|i| {
                    i.events.iter().rev().find_map(|e| match e {
                        egui::Event::Touch {
                            force: Some(force), ..
                        } => Some(*force),
                        _ => None,
                    })
                });
                let (pressure, kind) = self.pointer_reading(touch);
                if let Some(sample) =
                    InputSample::new(f64::from(at.x), f64::from(at.y), pressure, kind)
                {
                    let sample = self.steady_freehand(sample, scale);
                    self.push_freehand(sample, scale);
                    self.erase_as_dragged();
                    self.blend_as_dragged();
                }
            }
            if response.drag_stopped_by(primary) {
                self.finish_freehand(scale);
                // The overlay above has already been drawn this frame, so the stroke that was just
                // committed would be missing until the next one: a blink on release.
                self.draw_settling(painter, to_screen, scale);
            }
            if self.freehand.len() > 1 {
                let strokes = self.freehand_preview(scale);
                let pixel = 1.0 / painter.pixels_per_point();
                painter.add(egui::Shape::mesh(stroke_preview::mesh(
                    &strokes, to_screen, pixel,
                )));
            }
        }
        if self.tool == Tool::Lasso {
            let primary = egui::PointerButton::Primary;
            if let Some(p) = response.interact_pointer_pos()
                && response.dragged_by(primary)
            {
                flare_tools::lasso::freehand::extend(&mut self.lasso, to_doc(p), scale);
            }
            if response.drag_stopped_by(primary) {
                self.finish_lasso(ui.input(|i| i.modifiers.shift));
            }
            if self.lasso.len() > 1 {
                let mut points: Vec<Pos2> = self.lasso.iter().map(|p| to_screen(*p)).collect();
                points.push(points[0]);
                painter.add(egui::Shape::line(
                    points,
                    Stroke::new(1.5, flare_ui::theme::accent()),
                ));
            }
        }
        if self.tool == Tool::PolygonLasso {
            let additive = ui.input(|i| i.modifiers.shift);
            if response.double_clicked() {
                self.polygon_lasso_finish(additive);
            } else if response.clicked()
                && let Some(p) = response.interact_pointer_pos()
            {
                self.polygon_lasso_click(to_doc(p), scale, additive);
            }
            self.draw_polygon_lasso(painter, to_screen);
        }
        if self.tool == Tool::MagicWand {
            if response.clicked()
                && let Some(p) = response.interact_pointer_pos()
            {
                let (shift, alt) = ui.input(|i| (i.modifiers.shift, i.modifiers.alt));
                self.wand_click(to_doc(p), shift, alt);
            }
            self.draw_wand(painter, to_screen);
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
        if tool_acts_on_press(self.tool) {
            self.paint_tool_press(ui, &response, &to_doc);
        } else if response.clicked()
            && let Some(p) = response.interact_pointer_pos()
        {
            let shift = ui.input(|i| i.modifiers.shift);
            self.click_with_tool(to_doc(p), shift);
            self.anchor_click(to_doc(p));
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
        // egui ends a drag when any button comes up, so a second button pressed and let go
        // mid-gesture (a two-finger tap on a trackpad) would strand the gesture: frozen where
        // it was, and never committed. A held gesture follows the primary button instead.
        let (primary_down, primary_up, pointer) = ui.input(|i| {
            let p = &i.pointer;
            (p.primary_down(), p.primary_released(), p.interact_pos())
        });
        let held = self.drag.is_some() || self.anchor_drag.is_some();
        if (response.dragged_by(primary) || (held && primary_down))
            && let Some(p) = response.interact_pointer_pos().or(pointer)
        {
            self.update_drag(to_doc(p));
            // The stage picture for where the pointer is now is asked for on the next pass,
            // which must come even if the pointer stops here.
            if self.drag.is_some_and(|d| d.mode != DragMode::Draw) {
                ui.ctx().request_repaint();
            }
        }
        if response.drag_stopped_by(primary) || (held && primary_up) {
            self.end_drag();
            // What a geometry tool just committed: this frame's overlay is already drawn, and
            // the drag preview is gone, so it would be missing until the next one.
            self.draw_settling(painter, to_screen, scale);
        }
        // A plain click with the select tool also selects (or clears the selection).
        if response.clicked()
            && matches!(
                self.tool,
                Tool::Select | Tool::Subselection | Tool::FreeTransform
            )
            && let Some(p) = response.interact_pointer_pos()
        {
            // With the Subselection tool a click on a point of the selected shape chooses that
            // point instead.
            if !(self.tool == Tool::Subselection && self.select_anchor_at(to_doc(p))) {
                self.selected_anchor = None;
                self.select_at(to_doc(p), additive);
            }
        }

        // Double-click an instance to edit its symbol; double-click empty stage to go back.
        if response.double_clicked()
            && self.tool == Tool::Select
            && let Some(p) = response.interact_pointer_pos()
        {
            self.select_at(to_doc(p), false);
            if self.edit_selected_text() {
                // Text opens in its editor.
            } else if self.selected.is_some() {
                if let Some(symbol) = self.selected_instance_symbol() {
                    self.enter_symbol(symbol);
                }
            } else {
                self.exit_symbol();
            }
        }

        self.stage_motion_guides(painter, to_screen);
        self.draw_pen_path(painter, &to_screen);

        let outline = Stroke::new(1.5, Color32::from_rgb(40, 120, 255));
        // The boxes move as the artwork does, snapped to the grid if it is.
        let moving = match self.drag.and_then(|d| self.drag_edit(&d)) {
            Some(drag_preview::DragEdit::Move(delta)) => delta,
            _ => Vec2::ZERO,
        };
        // While text is typed the box around it is the editor's, not the stored text's.
        let boxes = if self.text.edit.is_some() {
            Vec::new()
        } else {
            self.selection_rects()
        };
        for bounds in boxes {
            let shown = bounds.translate(moving);
            let rect = Rect::from_min_max(to_screen(shown.min), to_screen(shown.max));
            painter.rect_stroke(rect, 0.0, outline, StrokeKind::Outside);
        }
        if matches!(
            self.tool,
            Tool::Subselection | Tool::AddAnchor | Tool::DeleteAnchor | Tool::ConvertAnchor
        ) {
            let ink = Stroke::new(1.5, crate::workspace::ANCHOR_INK);
            for (segment, point) in self.anchor_points() {
                // Dragging a point with the Subselection tool moves it; with the Convert tool
                // it pulls a handle out of it, drawn below, and the point stays.
                let at = self
                    .anchor_drag
                    .filter(|d| d.segment == segment && self.tool == Tool::Subselection)
                    .map_or(point, |d| d.to);
                let centre = to_screen(at);
                let square = Rect::from_center_size(centre, Vec2::splat(8.0));
                let chosen = self.tool == Tool::Subselection
                    && self
                        .selected_anchor
                        .is_some_and(|(span, s, _)| s == segment && Some(span) == self.selected);
                painter.rect_filled(
                    square,
                    1.0,
                    if chosen {
                        crate::workspace::ANCHOR_INK
                    } else {
                        Color32::WHITE
                    },
                );
                painter.rect_stroke(square, 1.0, ink, StrokeKind::Outside);
            }
            // The handles being pulled out of a point: the one going on, and its mirror.
            if self.tool == Tool::ConvertAnchor
                && let Some(d) = self.anchor_drag
            {
                let (from, to) = (to_screen(d.from), to_screen(d.to));
                let mirror = from + (from - to);
                painter.line_segment([mirror, to], Stroke::new(1.0, crate::workspace::ANCHOR_INK));
                for end in [mirror, to] {
                    painter.circle(end, 3.5, Color32::WHITE, ink);
                }
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
        self.draw_drag_preview(painter, to_screen, scale);
    }
}
