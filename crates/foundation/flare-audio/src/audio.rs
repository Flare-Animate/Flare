//! A decoded sound, and decoding one from a file.

use std::io::Cursor;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use crate::{AudioError, MAX_DECODED_SAMPLES, MAX_INPUT_BYTES};

/// Decoded sound: interleaved 32-bit float samples.
#[derive(Debug, Clone, PartialEq)]
pub struct Audio {
    sample_rate: u32,
    channels: u16,
    pub(crate) samples: Vec<f32>,
}

impl Audio {
    pub fn new(sample_rate: u32, channels: u16, samples: Vec<f32>) -> Option<Self> {
        (sample_rate > 0
            && channels > 0
            && !samples.is_empty()
            && samples.len().is_multiple_of(usize::from(channels)))
        .then_some(Self {
            sample_rate,
            channels,
            samples,
        })
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn channels(&self) -> u16 {
        self.channels
    }

    /// Samples per channel.
    pub fn frames(&self) -> usize {
        self.samples.len() / usize::from(self.channels)
    }

    /// How many samples (all channels) the sound holds.
    pub fn samples_len(&self) -> usize {
        self.samples.len()
    }

    pub fn seconds(&self) -> f64 {
        self.frames() as f64 / f64::from(self.sample_rate)
    }

    /// The sound as 16-bit signed samples at `rate`, interleaved, in one channel if it has one
    /// and in two otherwise (a sound with more is cut to its first two, as the mixer does).
    /// Resampled as [`Audio::resampled`] does, and clamped to what 16 bits hold. Returns the
    /// samples and how many channels they have. `None` if `rate` is zero or the result would
    /// have more than `max_samples` samples.
    pub fn to_pcm16(&self, rate: u32, max_samples: usize) -> Option<(Vec<i16>, u16)> {
        let channels = usize::from(self.channels.min(2));
        let out_frames =
            (self.frames() as u128 * u128::from(rate)).div_ceil(u128::from(self.sample_rate));
        if usize::try_from(out_frames).ok()?.checked_mul(channels)? > max_samples {
            return None;
        }
        let sound = self.resampled(rate)?;
        let out = sound
            .samples
            .iter()
            .map(|s| (s * 32767.0).round().clamp(-32768.0, 32767.0) as i16)
            .collect();
        Some((out, sound.channels))
    }

    pub(crate) fn at(&self, frame: usize, channel: usize) -> f32 {
        self.samples[frame * usize::from(self.channels) + channel]
    }
}

/// Decodes a WAV, MP3, Ogg Vorbis or FLAC file.
pub fn decode(bytes: &[u8]) -> Result<Audio, AudioError> {
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(AudioError::InputTooLarge);
    }
    let stream = MediaSourceStream::new(Box::new(Cursor::new(bytes.to_vec())), Default::default());
    let probed = symphonia::default::get_probe()
        .format(
            &Hint::new(),
            stream,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|_| AudioError::Unsupported)?;
    let mut format = probed.format;
    let track = format.default_track().ok_or(AudioError::Unsupported)?;
    let track_id = track.id;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|_| AudioError::Unsupported)?;
    let mut rate = track.codec_params.sample_rate;
    let mut channels = track
        .codec_params
        .channels
        .map(|c| u16::try_from(c.count()).unwrap_or(0));
    let mut samples: Vec<f32> = Vec::new();
    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(SymphoniaError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                break;
            }
            Err(SymphoniaError::ResetRequired) => break,
            Err(e) => return Err(AudioError::Corrupt(e.to_string())),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            // One bad packet is skipped, as players do; a file that is all bad ends up empty.
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(e) => return Err(AudioError::Corrupt(e.to_string())),
        };
        let spec = *decoded.spec();
        rate = Some(spec.rate);
        channels = Some(u16::try_from(spec.channels.count()).unwrap_or(0));
        let mut buffer = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
        buffer.copy_interleaved_ref(decoded);
        if samples.len() + buffer.samples().len() > MAX_DECODED_SAMPLES {
            return Err(AudioError::TooLong);
        }
        samples.extend_from_slice(buffer.samples());
    }
    let (Some(rate), Some(channels)) = (rate, channels) else {
        return Err(AudioError::Empty);
    };
    Audio::new(rate, channels, samples).ok_or(AudioError::Empty)
}
