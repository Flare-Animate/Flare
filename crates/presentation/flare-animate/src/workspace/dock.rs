//! The dock: draws each panel by calling back into the workspace.

use crate::layout::Pane;

use super::*;

/// Draws each dock panel by calling back into the workspace.
pub(super) struct Dock<'a> {
    pub(super) workspace: &'a mut Workspace,
}

impl egui_tiles::Behavior<Pane> for Dock<'_> {
    fn pane_ui(
        &mut self,
        ui: &mut egui::Ui,
        _tile: egui_tiles::TileId,
        pane: &mut Pane,
    ) -> egui_tiles::UiResponse {
        match pane {
            Pane::Stage => self.workspace.stage(ui),
            Pane::Timeline => self.workspace.timeline(ui),
            Pane::Library => self.workspace.library_panel(ui),
            Pane::Tools => self.workspace.tools_panel(ui),
            Pane::Properties => self.workspace.properties_panel(ui),
            Pane::Color => self.workspace.color_panel(ui),
            Pane::Align => self.workspace.align_panel(ui),
            Pane::Transform => self.workspace.transform_panel(ui),
            Pane::History => self.workspace.history_panel(ui),
            Pane::Info => self.workspace.info_panel(ui),
            Pane::Output => self.workspace.output_panel(ui),
        }
        egui_tiles::UiResponse::None
    }

    fn tab_title_for_pane(&mut self, pane: &Pane) -> egui::WidgetText {
        pane.title().into()
    }

    fn tab_bar_height(&self, _style: &egui::Style) -> f32 {
        26.0
    }

    fn gap_width(&self, _style: &egui::Style) -> f32 {
        3.0
    }

    fn tab_bar_color(&self, _visuals: &egui::Visuals) -> egui::Color32 {
        flare_ui::theme::BG
    }

    fn tab_bg_color(
        &self,
        _visuals: &egui::Visuals,
        _tiles: &egui_tiles::Tiles<Pane>,
        _tile_id: egui_tiles::TileId,
        state: &egui_tiles::TabState,
    ) -> egui::Color32 {
        if state.active {
            flare_ui::theme::PANEL
        } else {
            flare_ui::theme::BG
        }
    }

    fn tab_text_color(
        &self,
        _visuals: &egui::Visuals,
        _tiles: &egui_tiles::Tiles<Pane>,
        _tile_id: egui_tiles::TileId,
        state: &egui_tiles::TabState,
    ) -> egui::Color32 {
        if state.active {
            flare_ui::theme::TEXT
        } else {
            flare_ui::theme::TEXT_DIM
        }
    }
}
