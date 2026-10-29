//! Evaluate-and-render cost for a 1280x720 stage.
//!
//! The documents are deterministic and built outside the timed section. `evaluate` is timed
//! apart from `render` so a regression can be pinned on one of them. Output allocation and
//! destruction are inside the timed region, as in a real frame.
#![allow(clippy::expect_used, missing_docs)]

use std::hint::black_box;
use std::num::NonZeroU32;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use flare_core::{DocumentId, FrameRate, LayerId, Rgba8, SceneId, ShapeId};
use flare_document::{Content, Document, FrameSpan, Layer, Scene, Shape, Stage, Timeline};
use flare_render::render;

fn document(count: usize, gradient: bool) -> (Document, SceneId) {
    let scene_id = SceneId::new();
    let layers = (0..count)
        .map(|i| {
            let x = (i % 80) as f64 * 16.0;
            let y = (i / 80) as f64 * 16.0 % 700.0;
            let fill = Rgba8::new(40 + (i % 200) as u8, 90, 160, 255);
            let mut shape =
                Shape::ellipse(ShapeId::new(), x + 8.0, y + 8.0, 7.0, 7.0, fill).expect("ellipse");
            if gradient {
                let p = |x, y| flare_core::DocumentPoint::new(x, y).expect("finite");
                let g = flare_document::Gradient::linear(
                    p(x, y),
                    p(x + 16.0, y + 16.0),
                    fill,
                    Rgba8::opaque(250, 250, 250),
                )
                .expect("gradient");
                shape = shape.with_gradient(g);
            }
            let span = FrameSpan::new(
                0,
                NonZeroU32::new(24).expect("nonzero"),
                Content::Shape(shape),
            );
            Layer::new(LayerId::new(), format!("L{i}"), vec![span]).expect("layer")
        })
        .collect();
    let scene = Scene::new(scene_id, "S", Timeline::new(layers));
    let rate = FrameRate::new(24, 1).expect("rate");
    let stage = Stage::new(1280, 720, Rgba8::WHITE).expect("stage");
    let doc = Document::new(DocumentId::new(), rate, vec![scene], vec![])
        .expect("document")
        .with_stage(stage);
    (doc, scene_id)
}

fn evaluate(c: &mut Criterion) {
    let mut group = c.benchmark_group("evaluate_frame");
    for count in [100, 1_000, 5_000] {
        let (doc, scene) = document(count, false);
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, _| {
            b.iter(|| black_box(flare_eval::evaluate_frame(&doc, scene, 3).expect("frame")));
        });
    }
    group.finish();
}

fn render_full(c: &mut Criterion) {
    let mut group = c.benchmark_group("render_1280x720");
    group.sample_size(20);
    for (count, gradient) in [
        (100, false),
        (1_000, false),
        (5_000, false),
        (20_000, false),
    ] {
        let (doc, scene) = document(count, gradient);
        let frame = flare_eval::evaluate_frame(&doc, scene, 3).expect("frame");
        let label = format!("{count}{}", if gradient { "_gradient" } else { "" });
        group.bench_function(BenchmarkId::from_parameter(label), |b| {
            b.iter(|| black_box(render(&frame, 1280, 720, Rgba8::WHITE).expect("render")));
        });
    }
    group.finish();
}

criterion_group!(benches, evaluate, render_full);
criterion_main!(benches);
