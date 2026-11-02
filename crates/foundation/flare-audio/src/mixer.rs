//! Mixing placed sounds into output samples.

use std::sync::Arc;

use crate::Audio;

/// A sound playing in a [`Mixer`].
struct Voice {
    audio: Arc<Audio>,
    /// The output frame at which the sound's first sample plays.
    starts_at: u64,
}

/// Mixes sounds into stereo output at a fixed rate. Sounds must already be at that rate
/// (`Audio::resampled`), because resampling is too slow for the thread that mixes; mono is played
/// in both ears; channels past the second are ignored.
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

    /// Starts `audio` after `delay` output frames from now. False, and nothing starts, if the
    /// sound is not at the mixer's rate.
    pub fn play(&mut self, audio: Arc<Audio>, delay: u64) -> bool {
        if audio.sample_rate() != self.rate {
            return false;
        }
        self.voices.push(Voice {
            audio,
            starts_at: self.position.saturating_add(delay),
        });
        true
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
        for voice in &self.voices {
            let length = voice.audio.frames() as u64;
            let first = voice.starts_at.max(self.position);
            for frame in first..end.min(voice.starts_at + length) {
                let i = (frame - voice.starts_at) as usize;
                let left = voice.audio.at(i, 0);
                let right = if voice.audio.channels() > 1 {
                    voice.audio.at(i, 1)
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
        self.voices
            .retain(|voice| voice.starts_at + voice.audio.frames() as u64 > self.position);
    }
}
