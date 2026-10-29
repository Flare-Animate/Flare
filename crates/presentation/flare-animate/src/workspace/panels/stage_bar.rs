//! The status bar, document tabs and the bar above the stage.

use crate::actions::MenuAction;
use crate::workspace::{Mode, Workspace};
use egui::{Align, Layout, RichText, Ui};
use flare_ui::chrome;
use flare_ui::icons;
use flare_ui::theme;

use super::*;

impl Workspace {
    // ---- status bar --------------------------------------------------------------------

    pub(crate) fn status_bar(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            let message = match &self.status {
                Some(status) => RichText::new(status.as_str()).color(theme::WARNING),
                None => RichText::new(self.tool_hint()).color(theme::TEXT_DIM),
            };
            ui.label(message);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let extent = self.stage_extent();
                let percent = self.stage_view.scale(self.stage_panel, extent).max(0.0) * 100.0;
                ui.label(RichText::new(format!("{percent:.0}%")).color(theme::TEXT_DIM));
                ui.separator();
                ui.label(
                    RichText::new(format!(
                        "Frame {} / {}",
                        self.frame + 1,
                        self.scene_frames()
                    ))
                    .color(theme::TEXT_DIM),
                );
                ui.separator();
                let at = match self.pointer_stage {
                    Some(p) => format!("{:.0}, {:.0}", p.x, p.y),
                    None => "-, -".to_owned(),
                };
                ui.label(RichText::new(at).color(theme::TEXT_DIM).monospace());
                ui.add(icons::image("tool-selection", 12.0, theme::TEXT_DISABLED));
                if self.background.mode() == Mode::Threads && self.has_pending_io() {
                    ui.separator();
                    ui.label(RichText::new("Working...").color(theme::ACCENT));
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
            for (index, tab) in tabs.iter().enumerate() {
                let fill = if tab.current { theme::PANEL } else { theme::BG };
                egui::Frame::new()
                    .fill(fill)
                    .stroke(egui::Stroke::new(1.0, theme::BORDER))
                    .corner_radius(egui::CornerRadius {
                        nw: 6,
                        ne: 6,
                        sw: 0,
                        se: 0,
                    })
                    .inner_margin(egui::Margin::symmetric(8, 3))
                    .show(ui, |ui| {
                        ui.add(icons::image("document", 14.0, theme::TEXT_DIM));
                        let color = if tab.current {
                            theme::TEXT
                        } else {
                            theme::TEXT_DIM
                        };
                        let label = ui.add(
                            egui::Label::new(RichText::new(&tab.title).color(color))
                                .sense(egui::Sense::click()),
                        );
                        if label.clicked() {
                            switch = Some(index);
                        }
                        if tab.dirty {
                            ui.label(RichText::new("\u{25cf}").color(theme::ACCENT).small());
                        }
                        if chrome::icon_button(ui, "close", 10.0, false, true, "Close document")
                            .clicked()
                        {
                            close = Some(index);
                        }
                    });
            }
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
            ui.add(icons::image("scene", 16.0, theme::TEXT_DIM));
            for (depth, name) in crumbs.iter().enumerate() {
                if depth > 0 {
                    ui.add(icons::image("chevron-right", 12.0, theme::TEXT_DISABLED));
                }
                let last = depth + 1 == crumbs.len();
                let label = if last {
                    RichText::new(name).strong()
                } else {
                    RichText::new(name).color(theme::TEXT_DIM)
                };
                if ui.link(label).clicked() && !last {
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
