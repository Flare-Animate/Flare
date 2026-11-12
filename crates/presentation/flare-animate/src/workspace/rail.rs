//! The narrow bar of panel icons at the right edge: one click brings that panel to the front of
//! its stack, as the collapsed panel strip does in Animate.

use egui::{Rect, Sense, Ui, vec2};
use flare_ui::{icons, theme};

use super::*;

/// The panels in the rail, top to bottom.
pub(crate) const RAIL: [Pane; 8] = [
    Pane::Properties,
    Pane::Library,
    Pane::Layers,
    Pane::Color,
    Pane::Align,
    Pane::Transform,
    Pane::History,
    Pane::Info,
];

/// Width of the rail panel, in the touch sizes when the interface is driven with fingers.
pub(super) fn rail_width(touch: bool) -> f32 {
    touch::rail_width(touch)
}

/// How far the middle of the rail's icons is from the window's right edge: the rail is
/// `rail_width` wide with a one pixel separator on its inner side.
pub(super) fn center_from_edge(touch: bool) -> f32 {
    (rail_width(touch) - 1.0) / 2.0
}

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
        let touch = self.touch;
        let (button, icon) = (touch::rail_button(touch), touch::rail_icon(touch));
        ui.spacing_mut().item_spacing = vec2(0.0, if touch { 8.0 } else { 4.0 });
        ui.add_space(4.0);
        let mut chosen = None;
        for (index, pane) in RAIL.into_iter().enumerate() {
            let Some(icon_name) = pane.rail_icon() else {
                continue;
            };
            // A little space between the groups of related panels.
            if index == 3 || index == 6 {
                ui.add_space(4.0);
            }
            let front = self.pane_is_front(pane);
            ui.vertical_centered(|ui| {
                if rail_button(ui, icon_name, front, &self.tr(pane.key()), button, icon).clicked() {
                    chosen = Some(pane);
                }
            });
        }
        if let Some(pane) = chosen {
            self.show_pane(pane);
        }
    }
}

fn rail_button(
    ui: &mut Ui,
    icon_name: &str,
    front: bool,
    hint: &str,
    button: f32,
    icon: f32,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(button, button), Sense::click());
    if front {
        ui.painter().rect_filled(rect, 5.0, theme::panel_raised());
    } else if response.hovered() {
        ui.painter()
            .rect_filled(rect, 5.0, theme::hover().gamma_multiply(0.6));
    }
    let tint = if front || response.hovered() {
        theme::text()
    } else {
        theme::text_dim()
    };
    let at = Rect::from_center_size(rect.center(), vec2(icon, icon));
    icons::image(icon_name, icon, tint).paint_at(ui, at);
    response.on_hover_text(hint)
}
