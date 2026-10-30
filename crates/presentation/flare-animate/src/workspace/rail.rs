//! The narrow bar of panel icons at the right edge: one click brings that panel to the front of
//! its stack, as the collapsed panel strip does in Animate.

use egui::{Rect, Sense, Ui, vec2};
use flare_ui::{icons, theme};

use super::*;

/// The panels in the rail, top to bottom.
pub(crate) const RAIL: [Pane; 7] = [
    Pane::Properties,
    Pane::Library,
    Pane::Color,
    Pane::Align,
    Pane::Transform,
    Pane::History,
    Pane::Info,
];

/// Width of the rail.
pub(super) const RAIL_WIDTH: f32 = 34.0;

const BUTTON: f32 = 28.0;
const ICON: f32 = 18.0;

impl Workspace {
    /// Whether `pane` is the one showing in its stack.
    pub(crate) fn pane_is_front(&self, pane: Pane) -> bool {
        let Some(tile) = self.layout.tiles.find_pane(&pane) else {
            return false;
        };
        self.layout.active_tiles().contains(&tile)
    }

    /// The rail's contents.
    pub(super) fn panel_rail(&mut self, ui: &mut Ui) {
        ui.spacing_mut().item_spacing = vec2(0.0, 4.0);
        ui.add_space(4.0);
        let mut chosen = None;
        for (index, pane) in RAIL.into_iter().enumerate() {
            let Some(icon) = pane.rail_icon() else {
                continue;
            };
            // A little space between the groups of related panels.
            if index == 3 || index == 5 {
                ui.add_space(4.0);
            }
            let front = self.pane_is_front(pane);
            ui.vertical_centered(|ui| {
                if rail_button(ui, icon, front, pane.title()).clicked() {
                    chosen = Some(pane);
                }
            });
        }
        if let Some(pane) = chosen {
            self.show_pane(pane);
        }
    }
}

fn rail_button(ui: &mut Ui, icon: &str, front: bool, hint: &str) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(BUTTON, BUTTON), Sense::click());
    if front {
        ui.painter().rect_filled(rect, 5.0, theme::PANEL_RAISED);
    } else if response.hovered() {
        ui.painter()
            .rect_filled(rect, 5.0, theme::HOVER.gamma_multiply(0.6));
    }
    let tint = if front || response.hovered() {
        theme::TEXT
    } else {
        theme::TEXT_DIM
    };
    let at = Rect::from_center_size(rect.center(), vec2(ICON, ICON));
    icons::image(icon, ICON, tint).paint_at(ui, at);
    response.on_hover_text(hint)
}
