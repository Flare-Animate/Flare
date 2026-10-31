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
        let room = ctx.content_rect().size();
        egui::Window::new("Test Movie")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                if let Some(failure) = &self.failure {
                    ui.label(failure);
                } else {
                    let (width, height) = self.movie.size();
                    let picture = egui::vec2(width as f32, height as f32) / self.scale;
                    let size = fit(picture, room);
                    ui.add(egui::Image::new(egui::load::SizedTexture::new(
                        self.texture,
                        size,
                    )));
                    ui.horizontal(|ui| {
                        let playing = self.movie.is_playing();
                        if ui.button(if playing { "Pause" } else { "Play" }).clicked() {
                            let _ = self.movie.set_playing(!playing);
                        }
                        if ui.button("Restart").clicked() {
                            self.restart();
                        }
                    });
                }
            });
        self.closed = !open;
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
