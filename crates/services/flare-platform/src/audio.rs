//! The sound card. The only part of this crate that needs a device; everything it plays comes
//! from a [`Mixer`], which is tested without one.

use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use thiserror::Error;

use flare_audio::{Audio, AudioEvent, Excerpt, Mixer};

use crate::audio_prepared::Prepared;

#[derive(Debug, Error)]
pub enum OutputError {
    #[error("there is no sound output device")]
    NoDevice,
    #[error("the sound device does not offer 32-bit float output")]
    UnsupportedFormat,
    #[error("cannot open the sound device: {0}")]
    Device(String),
}

/// What the window asks of the sound card, queued without a lock.
enum Command {
    Play {
        audio: Arc<Audio>,
        delay: u64,
        excerpt: Excerpt,
    },
    Stop,
}

/// How many commands can wait for the next buffer. A person cannot start this many sounds in the
/// few milliseconds between buffers; a full queue drops the newest rather than make anyone wait.
const QUEUE: usize = 256;

/// The window's end of the queue.
pub(crate) struct Sender {
    commands: Mutex<rtrb::Producer<Command>>,
    rate: u32,
}

/// The audio callback's end: the mixer, which nothing else touches, and the commands waiting for
/// it. Rendering takes no lock and waits for nobody, so a sound being started on the window
/// thread can never make a buffer drop out.
pub(crate) struct Engine {
    mixer: Mixer,
    commands: rtrb::Consumer<Command>,
}

/// A queue and the engine at its other end, for sound at `rate`.
pub(crate) fn channel(rate: u32) -> (Sender, Engine) {
    let (producer, consumer) = rtrb::RingBuffer::new(QUEUE);
    (
        Sender {
            commands: Mutex::new(producer),
            rate,
        },
        Engine {
            mixer: Mixer::new(rate),
            commands: consumer,
        },
    )
}

impl Sender {
    fn push(&self, command: Command) -> bool {
        // Only the window thread ever takes this lock; the audio callback never sees it.
        self.commands
            .lock()
            .is_ok_and(|mut commands| commands.push(command).is_ok())
    }

    /// Starts `audio` after `delay_seconds`. False if the queue was full and the sound dropped.
    #[cfg(test)]
    pub(crate) fn play(&self, audio: Arc<Audio>, delay_seconds: f64) -> bool {
        self.play_excerpt(audio, delay_seconds, Excerpt::WHOLE)
    }

    /// Starts the `excerpt` of `audio` after `delay_seconds`. False if the queue was full.
    pub(crate) fn play_excerpt(
        &self,
        audio: Arc<Audio>,
        delay_seconds: f64,
        excerpt: Excerpt,
    ) -> bool {
        // The mixer takes only sounds at its rate; see `Prepared` for the others.
        if audio.sample_rate() != self.rate {
            return false;
        }
        let delay = (delay_seconds.max(0.0) * f64::from(self.rate)) as u64;
        self.push(Command::Play {
            audio,
            delay,
            excerpt,
        })
    }

    pub(crate) fn rate(&self) -> u32 {
        self.rate
    }

    pub(crate) fn stop_all(&self) -> bool {
        self.push(Command::Stop)
    }
}

impl Engine {
    /// Fills `out` (interleaved stereo) with the next buffer: first what was asked since the last
    /// one, in the order it was asked, then the sound.
    pub(crate) fn render(&mut self, out: &mut [f32]) {
        while let Ok(command) = self.commands.pop() {
            match command {
                Command::Play {
                    audio,
                    delay,
                    excerpt,
                } => {
                    self.mixer.play_excerpt(audio, delay, excerpt);
                }
                Command::Stop => self.mixer.stop_all(),
            }
        }
        self.mixer.render(out);
    }
}

/// Plays sounds on the default output device until dropped. Create it once, on the thread that
/// owns the window: the stream is not `Send`.
pub struct Player {
    sounds: Prepared,
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
        let (sender, mut engine) = channel(rate);
        let sounds = Prepared::new(Arc::new(sender));
        let mut stereo: Vec<f32> = Vec::new();
        let stream = device
            .build_output_stream(
                &config.into(),
                move |out: &mut [f32], _| {
                    let frames = out.len() / channels;
                    stereo.resize(frames * 2, 0.0);
                    engine.render(&mut stereo);
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
            sounds,
            _stream: stream,
        })
    }

    /// Starts `audio` after `delay_seconds`.
    pub fn play(&self, audio: Arc<Audio>, delay_seconds: f64) {
        self.sounds.play(audio, delay_seconds, Excerpt::WHOLE);
    }

    /// Carries out what a front end asked for.
    pub fn apply(&self, events: impl IntoIterator<Item = AudioEvent>) {
        for event in events {
            match event {
                AudioEvent::Stop => self.stop_all(),
                AudioEvent::Play {
                    audio,
                    delay_seconds,
                    excerpt,
                } => self.sounds.play(audio, delay_seconds, excerpt),
            }
        }
    }

    pub fn stop_all(&self) {
        self.sounds.stop_all();
    }
}

#[cfg(test)]
#[path = "audio_tests.rs"]
mod tests;
