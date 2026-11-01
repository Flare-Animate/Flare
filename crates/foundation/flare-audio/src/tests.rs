#![allow(clippy::expect_used)]

use super::*;

#[test]
fn pcm16_keeps_the_rate_and_clamps() {
    let audio = Audio::new(8000, 1, vec![0.0, 0.5, -0.5, 2.0, -2.0]).expect("audio");
    let (pcm, channels) = audio.to_pcm16(8000, 100).expect("pcm");
    assert_eq!(channels, 1);
    assert_eq!(pcm, vec![0, 16384, -16384, 32767, -32768]);
}

#[test]
fn pcm16_resamples_and_cuts_to_two_channels() {
    let frames = 100;
    let samples: Vec<f32> = (0..frames * 3).map(|n| (n % 3) as f32 * 0.1).collect();
    let audio = Audio::new(22050, 3, samples).expect("audio");
    let (pcm, channels) = audio.to_pcm16(11025, 1000).expect("pcm");
    assert_eq!(channels, 2);
    assert_eq!(pcm.len(), 50 * 2);
    assert_eq!(pcm[0], 0);
    assert_eq!(pcm[1], 3277);
}

#[test]
fn pcm16_refuses_a_zero_rate_and_too_many_samples() {
    let audio = Audio::new(8000, 2, vec![0.0; 20]).expect("audio");
    assert!(audio.to_pcm16(0, 100).is_none());
    assert!(audio.to_pcm16(8000, 19).is_none());
    assert!(audio.to_pcm16(8000, 20).is_some());
}

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
