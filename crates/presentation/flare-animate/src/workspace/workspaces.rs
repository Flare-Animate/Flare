//! Named workspaces and their switcher at the top right of the bar, as Animate's: a field to save
//! the arrangement on screen under a name, the built-in workspaces, the person's own, and on the
//! one in use a button that takes it back to how it was built in or saved.

use egui::{Popup, PopupCloseBehavior, RichText, Ui};
use flare_dock::{Preset, WorkspaceId};
use flare_ui::{chrome, theme};

use super::*;

impl Workspace {
    /// The workspace in use.
    pub fn active_workspace(&self) -> &WorkspaceId {
        &self.panels.workspaces.active
    }

    /// Puts the arrangement on screen away with the workspace in use and shows `to` as it was last
    /// left. The document is never touched.
    pub fn switch_workspace(&mut self, to: WorkspaceId) {
        if self.panels.workspaces.active == to {
            return;
        }
        let current = self.panels.layout.clone();
        match self.panels.workspaces.switch(current, to) {
            Ok(layout) => {
                self.panels.layout = layout;
                self.panels.flyout = None;
            }
            Err(error) => {
                let reason = error.to_string();
                self.report(tr!(self, "workspace-refused", &[("reason", &reason)]));
            }
        }
    }

    pub(super) fn switch_preset(&mut self, preset: Preset) {
        self.switch_workspace(WorkspaceId::Preset(preset));
    }

    /// Saves the arrangement on screen as a workspace of the person's own and makes it the one in
    /// use. A name already saved is replaced.
    pub fn save_workspace(&mut self, name: &str) -> bool {
        let current = self.panels.layout.clone();
        match self.panels.workspaces.save_as(name, current) {
            Ok(()) => {
                let name = name.trim().to_owned();
                self.report(tr!(self, "workspace-saved", &[("name", &name)]));
                true
            }
            Err(error) => {
                let reason = error.to_string();
                self.report(tr!(self, "workspace-refused", &[("reason", &reason)]));
                false
            }
        }
    }

    /// Forgets a workspace of the person's own; when it was in use, Essentials is shown.
    pub fn delete_workspace(&mut self, name: &str) {
        match self.panels.workspaces.delete(name) {
            Ok(Some(layout)) => {
                self.panels.layout = layout;
                self.panels.flyout = None;
            }
            Ok(None) => {}
            Err(error) => {
                let reason = error.to_string();
                self.report(tr!(self, "workspace-refused", &[("reason", &reason)]));
            }
        }
    }

    /// The name of `id` in the interface's language; a saved one keeps the name it was given.
    pub(super) fn workspace_name(&self, id: &WorkspaceId) -> String {
        match id {
            WorkspaceId::Preset(preset) => self.tr(preset.key()),
            WorkspaceId::Custom(name) => name.clone(),
        }
    }

    /// The switcher's button, and its list while it is open.
    pub(super) fn workspace_switcher(&mut self, ui: &mut Ui) {
        let active = self.workspace_name(&self.panels.workspaces.active.clone());
        let hint = format!("{}: {active}", tr!(self, "bar-workspaces"));
        let response = chrome::icon_button(ui, "interface", 16.0, false, true, &hint);
        Popup::from_toggle_button_response(&response)
            .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| {
                ui.set_width(230.0);
                self.workspace_list(ui);
            });
    }

    /// What the switcher lists: the field to save under a name, then every workspace.
    pub(super) fn workspace_list(&mut self, ui: &mut Ui) {
        ui.label(
            RichText::new(tr!(self, "workspace-new"))
                .small()
                .color(theme::text_dim()),
        );
        ui.horizontal(|ui| {
            let save_hint = tr!(self, "workspace-save");
            let field = ui.add(
                egui::TextEdit::singleline(&mut self.panels.new_workspace)
                    .char_limit(flare_dock::MAX_NAME_CHARS)
                    .desired_width(180.0),
            );
            let entered = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let can_save = !self.panels.new_workspace.trim().is_empty();
            let clicked =
                chrome::icon_button(ui, "file-save", 16.0, false, can_save, &save_hint).clicked();
            if can_save && (entered || clicked) {
                let name = self.panels.new_workspace.clone();
                if self.save_workspace(&name) {
                    self.panels.new_workspace.clear();
                }
            }
        });
        ui.separator();
        ui.label(
            RichText::new(tr!(self, "workspace-built-in"))
                .small()
                .color(theme::text_dim()),
        );
        for preset in Preset::ALL {
            self.workspace_row(ui, WorkspaceId::Preset(preset));
        }
        let custom: Vec<String> = self
            .panels
            .workspaces
            .custom_names()
            .map(str::to_owned)
            .collect();
        if !custom.is_empty() {
            ui.separator();
            ui.label(
                RichText::new(tr!(self, "workspace-yours"))
                    .small()
                    .color(theme::text_dim()),
            );
            for name in custom {
                self.workspace_row(ui, WorkspaceId::Custom(name));
            }
        }
    }

    fn workspace_row(&mut self, ui: &mut Ui, id: WorkspaceId) {
        let active = self.panels.workspaces.active == id;
        let name = self.workspace_name(&id);
        ui.horizontal(|ui| {
            let row =
                ui.add(egui::Button::selectable(active, &name).min_size(egui::vec2(150.0, 0.0)));
            if row.clicked() {
                self.switch_workspace(id.clone());
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let WorkspaceId::Custom(custom) = &id {
                    let hint = tr!(self, "workspace-delete", &[("name", &name)]);
                    if chrome::icon_button(ui, "close", 14.0, false, true, &hint).clicked() {
                        self.delete_workspace(custom);
                    }
                }
                if active {
                    let hint = tr!(self, "workspace-reset", &[("name", &name)]);
                    if chrome::icon_button(ui, "rotate-ccw", 14.0, false, true, &hint).clicked() {
                        self.reset_layout();
                    }
                }
            });
        });
    }
}
