#![allow(clippy::expect_used)]

use super::*;

/// A second of a constant 0.5 in both ears, at 1 kHz so a test can count samples.
fn steady() -> Arc<Audio> {
    Arc::new(Audio::new(1000, 2, vec![0.5; 2000]).expect("audio"))
}

fn buffer(engine: &mut Engine, frames: usize) -> Vec<f32> {
    let mut out = vec![0.0; frames * 2];
    engine.render(&mut out);
    out
}

#[test]
fn a_sound_asked_for_is_heard_in_the_next_buffer_and_not_before() {
    let (sender, mut engine) = channel(1000);
    assert!(
        buffer(&mut engine, 10).iter().all(|s| *s == 0.0),
        "silence to begin with"
    );
    assert!(sender.play(steady(), 0.0));
    let out = buffer(&mut engine, 10);
    assert!(
        out.iter().all(|s| (*s - 0.5).abs() < 1e-6),
        "heard at once: {out:?}"
    );
}

#[test]
fn a_delay_is_counted_in_samples_from_the_buffer_it_arrives_in() {
    let (sender, mut engine) = channel(1000);
    assert!(sender.play(steady(), 0.004));
    let out = buffer(&mut engine, 10);
    let quiet = out.chunks(2).take_while(|pair| pair[0] == 0.0).count();
    assert_eq!(quiet, 4, "four samples of silence, then the sound");
    assert!(out[8..].iter().all(|s| *s > 0.4));
}

#[test]
fn commands_are_carried_out_in_the_order_they_were_given() {
    let (sender, mut engine) = channel(1000);
    sender.play(steady(), 0.0);
    sender.stop_all();
    assert!(
        buffer(&mut engine, 10).iter().all(|s| *s == 0.0),
        "stopped before it was heard"
    );
    sender.play(steady(), 0.0);
    sender.stop_all();
    sender.play(steady(), 0.0);
    let out = buffer(&mut engine, 4);
    assert!(
        out.iter().all(|s| (*s - 0.5).abs() < 1e-6),
        "only the last one is playing, not two on top of each other: {out:?}"
    );
}

#[test]
fn stopping_ends_a_sound_that_is_already_playing() {
    let (sender, mut engine) = channel(1000);
    sender.play(steady(), 0.0);
    assert!(buffer(&mut engine, 10).iter().all(|s| *s > 0.4));
    sender.stop_all();
    assert!(buffer(&mut engine, 10).iter().all(|s| *s == 0.0));
}

#[test]
fn a_full_queue_drops_the_newest_command_and_never_makes_the_window_wait() {
    let (sender, mut engine) = channel(1000);
    let started = std::time::Instant::now();
    let accepted = (0..QUEUE * 3)
        .filter(|_| sender.play(steady(), 0.0))
        .count();
    assert!(
        started.elapsed() < std::time::Duration::from_secs(1),
        "no waiting"
    );
    assert_eq!(accepted, QUEUE, "exactly what fits");
    assert!(!sender.stop_all(), "and nothing more, not even a stop");
    // The engine takes what is there and the queue has room again.
    let out = buffer(&mut engine, 4);
    assert!(out.iter().all(|s| *s > 0.4), "{out:?}");
    assert!(sender.stop_all());
}

#[test]
fn the_window_can_ask_while_a_buffer_is_being_made_on_another_thread() {
    // The case the old lock lost buffers to: asking at the same moment as the callback runs.
    let (sender, mut engine) = channel(1000);
    let audio = steady();
    let callback = std::thread::spawn(move || {
        let mut silent_for_good = 0;
        let mut heard = 0;
        for _ in 0..20_000 {
            let out = buffer(&mut engine, 8);
            if out.iter().all(|s| *s == 0.0) {
                silent_for_good += 1;
            } else {
                heard += 1;
            }
        }
        (silent_for_good, heard)
    });
    let mut refused = 0;
    for _ in 0..5_000 {
        if !sender.play(Arc::clone(&audio), 0.0) {
            refused += 1;
        }
        std::thread::yield_now();
    }
    let (_, heard) = callback.join().expect("the callback ran to the end");
    assert!(heard > 0, "sounds reached the callback (refused {refused})");
}
