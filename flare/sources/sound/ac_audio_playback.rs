//! Live playback through the default output device (cpal).

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// A playing mix; dropping it stops the sound.
pub struct Player {
    _stream: cpal::Stream,
    pos: Arc<AtomicUsize>,
    pub rate: u32,
}

/// The output device's sample rate (to mix at), if there is a device.
pub fn output_rate() -> Option<u32> {
    let dev = cpal::default_host().default_output_device()?;
    Some(dev.default_output_config().ok()?.sample_rate().0)
}

impl Player {
    /// Play interleaved stereo `samples` (mixed at `rate`, the device rate).
    pub fn start(samples: Arc<Vec<f32>>, rate: u32) -> Result<Player, String> {
        let host = cpal::default_host();
        let dev = host.default_output_device().ok_or("no audio output device")?;
        let cfg = dev.default_output_config().map_err(|e| e.to_string())?;
        let channels = cfg.channels() as usize;
        let config: cpal::StreamConfig = cpal::StreamConfig { channels: cfg.channels(), sample_rate: cpal::SampleRate(rate), buffer_size: cpal::BufferSize::Default };
        let pos = Arc::new(AtomicUsize::new(0));
        let p2 = pos.clone();
        let stream = dev
            .build_output_stream(
                &config,
                move |out: &mut [f32], _| {
                    let mut i = p2.load(Ordering::Relaxed);
                    for fr in out.chunks_mut(channels) {
                        let (l, r) = if i * 2 + 1 < samples.len() { (samples[i * 2], samples[i * 2 + 1]) } else { (0.0, 0.0) };
                        for (c, s) in fr.iter_mut().enumerate() {
                            *s = if c % 2 == 0 { l } else { r };
                        }
                        i += 1;
                    }
                    p2.store(i, Ordering::Relaxed);
                },
                |e| eprintln!("audio: {e}"),
                None,
            )
            .map_err(|e| e.to_string())?;
        stream.play().map_err(|e| e.to_string())?;
        Ok(Player { _stream: stream, pos, rate })
    }
    /// Seconds played so far.
    pub fn position(&self) -> f32 {
        self.pos.load(Ordering::Relaxed) as f32 / self.rate as f32
    }
}
