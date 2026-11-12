//! The top bar where the menus are the system's (macOS): the window's own buttons float over the
//! left end, and at the right are the gear, the workspace switcher, undo, redo, Play and Publish,
//! with the document's name in the middle. Where the menus are in the window the bar is Animate's
//! (see `menu_bar`).

use egui::{Pos2, Rect, Vec2};
use flare_menubar::MenuAction;

use super::*;

/// The gear button's square: its 16 px icon with the padding `icon_button` adds.
const GEAR_SLOT: f32 = 22.0;

impl Workspace {
    /// The right of the bar for a native menu, drawn right to left inside `ui`.
    pub(super) fn classic_bar_right(&mut self, ui: &mut egui::Ui, bar: Rect, controls_width: f32) {
        let (preferences_tip, publish_tip) =
            (tr!(self, "bar-preferences"), tr!(self, "bar-publish"));
        // Preferences are the app's, not a document's: the gear is in the same corner on
        // Home and in the editor, centered over the rail's icons below it.
        let open = self.windows.preferences;
        let slot = Rect::from_center_size(
            Pos2::new(
                bar.right() - controls_width - rail::CENTER_FROM_EDGE,
                bar.center().y,
            ),
            Vec2::splat(GEAR_SLOT),
        );
        let gear = ui
            .scope_builder(egui::UiBuilder::new().max_rect(slot), |ui| {
                flare_ui::chrome::icon_button(ui, "gear", 16.0, open, true, &preferences_tip)
            })
            .inner;
        if gear.clicked() {
            self.windows.preferences = !open;
        }
        ui.add_space(rail::CENTER_FROM_EDGE + GEAR_SLOT / 2.0 + 6.0);
        // Undo, play and the document's name belong to a document; Home has none, and
        // no interface to switch.
        if !self.document_controls_shown() {
            return;
        }
        // Animate's workspace switcher is in this corner too.
        self.workspace_switcher(ui);
        ui.add_space(6.0);
        let can_redo = self.active().can_redo();
        if flare_ui::chrome::icon_button(ui, "redo", 16.0, false, can_redo, &tr!(self, "bar-redo"))
            .clicked()
        {
            self.redo();
        }
        let can_undo = self.active().can_undo();
        if flare_ui::chrome::icon_button(ui, "undo", 16.0, false, can_undo, &tr!(self, "bar-undo"))
            .clicked()
        {
            self.undo();
        }
        let (icon, hint) = if self.is_playing() {
            ("transport-pause", tr!(self, "bar-pause"))
        } else {
            ("transport-play", tr!(self, "bar-play"))
        };
        let play = flare_ui::chrome::icon_button(ui, icon, 16.0, false, true, &hint);
        if play.clicked() {
            self.perform(MenuAction::PlayPause);
        }
        play.context_menu(|ui| self.transport_menu(ui));
        // Animate's Publish button is next to Play too.
        let can_publish = self.is_enabled(MenuAction::Publish);
        if flare_ui::chrome::icon_button(ui, "publish", 16.0, false, can_publish, &publish_tip)
            .clicked()
        {
            self.perform(MenuAction::Publish);
        }
        // The name sits in the middle of what is left, whatever the window width.
        let title = egui::RichText::new(self.title()).color(flare_ui::theme::text_dim());
        let rect = ui.available_rect_before_wrap();
        ui.painter().text(
            egui::pos2(bar.center().x, rect.center().y),
            egui::Align2::CENTER_CENTER,
            title.text(),
            egui::FontId::proportional(13.0),
            flare_ui::theme::text_dim(),
        );
    }
}
