//! Test Movie in a window of its own: Ruffle plays the published movie on the same GPU device
//! the editor draws with, into a texture that egui shows, so nothing is copied through the CPU
//! and no other program is started.
//!
//! What plays and how is `flare-ruffle`'s business (and tested there); this file puts the
//! player's picture and controls in a window's contents, keeps its clock, and gives its texture
//! back when it closes. The window itself is in `movie_window.rs`.

use std::time::{Duration, Instant};

use egui_wgpu::RenderState;
use flare_ruffle::embedded::{EmbedError, EmbeddedMovie, Gpu};

mod words;

pub(crate) use words::MovieWords;

/// The longest stretch of movie time one frame of the window may add. A window that was hidden or
/// a machine that stalled must not make the movie race to catch up.
const MAX_STEP: Duration = Duration::from_millis(100);

/// The height of the bar of controls under the picture.
pub(crate) const BAR_HEIGHT: f32 = 40.0;

/// The playback speeds on offer. Sound plays at normal speed only, so at the others it is off.
const SPEEDS: [(f32, &str); 4] = [(0.25, "0.25×"), (0.5, "0.5×"), (1.0, "1×"), (2.0, "2×")];

/// The narrowest the window may be: the controls and the clock side by side.
pub(crate) const MIN_WIDTH: f32 = 520.0;

/// One small icon button. The name is its tooltip; a dimmed one is a thing the movie cannot do
/// now, and a lit one is switched on.
fn tool(ui: &mut egui::Ui, icon: &str, tip: &str, enabled: bool, on: bool) -> bool {
    let tint = ui.visuals().text_color();
    let button = egui::Button::image(flare_ui::icons::image(icon, 14.0, tint))
        .min_size(egui::vec2(26.0, 22.0))
        .selected(on);
    ui.add_enabled(enabled, button).on_hover_text(tip).clicked()
}

