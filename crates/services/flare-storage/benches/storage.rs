//! Saving and opening projects of different sizes.
//!
//! Each case saves or loads the same deterministic document: many small shapes, or one shape
//! with a very long path. File I/O is included (a temporary directory, so the filesystem cache
//! is warm for loads), because that is what a user waits for.
#![allow(clippy::expect_used, missing_docs)]

use std::hint::black_box;
use std::num::NonZeroU32;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use flare_core::{DocumentId, DocumentPoint, FrameRate, LayerId, Rgba8, SceneId, ShapeId};
use flare_document::{Content, Document, FrameSpan, Layer, PathSegment, Scene, Shape, Timeline};
use flare_storage::{load, save};

fn many_shapes(count: usize) -> Document {
    let layers = (0..count)
        .map(|i| {
            let x = (i % 100) as f64 * 10.0 + 0.123_456_789;
            let y = (i / 100) as f64 * 10.0 + 0.987_654_321;
            let shape = Shape::ellipse(ShapeId::new(), x, y, 4.0, 4.0, Rgba8::opaque(200, 60, 60))
                .expect("ellipse");
            let span = FrameSpan::new(
                0,
                NonZeroU32::new(24).expect("nonzero"),
                Content::Shape(shape),
            );
            Layer::new(LayerId::new(), format!("L{i}"), vec![span]).expect("layer")
        })
        .collect();
    let scene = Scene::new(SceneId::new(), "S", Timeline::new(layers));
    let rate = FrameRate::new(24, 1).expect("rate");
    Document::new(DocumentId::new(), rate, vec![scene], vec![]).expect("document")
}

fn long_path(segments: usize) -> Document {
    let p = |i: usize| {
        DocumentPoint::new(i as f64 * 0.37 + 0.001, (i % 997) as f64 * 1.13 + 0.002)
            .expect("finite")
    };
    let mut path = vec![PathSegment::MoveTo(p(0))];
    path.extend((1..segments).map(|i| PathSegment::LineTo(p(i))));
    let shape = Shape::new(ShapeId::new(), path, Rgba8::opaque(10, 20, 30)).expect("shape");
    let span = FrameSpan::new(
        0,
        NonZeroU32::new(1).expect("nonzero"),
        Content::Shape(shape),
    );
    let layer = Layer::new(LayerId::new(), "L", vec![span]).expect("layer");
    let scene = Scene::new(SceneId::new(), "S", Timeline::new(vec![layer]));
    let rate = FrameRate::new(24, 1).expect("rate");
    Document::new(DocumentId::new(), rate, vec![scene], vec![]).expect("document")
}

fn cases() -> Vec<(String, Document)> {
    vec![
        ("1000_shapes".into(), many_shapes(1_000)),
        ("10000_shapes".into(), many_shapes(10_000)),
        ("one_path_100000_segments".into(), long_path(100_000)),
    ]
}

fn save_bench(c: &mut Criterion) {
    let dir = tempfile::tempdir().expect("dir");
    let mut group = c.benchmark_group("save");
    group.sample_size(20);
    for (name, document) in cases() {
        let path = dir.path().join(format!("{name}.flare"));
        group.bench_with_input(BenchmarkId::from_parameter(&name), &document, |b, d| {
            b.iter(|| save(black_box(d), &path).expect("save"));
        });
        let bytes = std::fs::metadata(&path).expect("saved").len();
        eprintln!("{name}: {bytes} bytes on disk");
    }
    group.finish();
}

fn load_bench(c: &mut Criterion) {
    let dir = tempfile::tempdir().expect("dir");
    let mut group = c.benchmark_group("load");
    group.sample_size(20);
    for (name, document) in cases() {
        let path = dir.path().join(format!("{name}.flare"));
        save(&document, &path).expect("save");
        group.bench_function(BenchmarkId::from_parameter(&name), |b| {
            b.iter(|| black_box(load(&path).expect("load")));
        });
    }
    group.finish();
}

criterion_group!(benches, save_bench, load_bench);
criterion_main!(benches);
