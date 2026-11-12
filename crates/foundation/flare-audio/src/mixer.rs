//! Mixing placed sounds into output samples.

use std::sync::Arc;

use crate::{Audio, Excerpt};

/// Frames over which an excerpt that cuts into a sound fades in and out, so the cut does not
/// click: about a millisecond and a half at 44.1 kHz.
const EDGE_FADE: usize = 64;

/// A sound playing in a [`Mixer`].
struct Voice {
    audio: Arc<Audio>,
    /// The output frame at which the frame `from` of the sound plays.
    starts_at: u64,
    /// The frames of the sound that play.
    from: usize,
    until: usize,
}

impl Voice {
    fn length(&self) -> u64 {
        (self.until - self.from) as u64
    }

    /// The gain of the sound's frame `i`: full, except near an end that cuts into the sound.
    fn gain(&self, i: usize) -> f32 {
        let mut gain = 1.0_f32;
        if self.from > 0 {
            gain = gain.min((i - self.from + 1) as f32 / EDGE_FADE as f32);
        }
        if self.until < self.audio.frames() {
            gain = gain.min((self.until - i) as f32 / EDGE_FADE as f32);
        }
        gain.min(1.0)
    }
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
        self.play_excerpt(audio, delay, Excerpt::WHOLE)
    }

    /// Starts the `excerpt` of `audio` after `delay` output frames from now. False, and nothing
    /// starts, if the sound is not at the mixer's rate; an excerpt past the sound's end is
    /// accepted and plays nothing.
    pub fn play_excerpt(&mut self, audio: Arc<Audio>, delay: u64, excerpt: Excerpt) -> bool {
        if audio.sample_rate() != self.rate {
            return false;
        }
        let range = excerpt.frames(self.rate, audio.frames());
        if range.is_empty() {
            return true;
        }
        self.voices.push(Voice {
            audio,
            starts_at: self.position.saturating_add(delay),
            from: range.start,
            until: range.end,
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
            let first = voice.starts_at.max(self.position);
            for frame in first..end.min(voice.starts_at + voice.length()) {
                let i = voice.from + (frame - voice.starts_at) as usize;
                let gain = voice.gain(i);
                let left = voice.audio.at(i, 0) * gain;
                let right = if voice.audio.channels() > 1 {
                    voice.audio.at(i, 1) * gain
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
            .retain(|voice| voice.starts_at + voice.length() > self.position);
    }
}
