//! The status bar, document tabs and the bar above the stage.

use crate::workspace::{Mode, Workspace};
use egui::{Align, Layout, RichText, Ui};
use flare_menubar::MenuAction;
use flare_ui::chrome;
use flare_ui::icons;
use flare_ui::theme;

use super::*;

/// Space between one document tab and the next, in points.
const TAB_GAP: f32 = 3.0;

impl Workspace {
    // ---- status bar --------------------------------------------------------------------

    pub(crate) fn status_bar(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            let message = match &self.status {
                Some(status) => RichText::new(status.as_str()).color(theme::warning()),
                None => RichText::new(self.tool_hint()).color(theme::text_dim()),
            };
            ui.label(message);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                // The zoom is in the bar above the stage; here is what the document is made of,
                // and a click opens its settings.
                let stage = self.document().stage();
                let rate = self.document().frame_rate();
                let fps = rate.per_second();
                let facts = format!("{} × {} px · {fps:.2} fps", stage.width(), stage.height())
                    .replace(".00 fps", " fps");
                if ui
                    .add(
                        egui::Button::new(RichText::new(facts).color(theme::text_dim()))
                            .frame(false),
                    )
                    .on_hover_text("Document settings")
                    .clicked()
                {
                    self.perform(MenuAction::DocumentSettings);
                }
                ui.separator();
                ui.label(
                    RichText::new(format!(
                        "Frame {} / {}",
                        self.frame + 1,
                        self.scene_frames()
                    ))
                    .color(theme::text_dim()),
                );
                ui.separator();
                let at = match self.pointer_stage {
                    Some(p) => format!("{:.0}, {:.0}", p.x, p.y),
                    None => "-, -".to_owned(),
                };
                ui.label(RichText::new(at).color(theme::text_dim()).monospace());
                ui.add(icons::image("tool-selection", 12.0, theme::text_disabled()));
                if self.background.mode() == Mode::Threads && self.has_pending_io() {
                    ui.separator();
                    ui.label(RichText::new("Working...").color(theme::accent()));
                }
            });
        });
    }

    // ---- document tabs -----------------------------------------------------------------

    /// The strip above the stage with the open document as a tab.
    pub(crate) fn document_tabs(&mut self, ui: &mut Ui) {
        let tabs = self.tabs();
        let (mut switch, mut close, mut add) = (None, None, false);
        ui.horizontal(|ui| {
            // Tabs sit close together; what is inside each keeps the usual spacing.
            let inner_gap = std::mem::replace(&mut ui.spacing_mut().item_spacing.x, TAB_GAP);
            for (index, tab) in tabs.iter().enumerate() {
                let fill = if tab.current {
                    theme::panel()
                } else {
                    theme::bg()
                };
                egui::Frame::new()
                    .fill(fill)
                    .stroke(egui::Stroke::new(1.0, theme::border()))
                    .corner_radius(egui::CornerRadius {
                        nw: 6,
                        ne: 6,
                        sw: 0,
                        se: 0,
                    })
                    .inner_margin(egui::Margin::symmetric(8, 3))
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.x = inner_gap;
                        let color = if tab.current {
                            theme::text()
                        } else {
                            theme::text_dim()
                        };
                        // One row, with the icon and the name on the same middle.
                        ui.horizontal(|ui| {
                            let (_, label) = chrome::icon_and_label(
                                ui,
                                "document",
                                14.0,
                                RichText::new(&tab.title).color(color),
                            );
                            if label.clicked() {
                                switch = Some(index);
                            }
                            if tab.dirty {
                                ui.label(RichText::new("\u{25cf}").color(theme::accent()).small());
                            }
                            if chrome::icon_button(ui, "close", 10.0, false, true, "Close document")
                                .clicked()
                            {
                                close = Some(index);
                            }
                        });
                    });
            }
            ui.add_space(4.0);
            if chrome::icon_button(ui, "plus", 12.0, false, true, "New document").clicked() {
                add = true;
            }
        });
        if add {
            self.new_tab();
        } else if let Some(index) = close {
            self.close_tab(index);
        } else if let Some(index) = switch {
            self.switch_to_tab(index);
        }
    }

    // ---- stage bar ---------------------------------------------------------------------

    /// The bar above the stage: where you are, and zoom.
    pub(crate) fn stage_bar(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            let crumbs = self.breadcrumb();
            ui.add(icons::image("scene", 16.0, theme::text_dim()));
            for (depth, name) in crumbs.iter().enumerate() {
                if depth > 0 {
                    ui.add(icons::image("chevron-right", 12.0, theme::text_disabled()));
                }
                let last = depth + 1 == crumbs.len();
                if depth == 0 && self.scene_renaming.is_some() {
                    self.scene_name_editor(ui);
                    continue;
                }
                let label = if last {
                    RichText::new(name).strong()
                } else {
                    RichText::new(name).color(theme::text_dim())
                };
                let crumb = chrome::crumb(ui, label);
                if depth == 0 && crumb.double_clicked() {
                    self.begin_scene_rename();
                } else if crumb.clicked() && !last {
                    self.exit_to_depth(depth);
                    break;
                }
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if chrome::icon_button(ui, "zoom-in", BAR_ICON, false, true, "Zoom in").clicked() {
                    self.perform(MenuAction::ZoomIn);
                }
                let extent = self.stage_extent();
                let mut percent = self.stage_view.scale(self.stage_panel, extent) * 100.0;
                let before = percent;
                ui.add(
                    egui::DragValue::new(&mut percent)
                        .range(5.0..=3200.0)
                        .speed(2.0)
                        .suffix("%")
                        .fixed_decimals(0),
                );
                if (percent - before).abs() > f32::EPSILON {
                    self.set_stage_percent(percent);
                }
                if chrome::icon_button(ui, "zoom-out", BAR_ICON, false, true, "Zoom out").clicked()
                {
                    self.perform(MenuAction::ZoomOut);
                }
                if chrome::icon_button(ui, "zoom-fit", BAR_ICON, false, true, "Fit stage in window")
                    .clicked()
                {
                    self.perform(MenuAction::ZoomFit);
                }
                if self.has_camera()
                    && chrome::icon_button(
                        ui,
                        "tool-camera",
                        BAR_ICON,
                        self.camera_preview,
                        true,
                        "Preview through the camera",
                    )
                    .clicked()
                {
                    self.set_camera_preview(!self.camera_preview);
                }
                if chrome::icon_button(
                    ui,
                    "snap",
                    BAR_ICON,
                    self.display.snap_to_grid,
                    true,
                    "Snap to grid",
                )
                .clicked()
                {
                    self.perform(MenuAction::ToggleSnap);
                }
                if chrome::icon_button(
                    ui,
                    "ruler",
                    BAR_ICON,
                    self.display.show_rulers,
                    true,
                    "Rulers",
                )
                .clicked()
                {
                    self.perform(MenuAction::ToggleRulers);
                }
                if chrome::icon_button(ui, "grid", BAR_ICON, self.display.show_grid, true, "Grid")
                    .clicked()
                {
                    self.perform(MenuAction::ToggleGrid);
                }
            });
        });
    }

    /// Shows the stage at `percent` of its real size, about the center of the view.
    pub fn set_stage_percent(&mut self, percent: f32) {
        let extent = self.stage_extent();
        let current = self.stage_view.scale(self.stage_panel, extent);
        let wanted = (percent / 100.0).max(0.001);
        self.stage_view
            .zoom_center(wanted / current.max(f32::EPSILON), self.stage_panel, extent);
    }
}
