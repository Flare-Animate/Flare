//! The splash shown when the program starts: the name, the version and the tagline over a picture,
//! for a moment, as the old program's start-up screen was. It never blocks: a click or a key
//! dismisses it, and it goes by itself after `SPLASH_SECONDS`.

use super::Workspace;
use egui::{Align2, Color32, FontId, Rect, Sense, vec2};

/// How long the splash stays when nobody dismisses it.
pub const SPLASH_SECONDS: f64 = 2.5;

impl Workspace {
    /// Shows the splash over the window from the next frame. The apps call it on a plain launch.
    pub fn show_splash(&mut self) {
        self.windows.splash = Some(None);
    }

    pub fn splash_shown(&self) -> bool {
        self.windows.splash.is_some()
    }

    pub fn dismiss_splash(&mut self) {
        self.windows.splash = None;
    }

    pub(super) fn splash_window(&mut self, ctx: &egui::Context) {
        let Some(started) = self.windows.splash else {
            return;
        };
        let now = ctx.input(|i| i.time);
        let started = started.unwrap_or(now);
        self.windows.splash = Some(Some(started));
        let (dismissed, quiet) = ctx.input(|i| {
            let pressed = i.pointer.any_pressed() || i.events.iter().any(is_key_press);
            (pressed, now - started >= SPLASH_SECONDS)
        });
        if dismissed || quiet {
            self.windows.splash = None;
            ctx.request_repaint();
            return;
        }
        // Wake up again to close it, with no input to cause a frame.
        ctx.request_repaint_after(std::time::Duration::from_secs_f64(
            (SPLASH_SECONDS - (now - started)).max(0.0) + 0.01,
        ));
        let version = tr!(
            self,
            "menu-help-version",
            &[("version", env!("CARGO_PKG_VERSION"))],
        );
        let tagline = tr!(self, "menu-help-tagline");
        let screen = ctx.content_rect();
        let area = egui::Id::NULL.with("flare-splash");
        // egui measures a new Area on an invisible first frame, which put the bare window on
        // screen before the splash. Its size is the window's, so egui is told instead.
        if egui::AreaState::load(ctx, area).is_none() {
            let layer = egui::LayerId::new(egui::Order::Foreground, area);
            let state = egui::AreaState {
                pivot_pos: Some(screen.min),
                size: Some(screen.size()),
                ..Default::default()
            };
            ctx.memory_mut(|memory| memory.areas_mut().set_state(layer, state));
        }
        egui::Area::new(area)
            .order(egui::Order::Foreground)
            .fixed_pos(screen.min)
            .fade_in(false)
            .show(ctx, |ui| {
                // Dims and catches the window behind it, so nothing is clicked through.
                ui.allocate_rect(screen, Sense::click_and_drag());
                ui.painter()
                    .rect_filled(screen, 0.0, Color32::from_black_alpha(150));
                let size = flare_ui::splash::SIZE.min(screen.size() - vec2(32.0, 32.0));
                let size = size.max(vec2(1.0, 1.0));
                let rect = Rect::from_center_size(screen.center(), size);
                flare_ui::splash::backdrop(size).paint_at(ui, rect);
                let scale = size.x / flare_ui::splash::SIZE.x;
                let left = rect.left() + 56.0 * scale;
                let flame = 96.0 * scale;
                flare_ui::icons::flame(flame).paint_at(
                    ui,
                    Rect::from_min_size(
                        egui::pos2(left - 6.0 * scale, rect.top() + 92.0 * scale),
                        vec2(flame, flame),
                    ),
                );
                let painter = ui.painter();
                let text_left = left + flame + 18.0 * scale;
                painter.text(
                    egui::pos2(text_left, rect.top() + 150.0 * scale),
                    Align2::LEFT_BOTTOM,
                    "Flare",
                    FontId::proportional(56.0 * scale),
                    Color32::WHITE,
                );
                painter.text(
                    egui::pos2(text_left, rect.top() + 178.0 * scale),
                    Align2::LEFT_BOTTOM,
                    tagline,
                    FontId::proportional(15.0 * scale),
                    Color32::from_rgb(0xff, 0xf0, 0xd8),
                );
                painter.text(
                    egui::pos2(rect.right() - 16.0, rect.bottom() - 12.0),
                    Align2::RIGHT_BOTTOM,
                    version,
                    FontId::proportional(13.0),
                    Color32::from_rgb(0xff, 0xd9, 0xa8),
                );
            });
    }
}

fn is_key_press(event: &egui::Event) -> bool {
    matches!(event, egui::Event::Key { pressed: true, .. })
}
