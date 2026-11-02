//! Changing a sound's sample rate. The work is `rubato`'s FFT resampler, which is exact for a
//! fixed ratio; it runs over the whole sound, so it belongs on a worker thread, never in the
//! audio callback (see `flare-platform`, which does it before a sound is queued).

use rubato::audioadapter_buffers::direct::InterleavedSlice;
use rubato::{Fft, FixedSync, Resampler};

use crate::{Audio, MAX_DECODED_SAMPLES};

/// Frames per call into the resampler; only a trade between call overhead and memory.
const CHUNK: usize = 1024;

impl Audio {
    /// The sound at `rate`, in at most two channels (the mixer plays no more). The same sound if
    /// the rate is already `rate`. `None` if `rate` is zero or the result would have more than
    /// [`MAX_DECODED_SAMPLES`] samples, so a tiny input rate cannot grow without bound.
    pub fn resampled(&self, rate: u32) -> Option<Audio> {
        if rate == 0 {
            return None;
        }
        let channels = usize::from(self.channels().min(2));
        let frames = self.frames();
        let out_frames =
            (frames as u128 * u128::from(rate)).div_ceil(u128::from(self.sample_rate()));
        if usize::try_from(out_frames).ok()?.checked_mul(channels)? > MAX_DECODED_SAMPLES {
            return None;
        }
        if rate == self.sample_rate() && usize::from(self.channels()) == channels {
            return Some(self.clone());
        }
        // The stored samples may have more channels than are kept; take the first ones.
        let stride = usize::from(self.channels());
        let kept: Vec<f32> = if stride == channels {
            self.samples.clone()
        } else {
            self.samples
                .chunks_exact(stride)
                .flat_map(|frame| frame[..channels].iter().copied())
                .collect()
        };
        if rate == self.sample_rate() {
            return Audio::new(rate, channels as u16, kept);
        }
        let mut resampler = Fft::<f32>::new(
            self.sample_rate() as usize,
            rate as usize,
            CHUNK,
            channels,
            FixedSync::Input,
        )
        .ok()?;
        let input = InterleavedSlice::new(&kept, channels, frames).ok()?;
        let output = resampler.process_all(&input, frames, None).ok()?;
        Audio::new(rate, channels as u16, output.take_data())
    }
}
