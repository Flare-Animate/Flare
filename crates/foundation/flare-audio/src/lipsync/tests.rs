#![allow(clippy::expect_used)]

use super::*;
use crate::{Audio, decode, wav_pcm16};

const RATE: u32 = 16_000;

/// `seconds` of a sine of `hz` at amplitude `level`, mono.
fn tone(hz: f32, level: f32, seconds: f32) -> Vec<f32> {
    let n = (RATE as f32 * seconds) as usize;
    (0..n)
        .map(|i| (std::f32::consts::TAU * hz * i as f32 / RATE as f32).sin() * level)
        .collect()
}

fn silence(seconds: f32) -> Vec<f32> {
    vec![0.0; (RATE as f32 * seconds) as usize]
}

/// A hiss: a fixed pseudo-random sequence, so the test is the same every run.
fn hiss(seconds: f32) -> Vec<f32> {
    let mut state = 0x2545_f491_u32;
    (0..(RATE as f32 * seconds) as usize)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            (state as f32 / u32::MAX as f32 - 0.5) * 0.8
        })
        .collect()
}

fn audio(parts: &[Vec<f32>]) -> Audio {
    Audio::new(RATE, 1, parts.concat()).expect("audio")
}

#[test]
fn every_shape_has_a_name_that_reads_back_and_its_place_in_the_list() {
    for (index, viseme) in Viseme::ALL.into_iter().enumerate() {
        assert_eq!(Viseme::parse(viseme.name()), Some(viseme));
        assert_eq!(
            Viseme::parse(&format!(" {} ", viseme.name().to_uppercase())),
            Some(viseme)
        );
        assert_eq!(viseme.index(), index);
    }
    assert_eq!(Viseme::parse("ah"), None);
}

#[test]
fn silence_is_a_resting_mouth_and_every_frame_of_the_sound_has_a_shape() {
    let shapes = analyze(&audio(&[silence(1.0)]), 24.0).expect("analysis");
    assert_eq!(shapes.len(), 24);
    assert!(shapes.iter().all(|s| *s == Viseme::Neutral), "{shapes:?}");
    // The last frame is counted when the sound only reaches into it.
    let shapes = analyze(&audio(&[silence(0.51)]), 10.0).expect("analysis");
    assert_eq!(shapes.len(), 6);
}

#[test]
fn a_low_voice_rounds_the_mouth_a_middle_one_opens_it_and_a_hiss_is_f() {
    let sound = audio(&[
        silence(0.5),
        tone(220.0, 0.6, 0.5),
        tone(1000.0, 0.6, 0.5),
        tone(2500.0, 0.6, 0.5),
        hiss(0.5),
    ]);
    let shapes = analyze(&sound, 10.0).expect("analysis");
    assert_eq!(shapes.len(), 25);
    let middle = |second: usize| shapes[second * 5 + 2];
    assert_eq!(middle(0), Viseme::Neutral);
    assert_eq!(middle(1), Viseme::O);
    assert_eq!(middle(2), Viseme::Ai);
    assert_eq!(middle(3), Viseme::E);
    assert_eq!(middle(4), Viseme::Fv);
}

#[test]
fn a_quiet_low_voice_is_a_small_round_mouth() {
    let sound = audio(&[tone(220.0, 0.6, 0.5), tone(220.0, 0.2, 0.5)]);
    let shapes = analyze(&sound, 10.0).expect("analysis");
    assert_eq!(shapes[2], Viseme::O);
    assert_eq!(shapes[7], Viseme::U);
}

#[test]
fn one_silent_frame_between_sounds_closes_the_lips_and_a_one_frame_flicker_goes() {
    let sound = audio(&[
        tone(220.0, 0.6, 0.3),
        silence(0.1),
        tone(220.0, 0.6, 0.3),
        tone(1000.0, 0.6, 0.1),
        tone(220.0, 0.6, 0.3),
    ]);
    let shapes = analyze(&sound, 10.0).expect("analysis");
    assert_eq!(shapes[3], Viseme::Mbp, "{shapes:?}");
    assert_eq!(
        shapes[7],
        Viseme::O,
        "the one frame of another shape is dropped: {shapes:?}"
    );
}

#[test]
fn a_stereo_voice_is_read_as_its_two_channels_together() {
    let mono = tone(220.0, 0.6, 0.5);
    let stereo: Vec<f32> = mono.iter().flat_map(|s| [*s, *s]).collect();
    let a = analyze(&Audio::new(RATE, 1, mono).expect("mono"), 10.0).expect("mono");
    let b = analyze(&Audio::new(RATE, 2, stereo).expect("stereo"), 10.0).expect("stereo");
    assert_eq!(a, b);
}

