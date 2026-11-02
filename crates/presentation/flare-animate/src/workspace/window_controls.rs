//! The window's own buttons, for a window the system gives no title bar: minimize, maximize or
//! restore, and close, at the right end of the top bar where Windows and most Linux desktops
//! keep them.

use egui::{Color32, Pos2, Rect, Sense, Ui, Vec2};
use flare_menubar::MenuAction;

use super::*;

/// How wide each button is; the bar's height is theirs.
const BUTTON_WIDTH: f32 = 40.0;
/// Windows' own red for the close button under the pointer.
const CLOSE_HOVER: Color32 = Color32::from_rgb(232, 17, 35);

impl Workspace {
    /// How much of the bar's right end the buttons take.
    pub(super) fn window_controls_width(&self) -> f32 {
        if self.window_controls {
            BUTTON_WIDTH * 3.0
        } else {
            0.0
        }
    }

    /// Draws the buttons over the bar and does what a click on one asks.
    pub(super) fn window_buttons(&mut self, ui: &mut Ui, bar: Rect) {
        if !self.window_controls {
            return;
        }
        let middle = if self.window_maximized {
            ("window-restore", tr!(self, "bar-window-restore"))
        } else {
            ("window-maximize", tr!(self, "bar-window-maximize"))
        };
        let buttons = [
            ("window-minimize", tr!(self, "bar-window-minimize")),
            middle,
            ("close", tr!(self, "close")),
        ];
        let mut clicked = None;
        for (index, (icon, tip)) in buttons.iter().enumerate() {
            let from_right = (buttons.len() - index) as f32;
            let rect = Rect::from_min_max(
                Pos2::new(bar.right() - BUTTON_WIDTH * from_right, bar.top()),
                Pos2::new(
                    bar.right() - BUTTON_WIDTH * (from_right - 1.0),
                    bar.bottom() - 1.0,
                ),
            );
            let id = self.top_bar_id.with(("window_button", index));
            let response = ui.interact(rect, id, Sense::click()).on_hover_text(tip);
            let closing = index == buttons.len() - 1;
            if response.hovered() {
                let fill = if closing {
                    CLOSE_HOVER
                } else {
                    flare_ui::theme::hover()
                };
                ui.painter().rect_filled(rect, 0.0, fill);
            }
            let tint = if closing && response.hovered() {
                Color32::WHITE
            } else {
                flare_ui::theme::text()
            };
            let icon_rect = Rect::from_center_size(rect.center(), Vec2::splat(16.0));
            // Painted, not placed: placing would move the layout's cursor and push the menus
            // drawn after these buttons out of the bar.
            flare_ui::icons::image(icon, 16.0, tint).paint_at(ui, icon_rect);
            if response.clicked() {
                clicked = Some(index);
            }
        }
        match clicked {
            Some(0) => self.window_request = Some(WindowRequest::Minimize),
            Some(1) => self.window_request = Some(WindowRequest::ToggleMaximize),
            // The same road as Quit in the menu, so unsaved work is asked about first.
            Some(_) => self.perform(MenuAction::Quit),
            None => {}
        }
    }
}
