//! Test Movie inside the window: Ruffle plays the published movie on the same GPU device the
//! editor draws with, into a texture that egui shows in a window of its own, so nothing is
//! copied through the CPU and no other program is started.
//!
//! What plays and how is `flare-ruffle`'s business (and tested there); this file puts the
//! player's picture in a window, keeps its clock, and gives its texture back when it closes.

use std::time::{Duration, Instant};

use egui_wgpu::RenderState;
use flare_ruffle::embedded::{EmbedError, EmbeddedMovie, Gpu};

/// The longest stretch of movie time one frame of the window may add. A window that was hidden or
/// a machine that stalled must not make the movie race to catch up.
const MAX_STEP: Duration = Duration::from_millis(100);

/// How much of the screen the picture may take of the window around it.
const MAX_SHARE: f32 = 0.9;

/// The size of one control, and the space between two: the row of three is
/// [`CONTROLS_WIDTH`] wide.
const CONTROL_SIZE: egui::Vec2 = egui::vec2(104.0, 34.0);
const CONTROL_GAP: f32 = 8.0;
const CONTROLS_WIDTH: f32 = CONTROL_SIZE.x * 3.0 + CONTROL_GAP * 2.0;
const CONTROLS_MARGIN: f32 = 80.0;

/// One control: an icon beside its name. A dimmed one is a thing the movie cannot do now.
fn control(ui: &mut egui::Ui, icon: &str, label: &str, enabled: bool) -> bool {
    let tint = ui.visuals().text_color();
    let button = egui::Button::image_and_text(flare_ui::icons::image(icon, 18.0, tint), label)
        .min_size(CONTROL_SIZE);
    ui.add_enabled(enabled, button).clicked()
}

/// The Close button, at the right edge. True when it was pressed.
fn close_row(ui: &mut egui::Ui) -> bool {
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        control(ui, "close", "Close", true)
    })
    .inner
}

/// Movie time to add for `since` of real time.
pub(crate) fn step(since: Duration) -> Duration {
    since.min(MAX_STEP)
}

/// The size to show a picture of `picture` points in a space of `room` points: as it is when it
/// fits, shrunk to fit otherwise, never enlarged.
pub(crate) fn fit(picture: egui::Vec2, room: egui::Vec2) -> egui::Vec2 {
    let room = room * MAX_SHARE;
    if picture.x <= 0.0 || picture.y <= 0.0 {
        return egui::Vec2::ZERO;
    }
    let scale = (room.x / picture.x).min(room.y / picture.y).min(1.0);
    picture * scale.max(0.0)
}

/// An open Test Movie window.
pub(crate) struct MovieWindow {
    movie: EmbeddedMovie,
    swf: Vec<u8>,
    state: RenderState,
    texture: egui::TextureId,
    last_tick: Instant,
    scale: f32,
    failure: Option<String>,
    /// The sound is switched off.
    muted: bool,
    closed: bool,
}

impl MovieWindow {
    /// Starts `swf` on the GPU `state` belongs to, drawn sharply for a screen of `scale` pixels
    /// per point.
    pub(crate) fn open(swf: Vec<u8>, state: &RenderState, scale: f32) -> Result<Self, EmbedError> {
        let gpu = Gpu {
            instance: state.instance.clone(),
            adapter: state.adapter.clone(),
            device: state.device.clone(),
            queue: state.queue.clone(),
        };
        let movie = EmbeddedMovie::start(&swf, &gpu, f64::from(scale), true)?;
        let view = movie.texture().create_view(&Default::default());
        let texture = state.renderer.write().register_native_texture(
            &state.device,
            &view,
            egui_wgpu::wgpu::FilterMode::Linear,
        );
        Ok(Self {
            movie,
            swf,
            state: state.clone(),
            texture,
            last_tick: Instant::now(),
            scale,
            failure: None,
            muted: false,
            closed: false,
        })
    }