#[test]
fn a_rate_that_is_not_positive_or_a_sound_too_long_for_it_is_refused() {
    let sound = audio(&[silence(0.1)]);
    assert_eq!(analyze(&sound, 0.0), Err(LipSyncError::BadRate));
    assert_eq!(analyze(&sound, f64::NAN), Err(LipSyncError::BadRate));
    assert_eq!(analyze(&sound, 1e7), Err(LipSyncError::TooLong));
    assert_eq!(
        frames_from_cues(&[], 24.0, MAX_LIP_SYNC_FRAMES + 1),
        Err(LipSyncError::TooLong)
    );
    assert_eq!(frames_from_cues(&[], -1.0, 3), Err(LipSyncError::BadRate));
}

#[test]
fn rhubarb_cues_become_one_shape_a_frame_with_gaps_at_rest() {
    let json = br#"{"metadata":{"soundFile":"v.wav","duration":0.6},"mouthCues":[
        {"start":0.20,"end":0.35,"value":"D"},{"start":0.10,"end":0.20,"value":"A"},
        {"start":0.35,"end":0.50,"value":"F"},{"start":0.00,"end":0.10,"value":"X"}]}"#;
    let cues = parse_rhubarb(json).expect("cues");
    assert_eq!(cues.len(), 4);
    let frames = frames_from_cues(&cues, 20.0, 12).expect("frames");
    use Viseme::*;
    assert_eq!(
        frames,
        vec![
            Neutral, Neutral, Mbp, Mbp, Ai, Ai, Ai, U, U, U, Neutral, Neutral
        ]
    );
}

#[test]
fn every_rhubarb_shape_has_one_of_ours_and_others_do_not() {
    let mapped: Vec<Viseme> = "ABCDEFGHX".chars().filter_map(rhubarb_shape).collect();
    assert_eq!(mapped.len(), 9);
    assert_eq!(rhubarb_shape('g'), Some(Viseme::Fv));
    assert_eq!(rhubarb_shape('Z'), None);
}

#[test]
fn rhubarb_output_that_is_not_what_it_should_be_is_refused_with_the_reason() {
    let bad: [&[u8]; 7] = [
        b"not json",
        b"{}",
        br#"{"mouthCues":[{"start":0.1,"value":"A"}]}"#,
        br#"{"mouthCues":[{"start":0.5,"end":0.1,"value":"A"}]}"#,
        br#"{"mouthCues":[{"start":-1,"end":0.1,"value":"A"}]}"#,
        br#"{"mouthCues":[{"start":0,"end":0.1,"value":"Q"}]}"#,
        br#"{"mouthCues":[{"start":0,"end":0.1,"value":"AB"}]}"#,
    ];
    for text in bad {
        let error = parse_rhubarb(text).expect_err("refused");
        assert!(matches!(error, LipSyncError::Rhubarb(_)), "{error:?}");
        assert!(error.to_string().starts_with("Rhubarb"), "{error}");
    }
    let huge = vec![b' '; MAX_RHUBARB_OUTPUT_BYTES + 1];
    assert!(parse_rhubarb(&huge).is_err());
}

#[test]
fn a_wav_written_for_an_analysis_tool_decodes_back_to_the_same_sound() {
    let samples: Vec<i16> = (0..800).map(|i| ((i % 50) * 400 - 10_000) as i16).collect();
    let bytes = wav_pcm16(&samples, 8000, 1).expect("wav");
    let back = decode(&bytes).expect("decode");
    assert_eq!(
        (back.sample_rate(), back.channels(), back.frames()),
        (8000, 1, 800)
    );
    assert!((back.samples()[1] - f32::from(samples[1]) / 32768.0).abs() < 1e-4);
    assert_eq!(wav_pcm16(&samples, 8000, 0), None);
}

#[test]
fn the_same_voice_gives_the_same_shapes_at_any_sample_rate() {
    let at = |rate: u32| {
        let n = rate as usize / 2;
        let wave = |hz: f32| {
            (0..n).map(move |i| (std::f32::consts::TAU * hz * i as f32 / rate as f32).sin() * 0.6)
        };
        let samples: Vec<f32> = wave(220.0)
            .chain(wave(1000.0))
            .chain(wave(2500.0))
            .collect();
        analyze(&Audio::new(rate, 1, samples).expect("audio"), 10.0).expect("analysis")
    };
    assert_eq!(at(16_000), at(48_000));
}
