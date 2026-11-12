//! A waveform summary: the lowest and highest sample in each short stretch of a sound, so the
//! timeline can draw a sound's shape at any zoom without reading every sample on each repaint.

use crate::Audio;

/// How many stretches a second of sound is summarised by. Fine enough to see syllables at any
/// zoom the timeline offers, and small: ten minutes are 120,000 pairs.
pub const PEAKS_PER_SECOND: u32 = 200;

/// The lowest and highest sample of each stretch of a sound, over all its channels.
#[derive(Clone, Debug, PartialEq)]
pub struct Peaks {
    /// Seconds each stretch covers.
    stretch_seconds: f64,
    /// `[low, high]` of each stretch, in order, each in -1..=1.
    stretches: Vec<[f32; 2]>,
}

impl Peaks {
    /// The summary of `audio`. Reads every sample once, so it is made where the sound is decoded,
    /// on a worker, never while drawing.
    pub fn of(audio: &Audio) -> Self {
        let per_stretch = (audio.sample_rate() / PEAKS_PER_SECOND).max(1) as usize;
        let channels = usize::from(audio.channels());
        let stretches = audio
            .samples()
            .chunks(per_stretch * channels)
            .map(|chunk| {
                chunk.iter().fold([0.0_f32, 0.0_f32], |[low, high], &s| {
                    let s = if s.is_finite() {
                        s.clamp(-1.0, 1.0)
                    } else {
                        0.0
                    };
                    [low.min(s), high.max(s)]
                })
            })
            .collect();
        Self {
            stretch_seconds: per_stretch as f64 / f64::from(audio.sample_rate()),
            stretches,
        }
    }

    /// How many stretches the summary holds.
    pub fn len(&self) -> usize {
        self.stretches.len()
    }

    pub fn is_empty(&self) -> bool {
        self.stretches.is_empty()
    }

    /// The lowest and highest sample between `from` and `to` seconds into the sound, or `None`
    /// where that lies wholly outside it. A span shorter than one stretch gives the stretch it
    /// falls in, so a column of the timeline is never blank inside the sound when zoomed far in.
    pub fn between(&self, from: f64, to: f64) -> Option<[f32; 2]> {
        if !from.is_finite() || !to.is_finite() || to <= 0.0 || to < from {
            return None;
        }
        let first = (from.max(0.0) / self.stretch_seconds).floor() as usize;
        let last = ((to / self.stretch_seconds).ceil() as usize)
            .max(first + 1)
            .min(self.stretches.len());
        let slice = self.stretches.get(first..last)?;
        if slice.is_empty() {
            return None;
        }
        Some(
            slice
                .iter()
                .fold([0.0_f32, 0.0_f32], |[low, high], [l, h]| {
                    [low.min(*l), high.max(*h)]
                }),
        )
    }
}

/// A decoded sound with its waveform summary, both made on the decoding worker.
#[derive(Clone, Debug)]
pub struct Decoded {
    pub audio: std::sync::Arc<Audio>,
    pub peaks: std::sync::Arc<Peaks>,
}

impl Decoded {
    pub fn new(audio: Audio) -> Self {
        let peaks = Peaks::of(&audio);
        Self {
            audio: std::sync::Arc::new(audio),
            peaks: std::sync::Arc::new(peaks),
        }
    }
}
