//! Ruffle running inside the app: a movie plays on a wgpu device the app already has, into a
//! texture the app can show, and its sound goes to the sound card.
//!
//! Unlike [`crate::launch_test_movie`], nothing here starts another process. The player is
//! Ruffle's own core and its wgpu renderer, unmodified; this module is the glue: build a player
//! over a movie held in memory, advance it by the time that passed, draw it, and hand back the
//! texture. A foreign engine runs inside the app's process, so a panic in it is caught and the
//! movie is marked failed instead of taking the editor down with it.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

pub use ruffle_core::backend::audio::AudioBackend;
use ruffle_core::backend::locale::DeterministicLocaleBackend;
use ruffle_core::tag_utils::SwfMovieData;
use ruffle_core::{FloatDuration, Player, PlayerBuilder};
use ruffle_frontend_utils::backends::audio::CpalAudioBackend;
use ruffle_render_wgpu::backend::WgpuRenderBackend;
use ruffle_render_wgpu::backend::{create_wgpu_instance, request_adapter_and_device};
use ruffle_render_wgpu::descriptors::Descriptors;
use ruffle_render_wgpu::target::TextureTarget;
pub use ruffle_render_wgpu::wgpu;
use thiserror::Error;

/// The most pixels along a side the movie is drawn at, whatever its stage says.
pub const MAX_SIDE_PX: u32 = 4096;

#[derive(Debug, Error)]
pub enum EmbedError {
    #[error("not a SWF Ruffle can read: {0}")]
    Movie(String),
    #[error("the movie's stage is {0} x {1} px, which is too big to draw")]
    TooBig(u32, u32),
    #[error("Ruffle could not draw on this device: {0}")]
    Gpu(String),
    #[error("Ruffle stopped after an internal error")]
    Failed,
}

/// The graphics device Ruffle draws with: the app's own, so the picture never leaves the GPU.
#[derive(Clone)]
pub struct Gpu {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

impl Gpu {
    /// A device of our own, for when there is no window to share one with (tests, stills).
    pub fn headless() -> Result<Self, EmbedError> {
        let backends = wgpu::Backends::PRIMARY;
        let instance = create_wgpu_instance(backends, wgpu::BackendOptions::default(), None);
        let (adapter, device, queue) = futures_lite::future::block_on(request_adapter_and_device(
            backends,
            &instance,
            None,
            wgpu::PowerPreference::default(),
        ))
        .map_err(|e| EmbedError::Gpu(e.to_string()))?;
        Ok(Self {
            instance,
            adapter,
            device,
            queue,
        })
    }
}

/// A movie playing in the app.
pub struct EmbeddedMovie {
    player: Arc<Mutex<Player>>,
    texture: wgpu::Texture,
    size: (u32, u32),
    sound: bool,
    failed: bool,
}

impl EmbeddedMovie {
    /// Starts `swf` playing on `gpu`, drawn at the stage's own size times `scale`. With `sound`
    /// the movie plays through the default output device; if there is none, it plays silently.
    pub fn start(swf: &[u8], gpu: &Gpu, scale: f64, sound: bool) -> Result<Self, EmbedError> {
        let audio = if sound {
            CpalAudioBackend::new(None)
                .ok()
                .map(|audio| Box::new(audio) as Box<dyn AudioBackend>)
        } else {
            None
        };
        Self::start_with_audio(swf, gpu, scale, audio)
    }

