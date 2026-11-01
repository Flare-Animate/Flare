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

/// The playback speeds on offer. Sound plays at normal speed only, so at the others it is off.
const SPEEDS: [(f32, &str); 4] = [(0.25, "0.25×"), (0.5, "0.5×"), (1.0, "1×"), (2.0, "2×")];

/// The room the controls need beside the picture's own width.
const MIN_WIDTH: f32 = 330.0;

/// One small icon button. The name is its tooltip; a dimmed one is a thing the movie cannot do
/// now, and a lit one is switched on.
fn tool(ui: &mut egui::Ui, icon: &str, tip: &str, enabled: bool, on: bool) -> bool {
    let tint = ui.visuals().text_color();
    let button = egui::Button::image(flare_ui::icons::image(icon, 14.0, tint))
        .min_size(egui::vec2(26.0, 22.0))
        .selected(on);
    ui.add_enabled(enabled, button).on_hover_text(tip).clicked()
}

/// The Close button, at the right edge. True when it was pressed.
fn close_row(ui: &mut egui::Ui) -> bool {
    // One row high: a right-to-left layout on its own takes all the height there is.
    let row = egui::vec2(ui.available_width(), ui.spacing().interact_size.y);
    ui.allocate_ui_with_layout(
        row,
        egui::Layout::right_to_left(egui::Align::Center),
        |ui| {
            let tint = ui.visuals().text_color();
            ui.add(egui::Button::image_and_text(
                flare_ui::icons::image("close", 12.0, tint),
                "Close",
            ))
            .clicked()
        },
    )
    .inner
}

/// Movie time as the controls show it: minutes, seconds and tenths.
pub(crate) fn clock(time: Duration) -> String {
    let tenths = time.as_millis() / 100;
    format!("{}:{:02}.{}", tenths / 600, tenths / 10 % 60, tenths % 10)
}

/// Movie time to add for `since` of real time at `speed` times normal.
pub(crate) fn scaled(since: Duration, speed: f32) -> Duration {
    step(since).mul_f32(speed.clamp(0.0, 16.0))
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
    /// How fast the movie runs, 1 being normal.
    speed: f32,
    /// How much movie time has passed since it started, as the controls tell it.
    elapsed: Duration,
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
            speed: 1.0,
            elapsed: Duration::ZERO,
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
        let elapsed = scaled(now.duration_since(self.last_tick), self.speed);
        self.last_tick = now;
        if self.failure.is_none() {
            match self.movie.tick(elapsed) {
                Ok(()) => {
                    if self.movie.is_playing() {
                        self.elapsed += elapsed;
                    }
                }
                Err(error) => self.failure = Some(error.to_string()),
            }
        }

        let mut open = true;
        let (mut close, mut restart, mut toggle_play, mut step_frame, mut toggle_mute) =
            (false, false, false, false, false);
        let room = ctx.content_rect().size();
        let mut speed = self.speed;
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
                ui.set_min_width(size.x.max(MIN_WIDTH));
                ui.vertical_centered(|ui| {
                    ui.add(egui::Image::new(egui::load::SizedTexture::new(
                        self.texture,
                        size,
                    )));
                    ui.add_space(8.0);
                    // The controls share one box, in the middle under the picture.
                    egui::Frame::group(ui.style())
                        .inner_margin(egui::Margin::symmetric(8, 4))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 4.0;
                                restart = tool(ui, "transport-first", "Restart", true, false);
                                let playing = self.movie.is_playing();
                                let (icon, tip) = if playing {
                                    ("transport-pause", "Pause")
                                } else {
                                    ("transport-play", "Play")
                                };
                                toggle_play = tool(ui, icon, tip, true, false);
                                step_frame =
                                    tool(ui, "transport-next", "Step one frame", !playing, false);
                                ui.separator();
                                for (value, label) in SPEEDS {
                                    ui.selectable_value(&mut speed, value, label);
                                }
                                ui.separator();
                                let (icon, tip) = if self.muted {
                                    ("sound-mute", "Sound is off; click to turn it on")
                                } else {
                                    ("sound", "Sound is on; click to turn it off")
                                };
                                toggle_mute =
                                    tool(ui, icon, tip, self.movie.has_sound(), self.muted);
                                ui.separator();
                                let frame = self.movie.current_frame().unwrap_or(0);
                                ui.label(
                                    egui::RichText::new(format!(
                                        "{}  frame {frame}",
                                        clock(self.elapsed)
                                    ))
                                    .monospace(),
                                );
                            });
                        });
                });
                ui.add_space(6.0);
                ui.separator();
                close = close_row(ui);
            });
        if restart {
            self.restart();
        }
        if toggle_play {
            let _ = self.movie.set_playing(!self.movie.is_playing());
        }
        if step_frame && self.movie.step_frame().is_ok() {
            self.elapsed += Duration::from_secs_f64(1.0 / self.movie.frame_rate().max(1.0));
        }
        if toggle_mute {
            self.muted = !self.muted;
        }
        if speed != self.speed {
            self.speed = speed;
        }
        // Sound plays at normal speed only: at any other it would be out of step with the picture.
        let _ = self.movie.set_muted(self.muted || self.speed != 1.0);
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
                let _ = movie.set_muted(self.muted || self.speed != 1.0);
                self.movie = movie;
                self.elapsed = Duration::ZERO;
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
