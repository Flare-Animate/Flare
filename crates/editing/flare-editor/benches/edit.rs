//! Edit cost against document size.
//!
//! Every case builds the same deterministic document (one shape per layer, a 24-frame span
//! each) outside the timed section. Sizes are chosen so a cost that grows with the whole
//! document is visible: a rename touches one layer, so its time should not depend on how many
//! other layers exist.
#![allow(clippy::expect_used, missing_docs)]

use std::hint::black_box;
use std::num::NonZeroU32;

use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};
use flare_commands::{Command, LayerChange};
use flare_core::{DocumentId, FrameRate, LayerId, Rgba8, SceneId, ShapeId};
use flare_document::{Content, Document, FrameSpan, Layer, Scene, Shape, Timeline};
use flare_history::EditorSession;

struct Fixture {
    document: Document,
    scene: SceneId,
    layers: Vec<LayerId>,
}

fn fixture(count: usize) -> Fixture {
    let scene_id = SceneId::new();
    let mut ids = Vec::with_capacity(count);
    let layers = (0..count)
        .map(|i| {
            let id = LayerId::new();
            ids.push(id);
            let x = (i % 100) as f64 * 10.0;
            let y = (i / 100) as f64 * 10.0;
            let shape = Shape::ellipse(ShapeId::new(), x, y, 4.0, 4.0, Rgba8::opaque(200, 60, 60))
                .expect("ellipse");
            let span = FrameSpan::new(
                0,
                NonZeroU32::new(24).expect("nonzero"),
                Content::Shape(shape),
            );
            Layer::new(id, format!("L{i}"), vec![span]).expect("layer")
        })
        .collect();
    let scene = Scene::new(scene_id, "S", Timeline::new(layers));
    let rate = FrameRate::new(24, 1).expect("rate");
    let document = Document::new(DocumentId::new(), rate, vec![scene], vec![]).expect("document");
    Fixture {
        document,
        scene: scene_id,
        layers: ids,
    }
}

/// Re-inserts the first `n` layers' spans, as moving a selection of `n` shapes does.
fn move_commands(f: &Fixture, n: usize) -> Vec<Command> {
    let timeline = f.document.scenes()[0].timeline();
    timeline
        .layers()
        .iter()
        .take(n)
        .flat_map(|layer| {
            let span = layer.spans()[0].clone();
            [
                Command::RemoveSpan {
                    scene: f.scene,
                    layer: layer.id(),
                    start: 0,
                },
                Command::InsertSpan {
                    scene: f.scene,
                    layer: layer.id(),
                    span,
                },
            ]
        })
        .collect()
}

fn rename_one(c: &mut Criterion) {
    let mut group = c.benchmark_group("rename_one_layer");
    for size in [100, 1_000, 5_000] {
        let f = fixture(size);
        let rename = Command::UpdateLayer {
            scene: f.scene,
            layer: f.layers[0],
            change: LayerChange::Name("renamed".into()),
        };
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter_batched(
                || (EditorSession::new(f.document.clone()), rename.clone()),
                |(mut session, command)| {
                    session.apply([command]).expect("apply");
                    black_box(session.revision())
                },
                BatchSize::LargeInput,
            );
        });
    }
    group.finish();
}

fn move_selection(c: &mut Criterion) {
    let mut group = c.benchmark_group("move_100_spans");
    for size in [1_000, 5_000] {
        let f = fixture(size);
        let commands = move_commands(&f, 100);
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter_batched(
                || (EditorSession::new(f.document.clone()), commands.clone()),
                |(mut session, commands)| {
                    session.apply(commands).expect("apply");
                    black_box(session.revision())
                },
                BatchSize::LargeInput,
            );
        });
    }
    group.finish();
}

/// Rebuilding a document runs its whole validation; every edit pays this once.
fn validate(c: &mut Criterion) {
    let mut group = c.benchmark_group("validate_document");
    for size in [1_000, 5_000, 20_000] {
        let f = fixture(size);
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter(|| {
                black_box(
                    Document::new(
                        f.document.id(),
                        f.document.frame_rate(),
                        f.document.scenes().to_vec(),
                        f.document.symbols().to_vec(),
                    )
                    .expect("valid"),
                )
            });
        });
    }
    group.finish();
}

fn undo_redo(c: &mut Criterion) {
    let f = fixture(5_000);
    let rename = Command::UpdateLayer {
        scene: f.scene,
        layer: f.layers[0],
        change: LayerChange::Name("renamed".into()),
    };
    let mut session = EditorSession::new(f.document.clone());
    session.apply([rename]).expect("apply");
    c.bench_function("undo_then_redo_5000", |b| {
        b.iter(|| {
            session.undo();
            session.redo();
            black_box(session.revision())
        });
    });
}

criterion_group!(benches, rename_one, move_selection, validate, undo_redo);
criterion_main!(benches);