    /// [`Self::start`] with the sound going to `audio` (none: silence). The app uses the sound
    /// card; a test can use a mixer it reads from.
    pub fn start_with_audio(
        swf: &[u8],
        gpu: &Gpu,
        scale: f64,
        audio: Option<Box<dyn AudioBackend>>,
    ) -> Result<Self, EmbedError> {
        let movie = SwfMovieData::from_data(swf, "file:///flare/movie.swf".to_owned(), None, None)
            .map_err(|e| EmbedError::Movie(e.to_string()))?;
        let scale = if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        };
        let side = |pixels: f64| (pixels * scale).round().clamp(1.0, f64::from(u32::MAX)) as u32;
        let size = (
            side(movie.width().to_pixels()),
            side(movie.height().to_pixels()),
        );
        if size.0 > MAX_SIDE_PX || size.1 > MAX_SIDE_PX {
            return Err(EmbedError::TooBig(size.0, size.1));
        }
        let descriptors = Arc::new(Descriptors::new(
            gpu.instance.clone(),
            gpu.adapter.clone(),
            gpu.device.clone(),
            gpu.queue.clone(),
        ));
        let target =
            TextureTarget::new(&gpu.device, size).map_err(|e| EmbedError::Gpu(e.to_string()))?;
        let texture = target.get_texture();
        let renderer = WgpuRenderBackend::new(descriptors, target)
            .map_err(|e| EmbedError::Gpu(e.to_string()))?;
        let mut builder = PlayerBuilder::new()
            .with_renderer(renderer)
            .with_locale(DeterministicLocaleBackend::default())
            .with_movie(movie)
            .with_autoplay(true)
            .with_viewport_dimensions(size.0, size.1, scale);
        let has_sound = audio.is_some();
        if let Some(audio) = audio {
            builder = builder.with_boxed_audio(audio);
        }
        let player =
            catch_unwind(AssertUnwindSafe(|| builder.build())).map_err(|_| EmbedError::Failed)?;
        Ok(Self {
            player,
            texture,
            size,
            sound: has_sound,
            failed: false,
        })
    }

    fn lock(&self) -> Result<MutexGuard<'_, Player>, EmbedError> {
        self.player.lock().map_err(|_| EmbedError::Failed)
    }

    /// Lets `elapsed` of movie time pass, then draws the result if it changed.
    pub fn tick(&mut self, elapsed: Duration) -> Result<(), EmbedError> {
        if self.failed {
            return Err(EmbedError::Failed);
        }
        let ran = catch_unwind(AssertUnwindSafe(|| {
            let mut player = self.lock()?;
            player.tick(FloatDuration::from_std(elapsed));
            if player.needs_render() {
                player.render();
            }
            Ok(())
        }));
        match ran {
            Ok(result) => result,
            Err(_) => {
                self.failed = true;
                Err(EmbedError::Failed)
            }
        }
    }

    /// The texture the movie is drawn into (RGBA, not sRGB-encoded), `size()` pixels.
    pub fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }

    pub fn size(&self) -> (u32, u32) {
        self.size
    }

    /// Whether the movie is making sound (there was an output device and it was asked for).
    pub fn has_sound(&self) -> bool {
        self.sound
    }

    /// Whether the movie is still advancing.
    pub fn is_playing(&self) -> bool {
        self.lock()
            .map(|player| player.is_playing())
            .unwrap_or(false)
    }

    pub fn set_playing(&self, playing: bool) -> Result<(), EmbedError> {
        // Pausing talks to the sound card, which can refuse; that must not take the app down.
        catch_unwind(AssertUnwindSafe(|| {
            self.lock().map(|mut player| player.set_is_playing(playing))
        }))
        .map_err(|_| EmbedError::Failed)?
    }

    /// The picture as it is now, `size()` RGBA bytes rows, read back from the GPU. For tests and
    /// for stills; showing the movie uses [`Self::texture`] and never reads back.
    pub fn read_pixels(&self) -> Result<Vec<u8>, EmbedError> {
        let mut player = self.lock()?;
        let renderer = <dyn std::any::Any>::downcast_mut::<WgpuRenderBackend<TextureTarget>>(
            player.renderer_mut(),
        )
        .ok_or(EmbedError::Failed)?;
        renderer
            .capture_frame()
            .map(|image| image.into_raw())
            .ok_or(EmbedError::Failed)
    }
}

impl Drop for EmbeddedMovie {
    fn drop(&mut self) {
        // Stop sound before the player goes: its audio stream outlives a paused player otherwise.
        let _ = self.set_playing(false);
    }
}
