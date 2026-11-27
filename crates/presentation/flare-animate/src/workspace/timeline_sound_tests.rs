#![allow(clippy::expect_used)]

use super::*;
use flare_audio::Audio;

/// Half a second of silence, then half a second at full scale, at 1 kHz.
fn half_quiet() -> Peaks {
    let mut samples = vec![0.0_f32; 500];
    samples.extend((0..500).map(|i| if i % 2 == 0 { 1.0 } else { -1.0 }));
    Peaks::of(&Audio::new(1000, 1, samples).expect("audio"))
}

fn segments(shapes: &[Shape]) -> Vec<[Pos2; 2]> {
    shapes
        .iter()
        .map(|shape| match shape {
            Shape::LineSegment { points, .. } => *points,
            other => panic!("a line expected, not {other:?}"),
        })
        .collect()
}

#[test]
fn the_waveform_has_one_column_a_point_and_follows_the_sound() {
    // A bar 100 points wide and 20 high for the whole second: 10 ms a point.
    let bar = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(100.0, 20.0));
    let lines = segments(&waveform(bar, bar, &half_quiet(), 0.01, Stroke::NONE));
    assert_eq!(lines.len(), 100);
    let quiet = lines[10];
    assert!(
        (quiet[1].y - quiet[0].y - 1.0).abs() < 1e-4,
        "silence is a one-point line: {quiet:?}"
    );
    let loud = lines[80];
    assert!(
        (loud[0].y - 1.0).abs() < 1e-4 && (loud[1].y - 19.0).abs() < 1e-4,
        "full scale fills the bar but for a point each side: {loud:?}"
    );
}

#[test]
fn only_columns_on_screen_and_inside_the_sound_are_drawn() {
    let bar = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(1000.0, 20.0));
    let screen = Rect::from_min_max(Pos2::new(200.0, -50.0), Pos2::new(260.0, 50.0));
    let lines = segments(&waveform(bar, screen, &half_quiet(), 0.001, Stroke::NONE));
    assert_eq!(
        lines.len(),
        60,
        "the 60 points in view, not the 1000 of the bar"
    );
    assert!(lines.iter().all(|l| l[0].x >= 200.0 && l[0].x <= 260.0));
    // The bar runs on past the sound's one second: nothing is drawn there.
    let long = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(200.0, 20.0));
    assert_eq!(
        waveform(long, long, &half_quiet(), 0.01, Stroke::NONE).len(),
        100
    );
}

#[test]
fn nothing_is_drawn_off_screen_or_at_a_degenerate_scale() {
    let bar = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(100.0, 20.0));
    let away = Rect::from_min_size(Pos2::new(500.0, 0.0), Vec2::new(100.0, 20.0));
    assert!(waveform(bar, away, &half_quiet(), 0.01, Stroke::NONE).is_empty());
    for scale in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(
            waveform(bar, bar, &half_quiet(), scale, Stroke::NONE).is_empty(),
            "{scale}"
        );
    }
}

/// A second of a steady tone at `level` of full scale, at 1 kHz.
fn tone(level: f32) -> Peaks {
    let samples = (0..1000)
        .map(|i| if i % 2 == 0 { level } else { -level })
        .collect();
    Peaks::of(&Audio::new(1000, 1, samples).expect("audio"))
}

#[test]
fn a_quiet_sound_is_drawn_as_large_as_a_loud_one_but_hiss_stays_small() {
    let bar = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(100.0, 20.0));
    let height = |peaks: &Peaks| {
        let line = segments(&waveform(bar, bar, peaks, 0.01, Stroke::NONE))[50];
        line[1].y - line[0].y
    };
    // A soft recording, as a song mixed quietly is: its loudest moment reaches the edges.
    assert!(
        (height(&tone(0.15)) - 18.0).abs() < 1e-3,
        "{}",
        height(&tone(0.15))
    );
    assert!((height(&tone(1.0)) - 18.0).abs() < 1e-3);
    // Near-silence is enlarged only twenty times, so it does not pass for sound: a twenty-fifth
    // of full scale reaches four fifths of the way out, and hiss stays a one-point line.
    assert!(
        (height(&tone(0.04)) - 14.4).abs() < 1e-3,
        "{}",
        height(&tone(0.04))
    );
    assert!((height(&tone(0.001)) - 1.0).abs() < 1e-3);
}
