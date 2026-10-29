//! The Align and Transform panels.

use crate::actions::MenuAction;
use crate::workspace::TransformEdit;
use crate::workspace::Workspace;
use egui::Ui;
use flare_ui::chrome;

use super::*;

impl Workspace {
    // ---- align -------------------------------------------------------------------------

    pub(crate) fn align_panel(&mut self, ui: &mut Ui) {
        use flare_tools::ops::Align;
        chrome::section(ui, "panel-align", "Align");
        let has = self.has_selection();
        for group in [
            [
                ("align-left", "Align left edges", Align::Left),
                ("align-center-h", "Align horizontal centers", Align::CenterH),
                ("align-right", "Align right edges", Align::Right),
            ],
            [
                ("align-top", "Align top edges", Align::Top),
                ("align-center-v", "Align vertical centers", Align::CenterV),
                ("align-bottom", "Align bottom edges", Align::Bottom),
            ],
        ] {
            ui.horizontal(|ui| {
                for (icon, hint, how) in group {
                    if chrome::icon_button(ui, icon, BAR_ICON, false, has, hint).clicked() {
                        self.align_selection(how);
                    }
                }
            });
        }
        chrome::section(ui, "distribute-h", "Distribute");
        let three = self.selection_all().len() >= 3;
        ui.horizontal(|ui| {
            if chrome::icon_button(
                ui,
                "distribute-h",
                BAR_ICON,
                false,
                three,
                "Distribute horizontally",
            )
            .clicked()
            {
                self.distribute_selection(true);
            }
            if chrome::icon_button(
                ui,
                "distribute-v",
                BAR_ICON,
                false,
                three,
                "Distribute vertically",
            )
            .clicked()
            {
                self.distribute_selection(false);
            }
        });
        chrome::section(ui, "bring-front", "Arrange");
        ui.horizontal(|ui| {
            if chrome::icon_button(ui, "bring-front", BAR_ICON, false, has, "Bring to front")
                .clicked()
            {
                self.arrange_selection(true);
            }
            if chrome::icon_button(ui, "send-back", BAR_ICON, false, has, "Send to back").clicked()
            {
                self.arrange_selection(false);
            }
            let can_group = self.is_enabled(MenuAction::Group);
            if chrome::icon_button(ui, "group", BAR_ICON, false, can_group, "Group").clicked() {
                self.perform(MenuAction::Group);
            }
            let can_ungroup = self.is_enabled(MenuAction::Ungroup);
            if chrome::icon_button(ui, "ungroup", BAR_ICON, false, can_ungroup, "Ungroup").clicked()
            {
                self.perform(MenuAction::Ungroup);
            }
        });
        ui.add_space(6.0);
        ui.checkbox(&mut self.align_to_stage, "Align to stage");
    }

    // ---- transform ---------------------------------------------------------------------

    pub(crate) fn transform_panel(&mut self, ui: &mut Ui) {
        chrome::section(ui, "panel-transform", "Transform");
        let has = self.has_selection();
        let edit = &mut self.transform_edit;
        ui.add_enabled_ui(has, |ui| {
            chrome::row(ui, "Scale", |ui| {
                ui.add(
                    egui::DragValue::new(&mut edit.scale_x)
                        .range(1.0..=10_000.0)
                        .suffix("%"),
                );
                ui.add(
                    egui::DragValue::new(&mut edit.scale_y)
                        .range(1.0..=10_000.0)
                        .suffix("%"),
                );
            });
            chrome::row(ui, "Rotate", |ui| {
                ui.add(
                    egui::DragValue::new(&mut edit.rotate)
                        .range(-360.0..=360.0)
                        .suffix("\u{b0}"),
                );
            });
            chrome::row(ui, "Skew", |ui| {
                ui.add(
                    egui::DragValue::new(&mut edit.skew_x)
                        .range(-80.0..=80.0)
                        .suffix("\u{b0}"),
                );
                ui.add(
                    egui::DragValue::new(&mut edit.skew_y)
                        .range(-80.0..=80.0)
                        .suffix("\u{b0}"),
                );
            });
        });
        let changed = self.transform_edit != TransformEdit::default();
        ui.horizontal(|ui| {
            if ui
                .add_enabled(has && changed, egui::Button::new("Apply"))
                .clicked()
            {
                self.apply_transform_edit();
            }
            if ui
                .add_enabled(changed, egui::Button::new("Reset"))
                .clicked()
            {
                self.transform_edit = TransformEdit::default();
            }
        });
        chrome::section(ui, "rotate-cw", "Quick transforms");
        let enabled = self.has_selection();
        ui.horizontal(|ui| {
            for (icon, hint, action) in [
                (
                    "flip-horizontal",
                    "Flip horizontal",
                    MenuAction::FlipHorizontal,
                ),
                ("flip-vertical", "Flip vertical", MenuAction::FlipVertical),
                (
                    "rotate-cw",
                    "Rotate 90\u{b0} clockwise",
                    MenuAction::RotateClockwise,
                ),
                (
                    "rotate-ccw",
                    "Rotate 90\u{b0} counterclockwise",
                    MenuAction::RotateCounterclockwise,
                ),
                ("zoom-in", "Scale up", MenuAction::ScaleUp),
                ("zoom-out", "Scale down", MenuAction::ScaleDown),
            ] {
                if chrome::icon_button(ui, icon, BAR_ICON, false, enabled, hint).clicked() {
                    self.perform(action);
                }
            }
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let can_break = self.is_enabled(MenuAction::BreakApart);
            if chrome::icon_button(ui, "break-apart", BAR_ICON, false, can_break, "Break apart")
                .clicked()
            {
                self.perform(MenuAction::BreakApart);
            }
            let can = self.is_enabled(MenuAction::ConvertToSymbol);
            if chrome::icon_button(
                ui,
                "convert-symbol",
                BAR_ICON,
                false,
                can,
                "Convert to symbol (F8)",
            )
            .clicked()
            {
                self.perform(MenuAction::ConvertToSymbol);
            }
        });
    }
}