    /// Whether the person has closed the window. It is dropped on the next frame, not this one,
    /// because this frame's picture may still name its texture.
    pub(crate) fn is_closed(&self) -> bool {
        self.closed
    }

    /// Draws the window and advances the movie.
    pub(crate) fn show(&mut self, ctx: &egui::Context) {
        let now = Instant::now();
        let elapsed = step(now.duration_since(self.last_tick));
        self.last_tick = now;
        if self.failure.is_none()
            && let Err(error) = self.movie.tick(elapsed)
        {
            self.failure = Some(error.to_string());
        }

        let mut open = true;
        let (mut close, mut restart, mut toggle_play, mut toggle_mute) =
            (false, false, false, false);
        let room = ctx.content_rect().size();
        egui::Window::new("Test Movie")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                if let Some(failure) = &self.failure {
                    ui.label(failure);
                    ui.add_space(8.0);
                    close = close_row(ui);
                    return;
                }
                let (width, height) = self.movie.size();
                let picture = egui::vec2(width as f32, height as f32) / self.scale;
                let size = fit(picture, room);
                // Room around the row, so it reads as centered and Close sits clear of it.
                ui.set_min_width(size.x.max(CONTROLS_WIDTH + CONTROLS_MARGIN));
                ui.vertical_centered(|ui| {
                    ui.add(egui::Image::new(egui::load::SizedTexture::new(
                        self.texture,
                        size,
                    )));
                });
                ui.add_space(10.0);
                // The controls are one row in the middle, whatever the picture's width.
                ui.horizontal(|ui| {
                    ui.add_space(((ui.available_width() - CONTROLS_WIDTH) / 2.0).max(0.0));
                    ui.spacing_mut().item_spacing.x = CONTROL_GAP;
                    restart = control(ui, "transport-first", "Restart", true);
                    let playing = self.movie.is_playing();
                    let (icon, label) = if playing {
                        ("transport-pause", "Pause")
                    } else {
                        ("transport-play", "Play")
                    };
                    toggle_play = control(ui, icon, label, true);
                    let (icon, label) = if self.muted {
                        ("sound-mute", "Unmute")
                    } else {
                        ("sound", "Mute")
                    };
                    toggle_mute = control(ui, icon, label, self.movie.has_sound());
                });
                ui.add_space(8.0);
                ui.separator();
                close = close_row(ui);
            });
        if restart {
            self.restart();
        }
        if toggle_play {
            let _ = self.movie.set_playing(!self.movie.is_playing());
        }
        if toggle_mute {
            self.muted = !self.muted;
            let _ = self.movie.set_muted(self.muted);
        }
        self.closed = !open || close;
        // Movies advance with the clock, so keep drawing while one is on screen.
        ctx.request_repaint();
    }

    fn restart(&mut self) {
        let gpu = Gpu {
            instance: self.state.instance.clone(),
            adapter: self.state.adapter.clone(),
            device: self.state.device.clone(),
            queue: self.state.queue.clone(),
        };
        match EmbeddedMovie::start(&self.swf, &gpu, f64::from(self.scale), true) {
            Ok(movie) => {
                let view = movie.texture().create_view(&Default::default());
                self.state
                    .renderer
                    .write()
                    .update_egui_texture_from_wgpu_texture(
                        &self.state.device,
                        &view,
                        egui_wgpu::wgpu::FilterMode::Linear,
                        self.texture,
                    );
                // A new movie starts with its sound on; the person's choice is kept.
                let _ = movie.set_muted(self.muted);
                self.movie = movie;
                self.last_tick = Instant::now();
            }
            Err(error) => self.failure = Some(error.to_string()),
        }
    }
}

impl Drop for MovieWindow {
    fn drop(&mut self) {
        self.state.renderer.write().free_texture(&self.texture);
    }
}

#[cfg(test)]
#[path = "movie_tests.rs"]
mod tests;
