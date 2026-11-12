//! The waveform's cost: summarising a sound (once, on the decoding worker) and reading one
//! timeline row's columns from the summary (every repaint while the sound is in view).
//!
//! The sound is the longest the decoder keeps, ten minutes of 48 kHz stereo, built outside the
//! timed section. A row is 1600 columns, a wide timeline panel.
#![allow(clippy::expect_used, missing_docs)]

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use flare_audio::{Audio, Peaks};

const COLUMNS: u32 = 1600;

fn longest() -> Audio {
    let frames = 48_000 * 600;
    let samples = (0..frames * 2)
        .map(|n| ((n % 977) as f32 / 977.0) * 2.0 - 1.0)
        .collect();
    Audio::new(48_000, 2, samples).expect("audio")
}

fn row(peaks: &Peaks, seconds_per_column: f64) -> f32 {
    (0..COLUMNS)
        .filter_map(|c| {
            let from = f64::from(c) * seconds_per_column;
            peaks.between(from, from + seconds_per_column)
        })
        .map(|[low, high]| high - low)
        .sum()
}

fn waveform(c: &mut Criterion) {
    let audio = longest();
    c.bench_function("summarise ten minutes of stereo", |b| {
        b.iter(|| Peaks::of(black_box(&audio)))
    });
    let peaks = Peaks::of(&audio);
    // The default zoom: 10 points a frame at 24 fps.
    c.bench_function("one row at the default zoom", |b| {
        b.iter(|| row(black_box(&peaks), 1.0 / 240.0))
    });
    // Zoomed all the way out with the whole ten minutes across the row: the most stretches a
    // column ever reads.
    c.bench_function("one row with ten minutes across it", |b| {
        b.iter(|| row(black_box(&peaks), 600.0 / f64::from(COLUMNS)))
    });
}

criterion_group!(benches, waveform);
criterion_main!(benches);
