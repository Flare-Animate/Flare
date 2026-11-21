//! What Convert to Bitmap costs: drawing one keyframe of 200 ellipses spread over a 1920x1080 /
//! 4096x4096 area (the largest bitmap a document holds) into a picture, trimming and hashing it,
//! and building the edit. Made outside the timed section.
#![allow(clippy::expect_used, missing_docs)]

use std::hint::black_box;
use std::num::NonZeroU32;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use flare_core::{DocumentId, FrameRate, LayerId, Rgba8, SceneId, ShapeId};
use flare_document::{Content, Document, FrameSpan, Layer, Scene, Shape, Timeline};
use flare_tools::ops::SpanRef;

/// 200 ellipses tiling a `w` by `h` area on one keyframe, and the selection of all of them.
fn document(w: f64, h: f64) -> (Document, SceneId, Vec<SpanRef>) {
    let shapes: Vec<Content> = (0..200)
        .map(|i| {
            let (col, row) = (f64::from(i % 20), f64::from(i / 20));
            let (rx, ry) = (w / 40.0, h / 20.0);
            let color = Rgba8::opaque((i * 7 % 255) as u8, 80, 200);
            Content::Shape(
                Shape::ellipse(
                    ShapeId::new(),
                    rx + col * 2.0 * rx,
                    ry + row * 2.0 * ry,
                    rx,
                    ry,
                    color,
                )
                .expect("ellipse"),
            )
        })
        .collect();
    let (first, rest) = shapes.split_first().expect("shapes");
    let span = FrameSpan::new(0, NonZeroU32::MIN, first.clone())
        .with_extras(rest.to_vec())
        .expect("one keyframe");
    let layer = LayerId::new();
    let scene = SceneId::new();
    let document = Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![Scene::new(
            scene,
            "Scene 1",
            Timeline::new(vec![
                Layer::new(layer, "Layer 1", vec![span]).expect("layer"),
            ]),
        )],
        vec![],
    )
    .expect("document");
    let at = SpanRef {
        layer,
        start: 0,
        element: 0,
    };
    let selection = (0..200).map(|i| at.with_element(i)).collect();
    (document, scene, selection)
}

fn to_bitmap(c: &mut Criterion) {
    let mut group = c.benchmark_group("to_bitmap");
    group.sample_size(10);
    for (w, h) in [(1920.0, 1080.0), (4090.0, 4090.0)] {
        let (doc, scene, selection) = document(w, h);
        group.bench_function(BenchmarkId::new("convert", format!("{w}x{h}")), |b| {
            b.iter(|| {
                black_box(
                    flare_runtime::convert_to_bitmap(&doc, scene, 0, &selection, 1, "Bitmap")
                        .expect("converts"),
                )
            });
        });
    }
    group.finish();
}

criterion_group!(benches, to_bitmap);
criterion_main!(benches);
