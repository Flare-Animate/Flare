//! Sound: decoding, and a mixer that turns sounds placed on a timeline into output samples.
//!
//! Everything here is pure and testable without a sound card; the device itself is opened by
//! `flare-platform`. Decoding handles WAV,
//! MP3, Ogg Vorbis and FLAC (`symphonia`) with limits on input size and decoded length, so a
//! hostile or huge file is refused instead of exhausting memory.
#![forbid(unsafe_code)]

use std::io::Cursor;
use std::sync::Arc;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use thiserror::Error;

/// Something to do with the sound card. A front end decides what plays when and says so with
/// these; whoever owns the device carries them out (see `flare-platform`).
#[derive(Clone, Debug)]
pub enum AudioEvent {
    /// Silence everything that is playing.
    Stop,
    /// Start a sound after a delay.
    Play {
        audio: Arc<Audio>,
        delay_seconds: f64,
    },
}

/// Largest encoded file accepted.
pub const MAX_INPUT_BYTES: usize = 64 * 1024 * 1024;
/// Most samples (all channels) one decoded sound may have: ten minutes of 48 kHz stereo.
pub const MAX_DECODED_SAMPLES: usize = 57_600_000;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AudioError {
    #[error("the file is larger than {MAX_INPUT_BYTES} bytes")]
    InputTooLarge,
    #[error("the sound is longer than this editor handles (about ten minutes)")]
    TooLong,
    #[error(
        "this is not a sound file Flare can read (WAV, MP3, Ogg Vorbis and FLAC are supported)"
    )]
    Unsupported,
    #[error("the sound file is damaged: {0}")]
    Corrupt(String),
    #[error("the sound has no audio in it")]
    Empty,
}

/// Decoded sound: interleaved 32-bit float samples.
#[derive(Debug, Clone, PartialEq)]
pub struct Audio {
    sample_rate: u32,
    channels: u16,
    samples: Vec<f32>,
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

    pub fn seconds(&self) -> f64 {
        self.frames() as f64 / f64::from(self.sample_rate)
    }

