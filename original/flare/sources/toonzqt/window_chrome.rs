//! Windows caption controls share the menu bar. Native commands retain OS
//! move/resize behavior; close always passes through the document's save guard.
use super::*;

impl Studio {
    pub(super) fn window_buttons(&mut self, ui: &mut egui::Ui) {
        if !cfg!(target_os = "windows") {
            return;
        }
        let maximized = ui.input(|i| i.viewport().maximized.unwrap_or(false));
        let p = self.prefs.theme.palette();
        let pixel = 1.0 / ui.ctx().pixels_per_point();
        // Reserve the complete cluster once: its internal seams do not inherit
        // menu item spacing. The native caption sits against the window top,
        // independent of the menu's vertically centered text baseline.
        let (reserved, _) = ui.allocate_exact_size(vec2(109.0, 20.0), Sense::hover());
        let mut right = ((reserved.right() - 6.0) / pixel).round() * pixel;
        let top = ui.ctx().content_rect().top() + pixel;
        for (kind, width, label) in [
            (0, 47.0, "window-close"),
            (
                1,
                28.0,
                if maximized {
                    "window-restore"
                } else {
                    "window-maximize"
                },
            ),
            (2, 28.0, "window-minimize"),
        ] {
            let left = ((right - width) / pixel).round() * pixel;
            let rect = Rect::from_min_max(pos2(left, top), pos2(right, top + 19.2));
            right = left;
            let saved_clip = ui.clip_rect();
            ui.set_clip_rect(rect);
            let response =
                ui.interact(rect, ui.id().with(("window.caption", kind)), Sense::click());
            ui.set_clip_rect(saved_clip);
            // Caption controls deliberately paint above the MenuBar child clip.
            // The hit rectangles and reserved menu width remain state-invariant.
            let mut painter = ui.painter().clone();
            painter.set_clip_rect(rect);
            let dark = p.panel.r() < 128;
            let pressed = response.is_pointer_button_down_on();
            let (face, edge, top_shoulder, highlight, bottom_shoulder, bottom, glyph_shadow) =
                if dark {
                    ([77, 54], 38, 82, 91, 46, 32, 37)
                } else {
                    ([199, 176], 120, 189, 215, 150, 102, 222)
                };
            let shift = if pressed {
                -12
            } else if response.hovered() {
                9
            } else {
                0
            };
            let shade = |value: u8| Color32::from_gray((value as i16 + shift).clamp(0, 255) as u8);
            painter.rect_filled(rect, 0.0, Color32::from_gray(edge));
            let body = Rect::from_min_max(
                rect.min + vec2(pixel, 2.0 * pixel),
                rect.max - vec2(pixel, 2.0 * pixel),
            );
            painter.add(theme::gradient(body, shade(face[0]), shade(face[1])));
            painter.rect_filled(
                Rect::from_min_max(rect.min, pos2(rect.left() + pixel, rect.bottom())),
                0.0,
                Color32::from_gray(edge),
            );
            painter.rect_filled(
                Rect::from_min_max(
                    pos2(rect.left() + pixel, top),
                    pos2(rect.right(), top + pixel),
                ),
                0.0,
                Color32::from_gray(top_shoulder),
            );
            painter.rect_filled(
                Rect::from_min_max(
                    pos2(rect.left() + pixel, top + pixel),
                    pos2(rect.right(), top + 2.0 * pixel),
                ),
                0.0,
                Color32::from_gray(highlight),
            );
            painter.rect_filled(
                Rect::from_min_max(
                    pos2(rect.left() + pixel, rect.bottom() - 2.0 * pixel),
                    pos2(rect.right(), rect.bottom() - pixel),
                ),
                0.0,
                Color32::from_gray(bottom_shoulder),
            );
            painter.rect_filled(
                Rect::from_min_max(pos2(rect.left(), rect.bottom() - pixel), rect.max),
                0.0,
                Color32::from_gray(bottom),
            );
            if kind == 0 {
                painter.rect_filled(
                    Rect::from_min_max(pos2(rect.right() - pixel, top), rect.max),
                    0.0,
                    Color32::from_gray(edge),
                );
            }
            let center = pos2(
                (rect.center().x / pixel).round() * pixel,
                (rect.center().y / pixel).round() * pixel,
            );
            // Authored, pixel-aligned caption glyphs have the weight of a small
            // desktop caption instead of the thin large tool-icon strokes.
            let glyph = Rect::from_center_size(center, vec2(12.0 * pixel, 10.0 * pixel));
            for (offset, color) in [(pixel, Color32::from_gray(glyph_shadow)), (0.0, p.text)] {
                let r = glyph.translate(vec2(0.0, offset));
                match kind {
                    0 => {
                        let stroke = Stroke::new(2.8 * pixel, color);
                        painter.line_segment([r.left_top(), r.right_bottom()], stroke);
                        painter.line_segment([r.right_top(), r.left_bottom()], stroke);
                    }
                    1 => {
                        if maximized {
                            let back = r.shrink(pixel).translate(vec2(2.0 * pixel, -2.0 * pixel));
                            painter.line_segment(
                                [back.left_top(), back.right_top()],
                                Stroke::new(2.0 * pixel, color),
                            );
                            painter.line_segment(
                                [back.right_top(), back.right_bottom()],
                                Stroke::new(2.0 * pixel, color),
                            );
                        }
                        painter.rect_stroke(
                            r,
                            0.0,
                            Stroke::new(2.0 * pixel, color),
                            egui::StrokeKind::Inside,
                        );
                        painter.rect_filled(
                            Rect::from_min_max(r.min, pos2(r.right(), r.top() + 3.0 * pixel)),
                            0.0,
                            color,
                        );
                    }
                    _ => {
                        painter.rect_filled(
                            Rect::from_min_max(pos2(r.left(), r.bottom() - 3.0 * pixel), r.max),
                            0.0,
                            color,
                        );
                    }
                }
            }
            if response.has_focus() {
                painter.rect_stroke(
                    rect.shrink(pixel),
                    0.0,
                    Stroke::new(pixel, p.accent),
                    egui::StrokeKind::Inside,
                );
            }
            let label = self.i18n.text(label);
            response
                .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &label));
            if response.clone().on_hover_text(label).clicked() {
                match kind {
                    0 => self.request(Pending::Quit, ui.ctx()),
                    1 => ui
                        .ctx()
                        .send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized)),
                    _ => ui
                        .ctx()
                        .send_viewport_cmd(egui::ViewportCommand::Minimized(true)),
                }
            }
        }
    }

    pub(super) fn window_drag_area(&self, ui: &mut egui::Ui) {
        if !cfg!(target_os = "windows") {
            return;
        }
        let (_, response) = ui.allocate_exact_size(
            vec2(ui.available_width().max(0.0), 28.0),
            Sense::click_and_drag(),
        );
        if response.double_clicked() {
            let maximized = ui.input(|i| i.viewport().maximized.unwrap_or(false));
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
        } else if response.drag_started() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
    }

    pub(super) fn window_resize_edges(&self, ui: &mut egui::Ui, rect: Rect) {
        if !cfg!(target_os = "windows")
            || ui.input(|i| {
                i.viewport().maximized.unwrap_or(false) || i.viewport().fullscreen.unwrap_or(false)
            })
        {
            return;
        }
        use egui::{CursorIcon as C, ResizeDirection as D};
        let edge = 4.0;
        let corner = 8.0;
        // Fixed hit regions, independent of hover. Corners are registered last.
        let regions = [
            (
                Rect::from_min_max(rect.min, pos2(rect.right(), rect.top() + edge)),
                D::North,
                C::ResizeVertical,
            ),
            (
                Rect::from_min_max(pos2(rect.left(), rect.bottom() - edge), rect.max),
                D::South,
                C::ResizeVertical,
            ),
            (
                Rect::from_min_max(rect.min, pos2(rect.left() + edge, rect.bottom())),
                D::West,
                C::ResizeHorizontal,
            ),
            (
                Rect::from_min_max(pos2(rect.right() - edge, rect.top()), rect.max),
                D::East,
                C::ResizeHorizontal,
            ),
            (
                Rect::from_min_size(rect.min, vec2(corner, corner)),
                D::NorthWest,
                C::ResizeNwSe,
            ),
            (
                Rect::from_min_size(
                    pos2(rect.right() - corner, rect.top()),
                    vec2(corner, corner),
                ),
                D::NorthEast,
                C::ResizeNeSw,
            ),
            (
                Rect::from_min_size(
                    pos2(rect.left(), rect.bottom() - corner),
                    vec2(corner, corner),
                ),
                D::SouthWest,
                C::ResizeNeSw,
            ),
            (
                Rect::from_min_size(rect.max - vec2(corner, corner), vec2(corner, corner)),
                D::SouthEast,
                C::ResizeNwSe,
            ),
        ];
        for (index, (rect, direction, cursor)) in regions.into_iter().enumerate() {
            let response = ui
                .interact(rect, ui.id().with(("window.resize", index)), Sense::drag())
                .on_hover_cursor(cursor);
            if response.drag_started() {
                ui.ctx()
                    .send_viewport_cmd(egui::ViewportCommand::BeginResize(direction));
            }
        }
    }
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;
    use egui_kittest::{
        kittest::{NodeT, Queryable},
        Harness,
    };

    #[test]
    fn caption_commands_do_not_edit_document() {
        use egui::ViewportCommand as V;
        let mut h = Harness::builder()
            .with_size(vec2(1200.0, 900.0))
            .build_eframe(|cc| Studio::with_preferences(&cc.egui_ctx, Preferences::default()));
        h.run();
        let original = h.state().editor.doc.clone();
        for (label, command) in [
            ("Minimize window", V::Minimized(true)),
            ("Maximize window", V::Maximized(true)),
        ] {
            h.get_by_label(label).click();
            h.step();
            assert!(h.output().viewport_output[&egui::ViewportId::ROOT]
                .commands
                .contains(&command));
        }
        h.input_mut()
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .unwrap()
            .maximized = Some(true);
        h.step();
        h.get_by_label("Restore window").click();
        h.step();
        assert!(h.output().viewport_output[&egui::ViewportId::ROOT]
            .commands
            .contains(&V::Maximized(false)));
        assert_eq!(h.state().editor.doc, original);
        assert_eq!(h.state().editor.undo_depth(), 0);
    }

    #[test]
    fn dock_collapse_restore_and_panel_menu_preserve_document() {
        for locale in [Locale::English, Locale::French] {
            let mut h = Harness::builder()
                .with_size(vec2(1440.0, 920.0))
                .build_eframe(|cc| {
                    Studio::with_preferences(
                        &cc.egui_ctx,
                        Preferences {
                            locale,
                            ..Default::default()
                        },
                    )
                });
            h.run();
            let i18n = Localizer::new(locale);
            let before = h.state().editor.doc.clone();
            h.get_by_label(&i18n.text("dock-hide-properties")).click();
            h.run();
            assert!(!h.state().prefs.inspector);
            h.get_by_label(&i18n.text("dock-toggle-properties")).click();
            h.run();
            assert!(h.state().prefs.inspector);
            h.get_by_label(&i18n.text("dock-toggle-tools")).click();
            h.run();
            assert!(h.state().prefs.tools_collapsed);
            h.hover_at(pos2(500.0, 350.0));
            h.run();
            h.get_by_label(&i18n.text("dock-toggle-tools")).click();
            h.run();
            assert!(!h.state().prefs.tools_collapsed);
            h.get_by_label(&i18n.text("dock-properties-menu")).click();
            h.run();
            h.query_all_by_label(&i18n.text("panel-library"))
                .find(|n| n.accesskit_node().role() == egui::accesskit::Role::Button)
                .unwrap()
                .click();
            h.run();
            assert!(!h.state().props_tab);
            assert_eq!(h.state().editor.doc, before);
            assert_eq!(h.state().editor.undo_depth(), 0);
        }
    }

    #[test]
    fn caption_controls_preserve_menu_geometry_and_guard_unsaved_close() {
        for locale in [Locale::English, Locale::French, Locale::Expanded] {
            for theme in [theme::Theme::Light, theme::Theme::Dark] {
                let mut h = Harness::builder()
                    .with_size(vec2(1000.0, 800.0))
                    .with_pixels_per_point(1.25)
                    .build_eframe(|cc| {
                        Studio::with_preferences(
                            &cc.egui_ctx,
                            Preferences {
                                locale,
                                theme,
                                ..Default::default()
                            },
                        )
                    });
                h.run();
                let i18n = Localizer::new(locale);
                let file = i18n.text("menu-file");
                let close = i18n.text("window-close");
                let file_rect = h.get_by_label(&file).rect();
                let close_rect = h
                    .query_all_by_label(&close)
                    .find(|node| node.accesskit_node().role() == egui::accesskit::Role::Button)
                    .unwrap()
                    .rect();
                assert!(file_rect.right() < close_rect.left());
                assert!(close_rect.right() <= 1000.0);
                let maximize_rect = h.get_by_label(&i18n.text("window-maximize")).rect();
                let minimize_rect = h.get_by_label(&i18n.text("window-minimize")).rect();
                assert!(close_rect.top() <= 1.0, "caption belongs to the window top");
                assert!((close_rect.height() - 19.2).abs() < 0.01);
                assert_eq!(minimize_rect.right(), maximize_rect.left());
                assert_eq!(maximize_rect.right(), close_rect.left());
                h.query_all_by_label(&close)
                    .find(|node| node.accesskit_node().role() == egui::accesskit::Role::Button)
                    .unwrap()
                    .hover();
                for _ in 0..3 {
                    h.step();
                    assert_eq!(h.get_by_label(&file).rect(), file_rect);
                    assert_eq!(
                        h.query_all_by_label(&close)
                            .find(|node| node.accesskit_node().role()
                                == egui::accesskit::Role::Button)
                            .unwrap()
                            .rect(),
                        close_rect
                    );
                }
                let ctx = h.ctx.clone();
                h.state_mut().dispatch(Command::AddLayer, &ctx);
                let edited = h.state().editor.doc.clone();
                h.query_all_by_label(&close)
                    .find(|node| node.accesskit_node().role() == egui::accesskit::Role::Button)
                    .unwrap()
                    .click();
                h.run();
                assert!(matches!(h.state().pending, Some(Pending::Quit)));
                assert!(!h.state().allow_close);
                h.get_by_label(&i18n.text("action-cancel")).click();
                h.run();
                assert!(h.state().pending.is_none());
                assert_eq!(h.state().editor.doc, edited);
                assert!(h.state().dirty());
            }
        }
    }
}
