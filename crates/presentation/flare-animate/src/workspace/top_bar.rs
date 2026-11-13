//! The top bar: room for the window buttons, the house, the in-window menus (when there is no
//! native menu bar), and Play, Publish, the command palette and the workspace switcher in the
//! corner. Empty space drags the window.
//!
//! The corner is laid out first, from the bar's right end, and the menus get what is left: a
//! window too narrow for every menu puts the last ones in a "»" menu rather than letting them run
//! under the corner or off the window.

use egui::{Align, Layout, Rect, TextStyle, TextWrapMode, UiBuilder, WidgetText, pos2};
use flare_menubar::{MENU_LAYOUT, MenuAction};

use super::*;

/// Space at the bar's left end before the house.
const BAR_MARGIN: f32 = 10.0;
/// Least space between the last menu and the corner's buttons.
const CORNER_GAP: f32 = 12.0;
/// A corner button's square: its 16 px icon and the padding `icon_button` gives it.
const CORNER_SLOT: f32 = 22.0;
/// The menu that holds the menus there is no room for.
const MORE_MENUS: &str = "»";

impl Workspace {
    pub(super) fn menu_bar(&mut self, ui: &mut egui::Ui) {
        let bar = ui.max_rect();
        let drag = ui.interact(bar, self.top_bar_id, egui::Sense::click_and_drag());
        if drag.double_clicked() {
            self.window_request = Some(WindowRequest::ToggleMaximize);
        } else if drag.drag_started() {
            self.window_request = Some(WindowRequest::Drag);
        }
        self.window_buttons(ui, bar);
        ui.painter().hline(
            bar.x_range(),
            bar.bottom() - 0.5,
            egui::Stroke::new(1.0, flare_ui::theme::border()),
        );
        let controls_width = self.window_controls_width();
        let corner_area =
            Rect::from_min_max(bar.min, pos2(bar.right() - controls_width, bar.bottom()));
        let corner = ui
            .scope_builder(
                UiBuilder::new()
                    .max_rect(corner_area)
                    .layout(Layout::right_to_left(Align::Center)),
                |ui| {
                    if self.native_menu {
                        self.classic_bar_right(ui, bar, controls_width);
                    } else if self.document_controls_shown() {
                        // Play, Publish and the switcher belong to a document; Home has none.
                        ui.add_space(self.corner_margin(controls_width));
                        self.animate_bar_right(ui);
                    }
                },
            )
            .response
            .rect;
        let left_area = Rect::from_min_max(
            bar.min,
            pos2((corner.left() - CORNER_GAP).max(bar.left()), bar.bottom()),
        );
        ui.scope_builder(
            UiBuilder::new()
                .max_rect(left_area)
                .layout(Layout::left_to_right(Align::Center)),
            |ui| {
                // Nothing on the left is drawn under the corner, however narrow the window.
                ui.shrink_clip_rect(left_area);
                ui.add_space(self.titlebar_inset.max(BAR_MARGIN));
                self.home_button(ui);
                ui.add_space(6.0);
                if !self.native_menu {
                    self.menus(ui);
                }
            },
        );
    }

    /// Space between the bar's right end, or the window's own buttons, and the corner's last
    /// button. With no window buttons that button is centred over the rail's icons below it.
    fn corner_margin(&self, controls_width: f32) -> f32 {
        if controls_width > 0.0 {
            6.0
        } else {
            (rail::center_from_edge(self.touch) - CORNER_SLOT / 2.0).max(4.0)
        }
    }

    /// The house, which goes to Home and stays there; it is not a switch. Pressing it on Home
    /// used to leave for the untouched document underneath, which looked like a new file. A tab
    /// is how a document is gone back to.
    fn home_button(&mut self, ui: &mut egui::Ui) {
        let home = ui
            .horizontal(|ui| {
                ui.add(flare_ui::icons::image(
                    "home",
                    16.0,
                    flare_ui::theme::text(),
                ));
                // With the system's menu bar the bar keeps its name beside the house; with the
                // in-window menus the menus take that place, as in Animate.
                if self.native_menu {
                    ui.label(
                        egui::RichText::new("Flare")
                            .strong()
                            .color(flare_ui::theme::text()),
                    );
                }
            })
            .response
            .interact(egui::Sense::click())
            .on_hover_text(tr!(self, "bar-home"));
        if home.clicked() {
            self.windows.home = true;
        }
    }

    /// The in-window menus, as many as there is room for, and the rest in a "»" menu after them.
    fn menus(&mut self, ui: &mut egui::Ui) {
        let titles: Vec<String> = MENU_LAYOUT
            .iter()
            .map(|(title, _)| self.menu_title(title))
            .collect();
        let more_tip = tr!(self, "bar-more-menus");
        egui::MenuBar::new().ui(ui, |ui| {
            let shown = menus_that_fit(ui, &titles, ui.available_width());
            for ((_, entries), title) in MENU_LAYOUT.iter().zip(&titles).take(shown) {
                ui.menu_button(title.as_str(), |ui| self.menu_entries(ui, entries));
            }
            if shown < titles.len() {
                ui.menu_button(MORE_MENUS, |ui| {
                    for ((_, entries), title) in MENU_LAYOUT.iter().zip(&titles).skip(shown) {
                        ui.menu_button(title.as_str(), |ui| self.menu_entries(ui, entries));
                    }
                })
                .response
                .on_hover_text(more_tip);
            }
        });
    }

    /// The corner of the Animate bar, drawn right to left: the workspace switcher, the command
    /// palette, Publish and Play, as Animate has them.
    fn animate_bar_right(&mut self, ui: &mut egui::Ui) {
        self.workspace_switcher(ui);
        ui.add_space(6.0);
        // The command palette: every command, one search away.
        let commands_tip = tr!(self, "bar-commands");
        if flare_ui::chrome::icon_button(
            ui,
            "search",
            16.0,
            self.windows.palette,
            true,
            &commands_tip,
        )
        .clicked()
        {
            self.toggle_palette();
        }
        ui.add_space(6.0);
        // Animate's Publish button is next to Play too.
        let publish_tip = tr!(self, "bar-publish");
        let can_publish = self.is_enabled(MenuAction::Publish);
        if flare_ui::chrome::icon_button(ui, "publish", 16.0, false, can_publish, &publish_tip)
            .clicked()
        {
            self.perform(MenuAction::Publish);
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
    }
}

/// How many of the menus `titles` fit in `room` side by side in `ui`'s style, leaving space for
/// the "»" menu when not all of them do.
fn menus_that_fit(ui: &egui::Ui, titles: &[String], room: f32) -> usize {
    let spacing = ui.spacing().item_spacing.x;
    let padding = ui.spacing().button_padding.x * 2.0;
    let width = |text: &str| {
        WidgetText::from(text)
            .into_galley(
                ui,
                Some(TextWrapMode::Extend),
                f32::INFINITY,
                TextStyle::Button,
            )
            .size()
            .x
            + padding
    };
    let widths: Vec<f32> = titles.iter().map(|t| width(t)).collect();
    let all = widths.iter().sum::<f32>() + spacing * widths.len().saturating_sub(1) as f32;
    if all <= room {
        return titles.len();
    }
    let mut used = width(MORE_MENUS);
    let mut shown = 0;
    for width in widths {
        if used + spacing + width > room {
            break;
        }
        used += spacing + width;
        shown += 1;
    }
    shown
}
