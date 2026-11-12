//! The dock: draws each panel by calling back into the workspace.

use flare_dock::Pane;

use super::rail::PanelChange;

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
        self.workspace.pane_contents(ui, *pane);
        egui_tiles::UiResponse::None
    }

    /// A right click on the tab of a panel that can be kept in the strip offers to put it there.
    fn on_tab_button(
        &mut self,
        tiles: &mut egui_tiles::Tiles<Pane>,
        tile_id: egui_tiles::TileId,
        button_response: egui::Response,
    ) -> egui::Response {
        let Some(egui_tiles::Tile::Pane(pane)) = tiles.get(tile_id) else {
            return button_response;
        };
        let pane = *pane;
        if pane.can_collapse() {
            let label = self.workspace.tr("strip-collapse-to-icon");
            button_response.context_menu(|ui| {
                if ui.button(label).clicked() {
                    self.workspace.panels.change = Some(PanelChange::Collapse(pane));
                    ui.close();
                }
            });
        }
        button_response
    }

    /// At the right of a stack's tabs, as Animate's panel groups have it: put the panel in front
    /// into the strip.
    fn top_bar_right_ui(
        &mut self,
        tiles: &egui_tiles::Tiles<Pane>,
        ui: &mut egui::Ui,
        _tile_id: egui_tiles::TileId,
        tabs: &egui_tiles::Tabs,
        _scroll_offset: &mut f32,
    ) {
        let Some(egui_tiles::Tile::Pane(pane)) = tabs.active.and_then(|a| tiles.get(a)) else {
            return;
        };
        if !pane.can_collapse() {
            return;
        }
        let hint = self.workspace.tr("strip-collapse-to-icon");
        if flare_ui::chrome::icon_button(ui, "panel-collapse", 14.0, false, true, &hint).clicked() {
            self.workspace.panels.change = Some(PanelChange::Collapse(*pane));
        }
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

impl Workspace {
    /// What a panel shows, docked or open beside the strip. The stage and the timeline use their
    /// whole area; every other panel keeps its content off the edge.
    pub(super) fn pane_contents(&mut self, ui: &mut egui::Ui, pane: Pane) {
        let (horizontal, vertical) = flare_ui::theme::PANEL_PADDING;
        let inset = egui::Frame::new().inner_margin(egui::Margin::symmetric(horizontal, vertical));
        match pane {
            Pane::Stage => self.stage(ui),
            Pane::Timeline => self.timeline(ui),
            Pane::Tools => self.tools_panel(ui),
            Pane::Library => inset.show(ui, |ui| self.library_panel(ui)).inner,
            Pane::Layers => inset.show(ui, |ui| self.layers_panel(ui)).inner,
            Pane::Properties => inset.show(ui, |ui| self.properties_panel(ui)).inner,
            Pane::Color => inset.show(ui, |ui| self.color_panel(ui)).inner,
            Pane::Align => inset.show(ui, |ui| self.align_panel(ui)).inner,
            Pane::Transform => inset.show(ui, |ui| self.transform_panel(ui)).inner,
            Pane::History => inset.show(ui, |ui| self.history_panel(ui)).inner,
            Pane::Info => inset.show(ui, |ui| self.info_panel(ui)).inner,
            Pane::Output => inset.show(ui, |ui| self.output_panel(ui)).inner,
            Pane::MotionEditor => inset.show(ui, |ui| self.motion_editor_panel(ui)).inner,
            Pane::FindReplace => inset.show(ui, |ui| self.find_replace_panel(ui)).inner,
            Pane::Assets => inset.show(ui, |ui| self.assets_panel(ui)).inner,
            Pane::Swatches => inset.show(ui, |ui| self.swatches_panel(ui)).inner,
        }
    }
}