    fn at(&self, frame: usize, channel: usize) -> f32 {
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

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    /// A 16-bit PCM WAV file holding `samples` (interleaved).
    fn wav(sample_rate: u32, channels: u16, samples: &[i16]) -> Vec<u8> {
        let data_len = (samples.len() * 2) as u32;
        let mut out = Vec::new();
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + data_len).to_le_bytes());
        out.extend_from_slice(b"WAVEfmt ");
        out.extend_from_slice(&16_u32.to_le_bytes());
        out.extend_from_slice(&1_u16.to_le_bytes());
        out.extend_from_slice(&channels.to_le_bytes());
        out.extend_from_slice(&sample_rate.to_le_bytes());
        out.extend_from_slice(&(sample_rate * u32::from(channels) * 2).to_le_bytes());
        out.extend_from_slice(&(channels * 2).to_le_bytes());
        out.extend_from_slice(&16_u16.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&data_len.to_le_bytes());
        for s in samples {
            out.extend_from_slice(&s.to_le_bytes());
        }
        out
    }

    fn sound(rate: u32, channels: u16, samples: Vec<f32>) -> Arc<Audio> {
        Arc::new(Audio::new(rate, channels, samples).expect("audio"))
    }

    #[test]
    fn a_wav_file_decodes_to_the_samples_it_holds() {
        let audio = decode(&wav(8000, 2, &[0, 16384, -16384, 32767, 0, 0])).expect("decode");
        assert_eq!(
            (audio.sample_rate(), audio.channels(), audio.frames()),
            (8000, 2, 3)
        );
        assert!((audio.at(0, 1) - 0.5).abs() < 1e-3, "{}", audio.at(0, 1));
        assert!((audio.at(1, 0) + 0.5).abs() < 1e-3);
        assert!((audio.seconds() - 3.0 / 8000.0).abs() < 1e-9);
    }

    #[test]
    fn files_that_are_not_sound_or_are_damaged_or_too_big_are_refused() {
        assert_eq!(
            decode(b"not audio at all, just some text").unwrap_err_kind(),
            AudioError::Unsupported
        );
        assert_eq!(decode(&[]).unwrap_err_kind(), AudioError::Unsupported);
        let big = vec![0_u8; MAX_INPUT_BYTES + 1];
        assert_eq!(decode(&big).unwrap_err_kind(), AudioError::InputTooLarge);
        // A WAV header promising data that is not there decodes to nothing.
        let mut cut = wav(8000, 1, &[1, 2, 3, 4]);
        cut.truncate(44);
        assert!(decode(&cut).is_err());
    }

    trait ErrKind {
        fn unwrap_err_kind(self) -> AudioError;
    }

    impl ErrKind for Result<Audio, AudioError> {
        fn unwrap_err_kind(self) -> AudioError {
            self.expect_err("refused")
        }
    }

    #[test]
    fn audio_needs_samples_in_whole_frames() {
        assert!(Audio::new(44100, 2, vec![0.0; 3]).is_none());
        assert!(Audio::new(0, 1, vec![0.0]).is_none());
        assert!(Audio::new(44100, 0, vec![0.0]).is_none());
        assert!(Audio::new(44100, 1, vec![]).is_none());
    }

    #[test]
    fn a_mono_sound_plays_in_both_ears_after_its_delay() {
        let mut mixer = Mixer::new(100);
        mixer.play(sound(100, 1, vec![0.5, 0.25]), 2);
        let mut out = [9.0_f32; 12]; // six stereo frames
        mixer.render(&mut out);
        assert_eq!(&out[..4], &[0.0, 0.0, 0.0, 0.0], "silent for the delay");
        assert_eq!(&out[4..8], &[0.5, 0.5, 0.25, 0.25]);
        assert_eq!(&out[8..], &[0.0, 0.0, 0.0, 0.0], "and silent once it ends");
        assert!(!mixer.is_playing(), "a finished sound is dropped");
    }

    #[test]
    fn sounds_mix_by_addition_and_clamp_and_pick_up_across_buffers() {
        let mut mixer = Mixer::new(100);
        mixer.play(sound(100, 2, vec![0.75, -0.75, 0.5, 0.5, 0.1, 0.1]), 0);
        mixer.play(sound(100, 1, vec![0.5, 0.5, 0.5]), 0);
        let mut first = [0.0_f32; 4];
        mixer.render(&mut first);
        assert_eq!(
            first,
            [1.0, -0.25, 1.0, 1.0],
            "0.75+0.5 clamps to 1; the rest add"
        );
        let mut second = [0.0_f32; 4];
        mixer.render(&mut second);
        assert!(
            (second[0] - 0.6).abs() < 1e-6,
            "the third frame continues: {second:?}"
        );
        assert_eq!(&second[2..], &[0.0, 0.0], "past the end");
    }

    #[test]
    fn a_sound_of_another_rate_is_resampled_to_the_output_rate() {
        // 50 Hz source in a 100 Hz mixer: each source frame lasts two output frames, and the
        // in-between frame is the average of its neighbours.
        let mut mixer = Mixer::new(100);
        mixer.play(sound(50, 1, vec![0.0, 1.0, 0.0]), 0);
        let mut out = [0.0_f32; 12];
        mixer.render(&mut out);
        let left: Vec<f32> = out.iter().step_by(2).copied().collect();
        assert_eq!(left, vec![0.0, 0.5, 1.0, 0.5, 0.0, 0.0]);
    }

    #[test]
    fn stopping_silences_everything_and_the_clock_keeps_running() {
        let mut mixer = Mixer::new(100);
        mixer.play(sound(100, 1, vec![1.0; 50]), 0);
        mixer.stop_all();
        let mut out = [5.0_f32; 4];
        mixer.render(&mut out);
        assert_eq!(out, [0.0; 4]);
        // A sound started afterwards is delayed from "now", not from zero.
        mixer.play(sound(100, 1, vec![0.5]), 1);
        let mut next = [0.0_f32; 4];
        mixer.render(&mut next);
        assert_eq!(next, [0.0, 0.0, 0.5, 0.5]);
    }
}