/// The size, in points, to open the window at: the picture at its own size with the bar under it,
/// at least [`MIN_WIDTH`] wide, and no bigger than `room` (most of the screen). A picture bigger
/// than the room is shrunk to fit when it is drawn.
pub(crate) fn open_size(picture: egui::Vec2, room: egui::Vec2) -> egui::Vec2 {
    egui::vec2(
        picture.x.max(MIN_WIDTH).min(room.x.max(MIN_WIDTH)),
        (picture.y + BAR_HEIGHT).min(room.y.max(BAR_HEIGHT)),
    )
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
pub(crate) fn fit_in(picture: egui::Vec2, room: egui::Vec2) -> egui::Vec2 {
    if picture.x <= 0.0 || picture.y <= 0.0 || room.x <= 0.0 || room.y <= 0.0 {
        return egui::Vec2::ZERO;
    }
    picture * (room.x / picture.x).min(room.y / picture.y).min(1.0)
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
    /// Whether the movie starts over at its end; otherwise it holds its last frame.
    looping: bool,
    /// How much movie time has passed since it started, as the controls tell it.
    elapsed: Duration,
    /// How wide the box of controls was last time, to put it in the middle.
    box_width: f32,
    closed: bool,
    /// What the controls say, in the interface's language.
    words: MovieWords,
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
            looping: true,
            elapsed: Duration::ZERO,
            box_width: 0.0,
            closed: false,
            words: MovieWords::english(),
        })
    }

    /// Has the window speak with `words`; it starts in English.
    pub(crate) fn set_words(&mut self, words: MovieWords) {
        self.words = words;
    }

    /// Whether the person has closed the player with its Close button. The window drops it on the
    /// next frame, not this one, because this frame's picture may still name its texture.
    pub(crate) fn is_closed(&self) -> bool {
        self.closed
    }

    /// The picture's size in points, for opening a window to fit it.
    pub(crate) fn picture_points(&self) -> egui::Vec2 {
        let (width, height) = self.movie.size();
        egui::vec2(width as f32, height as f32) / self.scale
    }

    /// Draws the player into the window's contents: the picture, and under it a bar with the clock
    /// at the left, the controls in one box in the middle and Close at the right. Advances the
    /// movie first.
    pub(crate) fn show(&mut self, ui: &mut egui::Ui) {
        let now = Instant::now();
        let elapsed = scaled(now.duration_since(self.last_tick), self.speed);
        self.last_tick = now;
        self.advance(elapsed);

        let (mut close, mut restart, mut toggle_play, mut step_frame) =
            (false, false, false, false);
        let (mut toggle_mute, mut toggle_loop) = (false, false);
        let mut speed = self.speed;
        egui::Panel::bottom("movie_controls")
            .exact_size(BAR_HEIGHT)
            .show(ui, |ui| {
                let bar = ui.max_rect().width();
                ui.horizontal_centered(|ui| {
                    ui.add_space(10.0);
                    let frame = self.movie.current_frame().unwrap_or(0);
                    let clock = ui.label(
                        egui::RichText::new(self.words.clock_line(&clock(self.elapsed), frame))
                            .monospace()
                            .weak(),
                    );
                    // The box is centered in the bar, not in what the clock leaves.
                    let before = 10.0 + clock.rect.width();
                    ui.add_space(((bar - self.box_width) / 2.0 - before).max(0.0));
                    let boxed = egui::Frame::group(ui.style())
                        .inner_margin(egui::Margin::symmetric(6, 2))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 4.0;
                                restart =
                                    tool(ui, "transport-first", &self.words.restart, true, false);
                                let playing = self.movie.is_playing();
                                let (icon, tip) = if playing {
                                    ("transport-pause", &self.words.pause)
                                } else {
                                    ("transport-play", &self.words.play)
                                };
                                toggle_play = tool(ui, icon, tip, true, false);
                                step_frame =
                                    tool(ui, "transport-next", &self.words.step, !playing, false);
                                ui.separator();
                                let current = SPEEDS
                                    .iter()
                                    .find(|(value, _)| *value == self.speed)
                                    .map_or("1×", |(_, label)| *label);
                                egui::ComboBox::from_id_salt("movie_speed")
                                    .width(60.0)
                                    .selected_text(current)
                                    .show_ui(ui, |ui| {
                                        for (value, label) in SPEEDS {
                                            ui.selectable_value(&mut speed, value, label);
                                        }
                                    })
                                    .response
                                    .on_hover_text(&self.words.speed);
                                toggle_loop = tool(
                                    ui,
                                    "transport-loop",
                                    if self.looping {
                                        &self.words.loop_on
                                    } else {
                                        &self.words.loop_off
                                    },
                                    true,
                                    self.looping,
                                );
                                let (icon, tip) = if self.muted {
                                    ("sound-mute", &self.words.sound_off)
                                } else {
                                    ("sound", &self.words.sound_on)
                                };
                                toggle_mute =
                                    tool(ui, icon, tip, self.movie.has_sound(), self.muted);
                            });
                        });
                    self.box_width = boxed.response.rect.width();
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(8.0);
                        let tint = ui.visuals().text_color();
                        close = ui
                            .add(egui::Button::image_and_text(
                                flare_ui::icons::image("close", 12.0, tint),
                                &self.words.close,
                            ))
                            .clicked();
                    });
                });
            });
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(egui::Color32::from_gray(18)))
            .show(ui, |ui| {
                if let Some(failure) = &self.failure {
                    ui.centered_and_justified(|ui| ui.label(failure));
                    return;
                }
                let size = fit_in(self.picture_points(), ui.available_size());
                let at = egui::Rect::from_center_size(ui.max_rect().center(), size);
                ui.put(
                    at,
                    egui::Image::new(egui::load::SizedTexture::new(self.texture, size)),
                );
            });
        if restart {
            self.restart();
        }
        if toggle_play {
            self.toggle_play();
        }
        if toggle_loop {
            self.looping = !self.looping;
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
        self.closed |= close;
    }

    /// Lets `elapsed` of movie time pass. Without looping, the movie is paused as it reaches its
    /// last frame, so it stays on it.
    fn advance(&mut self, elapsed: Duration) {
        if self.failure.is_some() {
            return;
        }
        match self.movie.tick(elapsed) {
            Ok(()) => {
                if self.movie.is_playing() {
                    self.elapsed += elapsed;
                    if !self.looping && self.at_end() {
                        let _ = self.movie.set_playing(false);
                    }
                }
            }
            Err(error) => self.failure = Some(error.to_string()),
        }
    }

    /// Whether the movie is showing its last frame.
    fn at_end(&self) -> bool {
        self.movie
            .current_frame()
            .is_some_and(|frame| frame >= self.movie.frame_count())
    }

    /// Pause or play. Play at the end of a movie that does not loop starts it over.
    fn toggle_play(&mut self) {
        if !self.movie.is_playing() && !self.looping && self.at_end() {
            self.restart();
        } else {
            let _ = self.movie.set_playing(!self.movie.is_playing());
        }
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
