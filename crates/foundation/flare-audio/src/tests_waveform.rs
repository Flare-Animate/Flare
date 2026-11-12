//! Excerpts (playing part of a sound, as a scrub does) and the waveform summary.
#![allow(clippy::expect_used)]

use std::sync::Arc;

use super::*;

fn sound(rate: u32, channels: u16, samples: Vec<f32>) -> Arc<Audio> {
    Arc::new(Audio::new(rate, channels, samples).expect("audio"))
}

#[test]
fn an_excerpt_covers_the_frames_it_names_and_stays_inside_the_sound() {
    assert_eq!(Excerpt::WHOLE.frames(100, 50), 0..50);
    assert_eq!(Excerpt::from(0.2).frames(100, 50), 20..50);
    assert_eq!(Excerpt::slice(0.1, 0.05).frames(100, 50), 10..15);
    assert_eq!(
        Excerpt::slice(0.4, 1.0).frames(100, 50),
        40..50,
        "cut at the end"
    );
    assert!(Excerpt::from(0.5).frames(100, 50).is_empty(), "at the end");
    assert!(Excerpt::from(9.0).frames(100, 50).is_empty(), "past it");
    assert_eq!(Excerpt::from(-3.0).frames(100, 50), 0..50, "before it");
    assert_eq!(Excerpt::from(f64::NAN).frames(100, 50), 0..50);
    assert_eq!(Excerpt::slice(0.1, f64::INFINITY).frames(100, 50), 10..50);
    assert!(Excerpt::slice(0.1, -1.0).frames(100, 50).is_empty());
    assert!(Excerpt::from(f64::INFINITY).frames(100, 50).is_empty());
}

#[test]
fn an_excerpt_plays_only_its_frames_and_fades_where_it_cuts_into_the_sound() {
    let mut mixer = Mixer::new(1000);
    // A steady sound of 1000 frames; play 200 of them from the middle.
    mixer.play_excerpt(sound(1000, 1, vec![0.5; 1000]), 0, Excerpt::slice(0.4, 0.2));
    let mut out = vec![0.0_f32; 2 * 300];
    mixer.render(&mut out);
    let left: Vec<f32> = out.chunks(2).map(|f| f[0]).collect();
    assert!(left[0] > 0.0 && left[0] < 0.05, "fades in: {}", left[0]);
    assert!((left[100] - 0.5).abs() < 1e-6, "full in the middle");
    assert!(
        left[199] > 0.0 && left[199] < 0.05,
        "fades out: {}",
        left[199]
    );
    assert!(left[200..].iter().all(|s| *s == 0.0), "and stops after 200");
    assert!(!mixer.is_playing());
}

#[test]
fn a_whole_sound_does_not_fade_and_an_excerpt_past_the_end_plays_nothing() {
    let mut mixer = Mixer::new(100);
    mixer.play_excerpt(sound(100, 1, vec![0.5; 4]), 0, Excerpt::WHOLE);
    let mut out = [0.0_f32; 8];
    mixer.render(&mut out);
    assert_eq!(out, [0.5; 8], "the sound's own ends are not faded");
    assert!(mixer.play_excerpt(sound(100, 1, vec![0.5; 4]), 0, Excerpt::from(1.0)));
    assert!(!mixer.is_playing(), "nothing was left to play");
    assert!(!mixer.play_excerpt(sound(50, 1, vec![0.5; 4]), 0, Excerpt::WHOLE));
}

#[test]
fn peaks_hold_the_lowest_and_highest_sample_of_each_stretch_over_all_channels() {
    // 400 frames of stereo at 400 Hz: two frames a stretch, so 200 stretches.
    let mut samples = vec![0.0_f32; 800];
    samples[0] = 0.25; // frame 0, left
    samples[3] = -0.75; // frame 1, right
    samples[798] = 3.0; // the last frame, clamped to 1
    samples[400] = f32::NAN; // ignored
    let peaks = Peaks::of(&Audio::new(400, 2, samples).expect("audio"));
    assert_eq!(peaks.len(), 200);
    assert_eq!(peaks.between(0.0, 0.004), Some([-0.75, 0.25]));
    assert_eq!(
        peaks.between(0.5, 0.6),
        Some([0.0, 0.0]),
        "silence; NaN is 0"
    );
    assert_eq!(peaks.between(0.99, 1.0), Some([0.0, 1.0]));
    assert_eq!(peaks.between(0.0, 1.0), Some([-0.75, 1.0]), "all of it");
}

#[test]
fn peaks_between_a_tiny_span_give_its_stretch_and_outside_the_sound_give_none() {
    let peaks = Peaks::of(&Audio::new(1000, 1, vec![0.5; 1000]).expect("audio"));
    assert_eq!(peaks.len(), 200, "five frames a stretch");
    assert_eq!(
        peaks.between(0.3001, 0.3002),
        Some([0.0, 0.5]),
        "inside one stretch"
    );
    assert_eq!(peaks.between(1.0, 2.0), None, "past the end");
    assert_eq!(peaks.between(-2.0, -1.0), None, "before the start");
    assert_eq!(
        peaks.between(-1.0, 0.001),
        Some([0.0, 0.5]),
        "overlapping it"
    );
    assert_eq!(peaks.between(f64::NAN, 1.0), None);
    assert_eq!(peaks.between(0.5, 0.4), None, "backwards");
}

#[test]
fn decoding_with_peaks_summarises_the_same_sound() {
    let decoded = Decoded::new(Audio::new(8000, 1, vec![0.5; 8000]).expect("audio"));
    assert_eq!(decoded.audio.frames(), 8000);
    assert_eq!(decoded.peaks.len(), 200);
    assert!(!decoded.peaks.is_empty());
}
