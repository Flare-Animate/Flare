//! The dock: draws each panel by calling back into the workspace.

use flare_dock::Pane;

use super::*;

/// Height of a stack's row of tabs, and the size of the names in it.
pub(super) const TAB_BAR_HEIGHT: f32 = 22.0;
const TAB_TEXT_SIZE: f32 = 12.0;

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
            Pane::Layers => inset.show(ui, |ui| self.workspace.layers_panel(ui)).inner,
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
            Pane::MotionEditor => {
                inset
                    .show(ui, |ui| self.workspace.motion_editor_panel(ui))
                    .inner
            }
            Pane::FindReplace => {
                inset
                    .show(ui, |ui| self.workspace.find_replace_panel(ui))
                    .inner
            }
            Pane::Assets => inset.show(ui, |ui| self.workspace.assets_panel(ui)).inner,
        }
        egui_tiles::UiResponse::None
    }

    // Panel tabs are labels, smaller than the panel's own text, so the panel reads first.
    fn tab_title_for_pane(&mut self, pane: &Pane) -> egui::WidgetText {
        egui::RichText::new(self.workspace.tr(pane.key()))
            .size(TAB_TEXT_SIZE)
            .into()
    }

    fn tab_bar_height(&self, _style: &egui::Style) -> f32 {
        TAB_BAR_HEIGHT
    }

    fn gap_width(&self, _style: &egui::Style) -> f32 {
        flare_ui::theme::GUTTER
    }

    fn tab_bar_color(&self, _visuals: &egui::Visuals) -> egui::Color32 {
        flare_ui::theme::bg()
    }

    fn tab_bg_color(
        &self,
        _visuals: &egui::Visuals,
        _tiles: &egui_tiles::Tiles<Pane>,
        _tile_id: egui_tiles::TileId,
        state: &egui_tiles::TabState,
    ) -> egui::Color32 {
        if state.active {
            flare_ui::theme::panel()
        } else {
            flare_ui::theme::bg()
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
            flare_ui::theme::text()
        } else {
            flare_ui::theme::text_dim()
        }
    }
}
