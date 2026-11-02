//! The Align and Transform panels.

use crate::workspace::TransformEdit;
use crate::workspace::Workspace;
use egui::Ui;
use flare_menubar::MenuAction;
use flare_ui::chrome;

use super::*;

impl Workspace {
    // ---- align -------------------------------------------------------------------------

    pub(crate) fn align_panel(&mut self, ui: &mut Ui) {
        use flare_tools::ops::Align;
        chrome::section(ui, "panel-align", &tr!(self, "al-title"));
        let has = self.has_selection();
        for group in [
            [
                ("align-left", "al-left", Align::Left),
                ("align-center-h", "al-center-h", Align::CenterH),
                ("align-right", "al-right", Align::Right),
            ],
            [
                ("align-top", "al-top", Align::Top),
                ("align-center-v", "al-center-v", Align::CenterV),
                ("align-bottom", "al-bottom", Align::Bottom),
            ],
        ] {
            ui.horizontal(|ui| {
                for (icon, key, how) in group {
                    let hint = self.tr(key);
                    if chrome::icon_button(ui, icon, BAR_ICON, false, has, &hint).clicked() {
                        self.align_selection(how);
                    }
                }
            });
        }
        chrome::section(ui, "distribute-h", &tr!(self, "al-distribute"));
        let three = self.selection_all().len() >= 3;
        ui.horizontal(|ui| {
            if chrome::icon_button(
                ui,
                "distribute-h",
                BAR_ICON,
                false,
                three,
                &tr!(self, "al-distribute-h"),
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
                &tr!(self, "al-distribute-v"),
            )
            .clicked()
            {
                self.distribute_selection(false);
            }
        });
        chrome::section(ui, "bring-front", &tr!(self, "al-arrange"));
        ui.horizontal(|ui| {
            let front = tr!(self, "al-front");
            if chrome::icon_button(ui, "bring-front", BAR_ICON, false, has, &front).clicked() {
                self.arrange_selection(true);
            }
            let back = tr!(self, "al-back");
            if chrome::icon_button(ui, "send-back", BAR_ICON, false, has, &back).clicked() {
                self.arrange_selection(false);
            }
            let can_group = self.is_enabled(MenuAction::Group);
            let group = tr!(self, "al-group");
            if chrome::icon_button(ui, "group", BAR_ICON, false, can_group, &group).clicked() {
                self.perform(MenuAction::Group);
            }
            let can_ungroup = self.is_enabled(MenuAction::Ungroup);
            let ungroup = tr!(self, "al-ungroup");
            if chrome::icon_button(ui, "ungroup", BAR_ICON, false, can_ungroup, &ungroup).clicked()
            {
                self.perform(MenuAction::Ungroup);
            }
        });
        ui.add_space(6.0);
        let to_stage = tr!(self, "al-to-stage");
        ui.checkbox(&mut self.align_to_stage, to_stage);
    }

    // ---- transform ---------------------------------------------------------------------

    pub(crate) fn transform_panel(&mut self, ui: &mut Ui) {
        chrome::section(ui, "panel-transform", &tr!(self, "tf-title"));
        let (scale, rotate, skew) = (
            tr!(self, "tf-scale"),
            tr!(self, "tf-rotate"),
            tr!(self, "tf-skew"),
        );
        let has = self.has_selection();
        let edit = &mut self.transform_edit;
        ui.add_enabled_ui(has, |ui| {
            chrome::row(ui, &scale, |ui| {
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
            chrome::row(ui, &rotate, |ui| {
                ui.add(
                    egui::DragValue::new(&mut edit.rotate)
                        .range(-360.0..=360.0)
                        .suffix("\u{b0}"),
                );
            });
            chrome::row(ui, &skew, |ui| {
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
        let (apply, reset) = (tr!(self, "tf-apply"), tr!(self, "tf-reset"));
        ui.horizontal(|ui| {
            if ui
                .add_enabled(has && changed, egui::Button::new(apply))
                .clicked()
            {
                self.apply_transform_edit();
            }
            if ui.add_enabled(changed, egui::Button::new(reset)).clicked() {
                self.transform_edit = TransformEdit::default();
            }
        });
        chrome::section(ui, "rotate-cw", &tr!(self, "tf-quick"));
        let enabled = self.has_selection();
        ui.horizontal(|ui| {
            for (icon, key, action) in [
                ("flip-horizontal", "tf-flip-h", MenuAction::FlipHorizontal),
                ("flip-vertical", "tf-flip-v", MenuAction::FlipVertical),
                ("rotate-cw", "tf-rotate-cw", MenuAction::RotateClockwise),
                (
                    "rotate-ccw",
                    "tf-rotate-ccw",
                    MenuAction::RotateCounterclockwise,
                ),
                ("zoom-in", "tf-scale-up", MenuAction::ScaleUp),
                ("zoom-out", "tf-scale-down", MenuAction::ScaleDown),
            ] {
                let hint = self.tr(key);
                if chrome::icon_button(ui, icon, BAR_ICON, false, enabled, &hint).clicked() {
                    self.perform(action);
                }
            }
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let can_break = self.is_enabled(MenuAction::BreakApart);
            let apart = tr!(self, "tf-break-apart");
            if chrome::icon_button(ui, "break-apart", BAR_ICON, false, can_break, &apart).clicked()
            {
                self.perform(MenuAction::BreakApart);
            }
            let can = self.is_enabled(MenuAction::ConvertToSymbol);
            let convert = tr!(self, "tf-convert");
            if chrome::icon_button(ui, "convert-symbol", BAR_ICON, false, can, &convert).clicked() {
                self.perform(MenuAction::ConvertToSymbol);
            }
        });
    }
}
