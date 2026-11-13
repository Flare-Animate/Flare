//! The strip of panel icons at the right edge, as Animate's icon panels: each panel kept there
//! opens beside the strip on a click, over the stage, so the stack on the right (Properties above
//! all) stays in view. A panel can be docked from there, and a docked one collapsed back to an icon.

use egui::{Align2, Order, Pos2, Rect, Sense, Ui, vec2};
use flare_ui::{icons, theme};

use super::*;

/// Width of the rail, in the touch sizes when the interface is driven with fingers.
pub(super) fn rail_width(touch: bool) -> f32 {
    touch::rail_width(touch)
}

/// How far the middle of the rail's icons is from the window's right edge: the rail is
/// `rail_width` wide with a one pixel separator on its inner side.
pub(super) fn center_from_edge(touch: bool) -> f32 {
    (rail_width(touch) - 1.0) / 2.0
}

/// Width of a panel open beside the strip, as wide as Animate's.
pub(super) const FLYOUT_WIDTH: f32 = 300.0;

/// A change to where a panel lives, asked for while the dock is being drawn and made after it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PanelChange {
    Collapse(Pane),
    Dock(Pane),
}

/// The arrangement, the panel open beside the strip, and where its icons were drawn last.
pub(crate) struct Panels {
    pub(crate) layout: flare_dock::Layout,
    pub(crate) flyout: Option<Pane>,
    pub(crate) change: Option<PanelChange>,
    pub(crate) icons: Vec<(Pane, Rect)>,
    /// Every workspace but the arrangement on screen, which is `layout`.
    pub(crate) workspaces: flare_dock::Workspaces,
    /// The name being typed to save the arrangement as a workspace.
    pub(crate) new_workspace: String,
}

impl Default for Panels {
    fn default() -> Self {
        Self {
            layout: flare_dock::default_layout(),
            flyout: None,
            change: None,
            icons: Vec::new(),
            workspaces: flare_dock::Workspaces::default(),
            new_workspace: String::new(),
        }
    }
}

impl Workspace {
    /// Whether `pane` is showing: the front tab of its stack, or open beside the strip.
    pub(crate) fn pane_is_front(&self, pane: Pane) -> bool {
        if self.panels.layout.in_strip(pane) {
            return self.panels.flyout == Some(pane);
        }
        let tree = &self.panels.layout.tree;
        let Some(tile) = tree.tiles.find_pane(&pane) else {
            return false;
        };
        tree.active_tiles().contains(&tile)
    }

    /// Makes the change asked for while the panels were drawn.
    pub(super) fn apply_panel_change(&mut self) {
        match self.panels.change.take() {
            Some(PanelChange::Collapse(pane)) => {
                // Only panels that can be collapsed are offered, so a refusal cannot happen.
                if self.panels.layout.collapse(pane).is_ok() && self.panels.flyout == Some(pane) {
                    self.panels.flyout = None;
                }
            }
            Some(PanelChange::Dock(pane)) => {
                self.panels.layout.dock(pane);
                if self.panels.flyout == Some(pane) {
                    self.panels.flyout = None;
                }
            }
            None => {}
        }
    }

    /// The rail's contents: one icon for each panel kept in the strip.
    pub(super) fn panel_rail(&mut self, ui: &mut Ui) {
        let touch = self.touch;
        let (button, icon_size) = (touch::rail_button(touch), touch::rail_icon(touch));
        ui.spacing_mut().item_spacing = vec2(0.0, if touch { 8.0 } else { 4.0 });
        ui.add_space(4.0);
        let mut clicked = None;
        let mut drawn = Vec::with_capacity(self.panels.layout.strip.len());
        for pane in self.panels.layout.strip.clone() {
            let Some(icon) = pane.rail_icon() else {
                continue;
            };
            let open = self.pane_is_front(pane);
            let response = ui
                .vertical_centered(|ui| {
                    rail_button(ui, icon, open, &self.tr(pane.key()), button, icon_size)
                })
                .inner;
            if response.clicked() {
                clicked = Some(pane);
            }
            drawn.push((pane, response.rect));
        }
        self.panels.icons = drawn;
        if let Some(pane) = clicked {
            // A second click on the open one puts it away, as in Animate.
            self.panels.flyout = if self.panels.flyout == Some(pane) {
                None
            } else {
                Some(pane)
            };
        }
    }

