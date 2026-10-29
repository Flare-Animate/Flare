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
        // The stage and the timeline use their whole area; every other panel keeps its content
        // off the edge.
        let (horizontal, vertical) = flare_ui::theme::PANEL_PADDING;
        let inset = egui::Frame::new().inner_margin(egui::Margin::symmetric(horizontal, vertical));
        match pane {
            Pane::Stage => self.workspace.stage(ui),
            Pane::Timeline => self.workspace.timeline(ui),
            Pane::Library => inset.show(ui, |ui| self.workspace.library_panel(ui)).inner,
            Pane::Tools => self.workspace.tools_panel(ui),
            Pane::Properties => {
                inset
                    .show(ui, |ui| self.workspace.properties_panel(ui))
                    .inner
            }
            Pane::Color => inset.show(ui, |ui| self.workspace.color_panel(ui)).inner,
            Pane::Align => inset.show(ui, |ui| self.workspace.align_panel(ui)).inner,
            Pane::Transform => {
                inset
                    .show(ui, |ui| self.workspace.transform_panel(ui))
                    .inner
            }
            Pane::History => inset.show(ui, |ui| self.workspace.history_panel(ui)).inner,
            Pane::Info => inset.show(ui, |ui| self.workspace.info_panel(ui)).inner,
            Pane::Output => inset.show(ui, |ui| self.workspace.output_panel(ui)).inner,
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
        flare_ui::theme::GUTTER
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

    /// Dock tabs are flat: the raised one is told apart by its fill, not by an outline, which
    /// leaves the outlined, accent-marked segmented control inside panels clearly a different
    /// kind of control.
    fn tab_outline_stroke(
        &self,
        _visuals: &egui::Visuals,
        _tiles: &egui_tiles::Tiles<Pane>,
        _tile_id: egui_tiles::TileId,
        _state: &egui_tiles::TabState,
    ) -> egui::Stroke {
        egui::Stroke::NONE
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
