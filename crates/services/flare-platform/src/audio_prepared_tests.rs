#![allow(clippy::expect_used)]

use super::*;
use crate::audio::channel;

/// Two seconds of a constant 0.5 in both ears at 8 kHz, for a 1 kHz device.
fn other_rate() -> Arc<Audio> {
    Arc::new(Audio::new(8000, 2, vec![0.5; 32000]).expect("audio"))
}

fn heard_within(engine: &mut crate::audio::Engine, limit: Duration) -> bool {
    let end = Instant::now() + limit;
    while Instant::now() < end {
        let mut out = vec![0.0; 20];
        engine.render(&mut out);
        if out.iter().any(|s| s.abs() > 0.1) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    false
}

#[test]
fn a_sound_of_the_devices_rate_is_queued_at_once() {
    let (sender, mut engine) = channel(1000);
    let prepared = Prepared::new(Arc::new(sender));
    prepared.play(
        Arc::new(Audio::new(1000, 1, vec![0.5; 100]).expect("audio")),
        0.0,
        Excerpt::WHOLE,
    );
    let mut out = vec![0.0; 20];
    engine.render(&mut out);
    assert!(out.iter().all(|s| (*s - 0.5).abs() < 1e-6), "{out:?}");
}

#[test]
fn a_sound_of_another_rate_is_heard_once_resampled_and_replays_from_the_kept_copy() {
    let (sender, mut engine) = channel(1000);
    let prepared = Prepared::new(Arc::new(sender));
    let sound = other_rate();
    prepared.play(sound.clone(), 0.0, Excerpt::WHOLE);
    assert!(
        heard_within(&mut engine, Duration::from_secs(15)),
        "the first play arrives after the resampling"
    );
    assert_eq!(lock(&prepared.state).kept.len(), 1);
    prepared.stop_all();
    let mut out = vec![0.0; 20];
    engine.render(&mut out);
    prepared.play(sound, 0.0, Excerpt::WHOLE);
    let mut out = vec![0.0; 20];
    engine.render(&mut out);
    assert!(
        out.iter().any(|s| s.abs() > 0.1),
        "a second play needs no waiting: {out:?}"
    );
    assert_eq!(lock(&prepared.state).kept.len(), 1, "one copy, not two");
}

#[test]
fn an_excerpt_of_a_resampled_sound_plays_the_same_seconds_of_it() {
    let (sender, mut engine) = channel(1000);
    let prepared = Prepared::new(Arc::new(sender));
    // A second of silence, then a second of 0.5, at 8 kHz for a 1 kHz device.
    let mut samples = vec![0.0; 8000];
    samples.extend(std::iter::repeat_n(0.5, 8000));
    let sound = Arc::new(Audio::new(8000, 1, samples).expect("audio"));
    prepared.play(sound.clone(), 0.0, Excerpt::slice(0.0, 0.5));
    assert!(
        !heard_within(&mut engine, Duration::from_secs(2)),
        "the silent half second"
    );
    prepared.play(sound, 0.0, Excerpt::from(1.25));
    assert!(
        heard_within(&mut engine, Duration::from_secs(15)),
        "the loud second, though the copy has another rate"
    );
}

#[test]
fn stopping_forgets_a_play_that_is_still_waiting() {
    let (sender, mut engine) = channel(1000);
    let prepared = Prepared::new(Arc::new(sender));
    // Long enough that the resampling is still running when the stop arrives.
    let long = Arc::new(Audio::new(8000, 2, vec![0.5; 8000 * 2 * 60]).expect("audio"));
    prepared.play(long, 0.0, Excerpt::WHOLE);
    prepared.stop_all();
    assert!(
        !heard_within(&mut engine, Duration::from_secs(2)),
        "a stopped play must not start later"
    );
}

#[test]
fn the_kept_copies_are_bounded() {
    let mut state = State::default();
    let big = |n: usize| Arc::new(Audio::new(1000, 1, vec![0.0; n]).expect("audio"));
    for _ in 0..4 {
        state.remember(big(1), big(KEPT_SAMPLES / 2));
    }
    assert!(state.kept.len() <= 3, "{}", state.kept.len());
    state.remember(big(1), big(KEPT_SAMPLES * 2));
    assert_eq!(state.kept.len(), 1, "the newest is always kept");
}