    /// The panel open beside the strip, if any: level with its icon, as high as the window allows.
    /// A press anywhere on the panels or the stage, Escape, or its icon again puts it away.
    pub(super) fn panel_flyout(&mut self, ctx: &egui::Context) {
        let Some(pane) = self.panels.flyout else {
            return;
        };
        if !self.panels.layout.in_strip(pane) {
            self.panels.flyout = None;
            return;
        }
        let screen = ctx.content_rect();
        let icon = self
            .panels
            .icons
            .iter()
            .find(|(p, _)| *p == pane)
            .map(|(_, r)| *r);
        let rail = rail_width(self.touch);
        let right = screen.right() - rail - 2.0;
        let top = icon.map_or(screen.top() + 40.0, |r| r.top());
        let max_height = (screen.bottom() - top - 30.0).max(160.0);
        let top = top
            .min(screen.bottom() - max_height - 30.0)
            .max(screen.top());
        let id = egui::Id::NULL.with("strip-flyout");
        let title = self.tr(pane.key());
        let dock_hint = self.tr("strip-dock");
        let collapse_hint = self.tr("strip-collapse-to-icon");
        let area = egui::Area::new(id)
            .order(Order::Foreground)
            .pivot(Align2::RIGHT_TOP)
            .fixed_pos(Pos2::new(right, top))
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(theme::panel())
                    .stroke(egui::Stroke::new(1.0, theme::border()))
                    .corner_radius(4)
                    .shadow(ui.visuals().popup_shadow)
                    .show(ui, |ui| {
                        ui.set_width(FLYOUT_WIDTH);
                        ui.set_max_height(max_height);
                        egui::Frame::new()
                            .fill(theme::chrome())
                            .inner_margin(egui::Margin::symmetric(8, 3))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(&title).size(12.0));
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            let chrome = flare_ui::chrome::icon_button;
                                            if chrome(
                                                ui,
                                                "panel-collapse",
                                                14.0,
                                                false,
                                                true,
                                                &collapse_hint,
                                            )
                                            .clicked()
                                            {
                                                self.panels.flyout = None;
                                            }
                                            if chrome(
                                                ui,
                                                "panel-dock",
                                                14.0,
                                                false,
                                                true,
                                                &dock_hint,
                                            )
                                            .clicked()
                                            {
                                                self.panels.change = Some(PanelChange::Dock(pane));
                                            }
                                        },
                                    );
                                });
                            });
                        egui::ScrollArea::vertical()
                            .auto_shrink([false, true])
                            .show(ui, |ui| self.pane_contents(ui, pane));
                    });
            });
        self.apply_panel_change();
        let rect = area.response.rect;
        let (escape, pressed_at) = ctx.input(|i| {
            (
                i.key_pressed(egui::Key::Escape),
                i.pointer
                    .any_pressed()
                    .then(|| i.pointer.interact_pos())
                    .flatten(),
            )
        });
        let on_rail = |p: Pos2| p.x >= screen.right() - rail;
        let away = pressed_at.is_some_and(|p| {
            // Popups the panel opened (a color picker, a list) are on layers above it; only a
            // press on the workspace itself puts the panel away.
            let on_workspace = ctx
                .layer_id_at(p)
                .is_none_or(|layer| matches!(layer.order, Order::Background | Order::Middle));
            !rect.contains(p) && !on_rail(p) && on_workspace
        });
        if escape || away {
            self.panels.flyout = None;
        }
    }
}

fn rail_button(
    ui: &mut Ui,
    icon: &str,
    open: bool,
    hint: &str,
    button: f32,
    icon_size: f32,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(button, button), Sense::click());
    if open {
        ui.painter().rect_filled(rect, 5.0, theme::panel_raised());
    } else if response.hovered() {
        ui.painter()
            .rect_filled(rect, 5.0, theme::hover().gamma_multiply(0.6));
    }
    let tint = if open || response.hovered() {
        theme::text()
    } else {
        theme::text_dim()
    };
    let at = Rect::from_center_size(rect.center(), vec2(icon_size, icon_size));
    icons::image(icon, icon_size, tint).paint_at(ui, at);
    response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, open, hint));
    response.on_hover_text(hint)
}
