//! The sound card. The only part of this crate that needs a device; everything it plays comes
//! from a [`Mixer`](crate::Mixer), which is tested without one.

use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use thiserror::Error;

use crate::{Audio, Mixer};

#[derive(Debug, Error)]
pub enum OutputError {
    #[error("there is no sound output device")]
    NoDevice,
    #[error("the sound device does not offer 32-bit float output")]
    UnsupportedFormat,
    #[error("cannot open the sound device: {0}")]
    Device(String),
}

/// Plays sounds on the default output device until dropped. Create it once, on the thread that
/// owns the window: the stream is not `Send`.
pub struct Player {
    mixer: Arc<Mutex<Mixer>>,
    rate: u32,
    _stream: cpal::Stream,
}

impl Player {
    pub fn new() -> Result<Self, OutputError> {
        let device = cpal::default_host()
            .default_output_device()
            .ok_or(OutputError::NoDevice)?;
        let config = device
            .default_output_config()
            .map_err(|e| OutputError::Device(e.to_string()))?;
        if config.sample_format() != cpal::SampleFormat::F32 {
            return Err(OutputError::UnsupportedFormat);
        }
        let rate = config.sample_rate().0;
        let channels = usize::from(config.channels()).max(1);
        let mixer = Arc::new(Mutex::new(Mixer::new(rate)));
        let callback_mixer = Arc::clone(&mixer);
        let mut stereo: Vec<f32> = Vec::new();
        let stream = device
            .build_output_stream(
                &config.into(),
                move |out: &mut [f32], _| {
                    let frames = out.len() / channels;
                    stereo.resize(frames * 2, 0.0);
                    // Never wait for the UI thread inside the audio callback: if the mixer is
                    // being changed right now, this buffer is silent.
                    match callback_mixer.try_lock() {
                        Ok(mut mixer) => mixer.render(&mut stereo),
                        Err(_) => stereo.fill(0.0),
                    }
                    for (frame, pair) in out.chunks_mut(channels).zip(stereo.chunks(2)) {
                        for (i, slot) in frame.iter_mut().enumerate() {
                            *slot = if i < 2 { pair[i] } else { 0.0 };
                        }
                    }
                },
                |error| eprintln!("sound output error: {error}"),
                None,
            )
            .map_err(|e| OutputError::Device(e.to_string()))?;
        stream
            .play()
            .map_err(|e| OutputError::Device(e.to_string()))?;
        Ok(Self {
            mixer,
            rate,
            _stream: stream,
        })
    }

    /// Starts `audio` after `delay_seconds`.
    pub fn play(&self, audio: Arc<Audio>, delay_seconds: f64) {
        let delay = (delay_seconds.max(0.0) * f64::from(self.rate)) as u64;
        if let Ok(mut mixer) = self.mixer.lock() {
            mixer.play(audio, delay);
        }
    }

    pub fn stop_all(&self) {
        if let Ok(mut mixer) = self.mixer.lock() {
            mixer.stop_all();
        }
    }
}
