//! The Magic Wand's cost against the size of the bitmap: picking (through neighbours, and across
//! the picture) and the soft edge a clear leaves. A picture of a disc on a plain ground, made
//! outside the timed section; the pick starts on the ground, as taking off a background does.
#![allow(clippy::expect_used, missing_docs)]

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use flare_core::BitmapId;
use flare_document::Bitmap;
use flare_tools::magic_wand::{coverage, pick};

fn disc(w: u32, h: u32) -> Bitmap {
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    let (cx, cy, r) = (i64::from(w / 2), i64::from(h / 2), i64::from(h / 3));
    for y in 0..i64::from(h) {
        for x in 0..i64::from(w) {
            let inside = (x - cx).pow(2) + (y - cy).pow(2) < r * r;
            rgba.extend_from_slice(if inside {
                &[200, 40, 40, 255]
            } else {
                &[250, 250, 250, 255]
            });
        }
    }
    Bitmap::new(BitmapId::new(), "disc", w, h, rgba).expect("bitmap")
}

fn wand(c: &mut Criterion) {
    let mut group = c.benchmark_group("magic_wand");
    group.sample_size(10);
    for (w, h) in [(1920, 1080), (4096, 4096)] {
        let picture = disc(w, h);
        let size = format!("{w}x{h}");
        group.bench_with_input(BenchmarkId::new("contiguous", &size), &picture, |b, p| {
            b.iter(|| black_box(pick(p, 0, 0, 20, true)));
        });
        group.bench_with_input(BenchmarkId::new("global", &size), &picture, |b, p| {
            b.iter(|| black_box(pick(p, 0, 0, 20, false)));
        });
        let mask = pick(&picture, 0, 0, 20, true).expect("on the picture");
        group.bench_with_input(BenchmarkId::new("soft_edge", &size), &mask, |b, m| {
            b.iter(|| black_box(coverage(m, 2.0)));
        });
    }
    group.finish();
}

criterion_group!(benches, wand);
criterion_main!(benches);
