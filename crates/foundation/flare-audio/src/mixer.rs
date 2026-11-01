//! Mixing placed sounds into output samples.

use std::sync::Arc;

use crate::Audio;

/// A sound playing in a [`Mixer`].
struct Voice {
    audio: Arc<Audio>,
    /// The output frame at which the sound's first sample plays.
    starts_at: u64,
}

/// Mixes sounds into stereo output at a fixed rate. Sounds of another rate are resampled by
/// linear interpolation; mono is played in both ears; channels past the second are ignored.
pub struct Mixer {
    rate: u32,
    /// The next output frame to be produced.
    position: u64,
    voices: Vec<Voice>,
}

impl Mixer {
    pub fn new(rate: u32) -> Self {
        Self {
            rate: rate.max(1),
            position: 0,
            voices: Vec::new(),
        }
    }

    pub fn rate(&self) -> u32 {
        self.rate
    }

    /// Starts `audio` after `delay` output frames from now.
    pub fn play(&mut self, audio: Arc<Audio>, delay: u64) {
        self.voices.push(Voice {
            audio,
            starts_at: self.position.saturating_add(delay),
        });
    }

    pub fn stop_all(&mut self) {
        self.voices.clear();
    }

    pub fn is_playing(&self) -> bool {
        !self.voices.is_empty()
    }

    /// Fills `out` with interleaved stereo frames and moves on. Sounds that have finished are
    /// dropped. Output is clamped to the range a device accepts.
    pub fn render(&mut self, out: &mut [f32]) {
        out.fill(0.0);
        let frames = (out.len() / 2) as u64;
        let end = self.position + frames;
        let rate = f64::from(self.rate);
        for voice in &self.voices {
            let step = f64::from(voice.audio.sample_rate()) / rate;
            let length = voice.audio.frames();
            let first = voice.starts_at.max(self.position);
            for frame in first..end.min(voice.starts_at + (length as f64 / step).ceil() as u64) {
                let t = (frame - voice.starts_at) as f64 * step;
                let i = t as usize;
                if i >= length {
                    break;
                }
                let fraction = (t - i as f64) as f32;
                let j = (i + 1).min(length - 1);
                let sample = |channel: usize| {
                    let a = voice.audio.at(i, channel);
                    let b = voice.audio.at(j, channel);
                    a + (b - a) * fraction
                };
                let left = sample(0);
                let right = if voice.audio.channels() > 1 {
                    sample(1)
                } else {
                    left
                };
                let slot = ((frame - self.position) * 2) as usize;
                out[slot] += left;
                out[slot + 1] += right;
            }
        }
        for value in out.iter_mut() {
            *value = value.clamp(-1.0, 1.0);
        }
        self.position = end;
        self.voices.retain(|voice| {
            let length =
                voice.audio.frames() as f64 / (f64::from(voice.audio.sample_rate()) / rate);
            (voice.starts_at as f64 + length) > self.position as f64
        });
    }
}
