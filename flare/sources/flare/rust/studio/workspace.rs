//! Workspace-only docking. Dragging a grip previews a destination; release
//! commits a preference change, while Escape/outside release leaves layout intact.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DockModule {
    Inspector,
    Tools,
    Timeline,
}

#[cfg(test)]
mod tests {
    use super::super::timeline_tests::drag;
    use super::*;
    use egui_kittest::{kittest::Queryable, Harness};

    #[test]
    fn grips_dock_modules_cancel_and_persist_without_document_edits() {
        for locale in [Locale::English, Locale::French, Locale::Expanded] {
            for theme in [theme::Theme::Light, theme::Theme::Dark] {
                let mut h = Harness::builder()
                    .with_size(vec2(1440.0, 920.0))
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
                let doc = h.state().editor.doc.clone();
                let inspector = i18n.text("dock-drag-properties");
                let tools = i18n.text("dock-drag-tools");
                let timeline = i18n.text("dock-drag-timeline");
                let start = h.get_by_label(&inspector).rect().center();
                drag(
                    &mut h,
                    start,
                    pos2(70.0, 460.0),
                    true,
                    egui::Modifiers::NONE,
                );
                assert_eq!(h.state().prefs.inspector_left, None);
                let start = h.get_by_label(&inspector).rect().center();
                drag(
                    &mut h,
                    start,
                    pos2(70.0, 460.0),
                    false,
                    egui::Modifiers::NONE,
                );
                assert_eq!(h.state().prefs.inspector_left, Some(true));
                assert!(h.get_by_label(&inspector).rect().center().x < 450.0);
                let start = h.get_by_label(&tools).rect().center();
                drag(
                    &mut h,
                    start,
                    pos2(720.0, 460.0),
                    false,
                    egui::Modifiers::NONE,
                );
                assert_eq!(h.state().prefs.tools_left, None, "outside drop must cancel");
                let start = h.get_by_label(&tools).rect().center();
                drag(
                    &mut h,
                    start,
                    pos2(70.0, 460.0),
                    false,
                    egui::Modifiers::NONE,
                );
                assert_eq!(h.state().prefs.tools_left, Some(true));
                assert!(h.get_by_label(&tools).rect().center().x < 50.0);
                let start = h.get_by_label(&timeline).rect().center();
                drag(
                    &mut h,
                    start,
                    pos2(720.0, 72.0),
                    false,
                    egui::Modifiers::NONE,
                );
                assert!(h.state().prefs.timeline_top);
                assert!(h.get_by_label(&timeline).rect().center().y < 100.0);
                let json = serde_json::to_string(&h.state().prefs).unwrap();
                let prefs: Preferences = serde_json::from_str(&json).unwrap();
                assert_eq!(prefs.inspector_left, Some(true));
                assert_eq!(prefs.tools_left, Some(true));
                assert!(prefs.timeline_top);
                let ctx = h.ctx.clone();
                h.state_mut().reset_layout(&ctx);
                h.run();
                assert_eq!(h.state().prefs.inspector_left, None);
                assert_eq!(h.state().prefs.tools_left, None);
                assert!(!h.state().prefs.timeline_top);
                assert_eq!(h.state().editor.doc, doc);
                assert_eq!(h.state().editor.undo_depth(), 0);
                assert!(!h.state().dirty());
            }
        }
    }
}

impl Studio {
    pub(super) fn inspector_on_left(&self) -> bool {
        self.prefs.inspector_left.unwrap_or(self.prefs.locale.rtl())
    }
    pub(super) fn tools_on_left(&self) -> bool {
        self.prefs.tools_left.unwrap_or(self.prefs.locale.rtl())
    }
    pub(super) fn workspace_grip(
        &mut self,
        ui: &mut egui::Ui,
        module: DockModule,
        size: egui::Vec2,
    ) {
        let (rect, response) = ui.allocate_exact_size(size, Sense::drag());
        let response = response.on_hover_cursor(egui::CursorIcon::Grab);
        let p = self.prefs.theme.palette();
        let pixel = 1.0 / ui.ctx().pixels_per_point();
        let painter = ui.painter_at(rect);
        let center = rect.center();
        let shades: &[u8] = if p.back.r() > 128 {
            &[122, 90, 193]
        } else {
            &[42, 24, 71]
        };
        for i in -4..4 {
            let x = center.x + i as f32 * 3.0 * pixel;
            let stripe = Rect::from_min_size(
                pos2(x, center.y - 3.5 * pixel),
                vec2(3.0 * pixel, 7.0 * pixel),
            );
            theme::edge_bands(&painter, stripe, true, false, shades);
        }
        let key = match module {
            DockModule::Inspector => "dock-drag-properties",
            DockModule::Tools => "dock-drag-tools",
            DockModule::Timeline => "dock-drag-timeline",
        };
        let label = self.i18n.text(key);
        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &label));
        if response.drag_started() && self.drag.is_none() && self.dock_drag.is_none() {
            self.dock_drag = Some(module);
        }
        response.on_hover_text(label);
    }

    pub(super) fn workspace_drop_targets(&mut self, ui: &mut egui::Ui, area: Rect) {
        let Some(module) = self.dock_drag else {
            return;
        };
        if ui.input(|i| i.key_pressed(egui::Key::Escape) || !i.focused) {
            self.dock_drag = None;
            return;
        }
        let center = area.center();
        let targets = if module == DockModule::Timeline {
            [
                (
                    true,
                    Rect::from_center_size(pos2(center.x, area.top() + 72.0), vec2(200.0, 40.0)),
                    "dock-top",
                ),
                (
                    false,
                    Rect::from_center_size(pos2(center.x, area.bottom() - 30.0), vec2(200.0, 40.0)),
                    "dock-bottom",
                ),
            ]
        } else {
            [
                (
                    true,
                    Rect::from_center_size(pos2(area.left() + 70.0, center.y), vec2(120.0, 70.0)),
                    "dock-left",
                ),
                (
                    false,
                    Rect::from_center_size(pos2(area.right() - 70.0, center.y), vec2(120.0, 70.0)),
                    "dock-right",
                ),
            ]
        };
        let pointer = ui.input(|i| i.pointer.interact_pos());
        let destination = targets
            .iter()
            .find(|(_, rect, _)| pointer.is_some_and(|pos| rect.contains(pos)))
            .map(|(side, _, _)| *side);
        let painter = ui.ctx().layer_painter(egui::LayerId::new(
            egui::Order::Tooltip,
            egui::Id::new("studio.dock.targets"),
        ));
        let p = self.prefs.theme.palette();
        for (side, rect, label) in targets {
            painter.rect_filled(
                rect,
                2.0,
                if destination == Some(side) {
                    p.select
                } else {
                    p.panel
                },
            );
            painter.rect_stroke(
                rect,
                2.0,
                Stroke::new(2.0, p.accent),
                egui::StrokeKind::Inside,
            );
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                self.i18n.text(label),
                theme::medium(theme::UI_TEXT_SIZE),
                p.text,
            );
        }
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        if ui.input(|i| i.pointer.any_released()) {
            if let Some(side) = destination {
                match module {
                    DockModule::Inspector => self.prefs.inspector_left = Some(side),
                    DockModule::Tools => self.prefs.tools_left = Some(side),
                    DockModule::Timeline => self.prefs.timeline_top = side,
                }
            }
            self.dock_drag = None;
            ui.ctx().request_repaint();
        }
    }
}
