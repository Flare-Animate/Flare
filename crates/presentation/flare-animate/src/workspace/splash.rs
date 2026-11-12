//! The splash shown when the program starts, laid out as Animate's: a card in the middle of the
//! window with the mark, the name, the version and a word on what is loading on a light half, and
//! Flare's own artwork on the other. It never blocks: a click or a key dismisses it, and it goes
//! by itself after `SPLASH_SECONDS`.

use super::Workspace;
use egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Ui, vec2};

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
        let words = Words {
            version: tr!(
                self,
                "menu-help-version",
                &[("version", env!("CARGO_PKG_VERSION"))],
            ),
            tagline: tr!(self, "menu-help-tagline"),
            credit: tr!(self, "start-splash-credit"),
            loading: tr!(self, "start-splash-loading"),
        };
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
                // Covers the window, so the app is not seen until the splash has gone, and catches
                // the pointer, so nothing is clicked through.
                ui.allocate_rect(screen, Sense::click_and_drag());
                let sheet = ui.ctx().global_style().visuals.panel_fill;
                ui.painter().rect_filled(screen, 0.0, sheet.to_opaque());
                card(ui, screen, &words);
            });
    }
}

fn is_key_press(event: &egui::Event) -> bool {
    matches!(event, egui::Event::Key { pressed: true, .. })
}

/// The words on the card, in the interface's language.
struct Words {
    version: String,
    tagline: String,
    credit: String,
    loading: String,
}

/// The card's size in points when the window has room for it.
const CARD: egui::Vec2 = egui::Vec2::new(640.0, 360.0);
const RADIUS: u8 = 10;
const INK: Color32 = Color32::from_rgb(0x1c, 0x1c, 0x22);
const DIM: Color32 = Color32::from_rgb(0x6b, 0x6b, 0x75);

/// Paints the card in the middle of `screen`, scaled down to fit a small window.
fn card(ui: &mut Ui, screen: Rect, words: &Words) {
    let room = (screen.size() - vec2(32.0, 32.0)).max(vec2(1.0, 1.0));
    let scale = (room.x / CARD.x).min(room.y / CARD.y).min(1.0);
    let rect = Rect::from_center_size(screen.center(), CARD * scale);
    let painter = ui.painter().clone();
    let shadow = egui::epaint::Shadow {
        offset: [0, 10],
        blur: 36,
        spread: 0,
        color: Color32::from_black_alpha(140),
    };
    painter.add(shadow.as_shape(rect, RADIUS));
    // The light card, then the picture set into its right with a white margin on three sides, as
    // Animate's is, at the picture's own proportions so nothing is cropped or stretched.
    painter.rect_filled(rect, RADIUS, Color32::WHITE);
    let inset = 10.0 * scale;
    let art_height = rect.height() - 2.0 * inset;
    let art_size = vec2(
        art_height * flare_ui::splash::SIZE.x / flare_ui::splash::SIZE.y,
        art_height,
    );
    let art = Rect::from_min_size(
        Pos2::new(rect.right() - inset - art_size.x, rect.top() + inset),
        art_size,
    );
    flare_ui::splash::paint(ui, art, RADIUS - 2);
    let split = art.left() - inset;
    // The mark: the flame on a dark tile, as Animate's card starts with its own.
    let pad = 32.0 * scale;
    let tile = Rect::from_min_size(rect.min + vec2(pad, pad), vec2(44.0, 44.0) * scale);
    painter.rect_filled(tile, 8.0 * scale, Color32::from_rgb(0x1b, 0x12, 0x30));
    flare_ui::icons::flame(30.0 * scale).paint_at(ui, tile.shrink(7.0 * scale));
    painter.text(
        Pos2::new(tile.right() + 12.0 * scale, tile.center().y),
        Align2::LEFT_CENTER,
        "Flare",
        FontId::proportional(24.0 * scale),
        INK,
    );
    let text_width = split - rect.left() - 2.0 * pad;
    let mut y = tile.bottom() + 22.0 * scale;
    for (text, size, color) in [
        (&words.version, 12.0, DIM),
        (&words.tagline, 12.0, DIM),
        (&words.credit, 11.0, DIM),
    ] {
        let galley = painter.layout(
            text.clone(),
            FontId::proportional(size * scale),
            color,
            text_width,
        );
        let height = galley.size().y;
        painter.galley(Pos2::new(rect.left() + pad, y), galley, color);
        y += height + 8.0 * scale;
    }
    painter.text(
        Pos2::new(rect.left() + pad, rect.bottom() - pad),
        Align2::LEFT_BOTTOM,
        &words.loading,
        FontId::proportional(11.0 * scale),
        DIM,
    );
}
