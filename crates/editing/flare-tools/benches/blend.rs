//! What a Blur, Smudge or Raster Brush drag costs each frame: laying the dabs of one frame's
//! stretch of the drag (30 stage units at size 80 for Blur, 30 for Smudge, 20 for the Raster Brush
//! at half mix), starting a stroke (the copy of the bitmap it works on, or for the Raster Brush on
//! an empty layer a clear bitmap the size of the 1920x1080 stage), and the edit that shows the
//! stroke (the new bitmap and its hash). Against the size of the bitmap, made outside the timed
//! section.
#![allow(clippy::expect_used, missing_docs)]

use std::hint::black_box;
use std::num::NonZeroU32;

use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};
use flare_core::{
    BitmapId, DocumentId, FrameRate, InstanceId, LayerId, Rgba8, SceneId, Transform2D,
};
use flare_document::{
    Bitmap, BitmapInstance, Content, Document, FrameSpan, Layer, Scene, Timeline,
};
use flare_input::InputSample;
use flare_tools::blend::{BlendKind, BlendSettings, BlendStroke, RasterPaint};
use flare_tools::magic_wand::Pos2;
use flare_tools::ops::Target;
use glamx::DAffine2;

/// A document whose one frame shows a `w` by `h` picture of stripes at the origin.
fn document(w: u32, h: u32) -> (Document, SceneId) {
    let rgba = (0..w * h)
        .flat_map(|i| {
            if (i % w / 16).is_multiple_of(2) {
                [30, 60, 200, 255]
            } else {
                [250, 240, 220, 255]
            }
        })
        .collect();
    let bitmap = Bitmap::new(BitmapId::new(), "stripes", w, h, rgba).expect("bitmap");
    let placed = BitmapInstance::new(
        InstanceId::new(),
        bitmap.id(),
        Transform2D::new(DAffine2::IDENTITY).expect("matrix"),
    );
    let one = NonZeroU32::new(1).expect("one");
    let span = FrameSpan::new(0, one, Content::Bitmap(placed));
    let layer = Layer::new(LayerId::new(), "Layer 1", vec![span]).expect("layer");
    let scene = Scene::new(SceneId::new(), "Scene 1", Timeline::new(vec![layer]));
    let id = scene.id();
    let rate = FrameRate::new(24, 1).expect("rate");
    let document = Document::new(DocumentId::new(), rate, vec![scene], vec![])
        .expect("document")
        .with_bitmaps(vec![bitmap])
        .expect("bitmaps");
    (document, id)
}

/// Blue paint on the topmost layer.
fn paint() -> RasterPaint {
    RasterPaint {
        color: Rgba8::opaque(0, 0, 255),
        layer: Target::Default,
        new_layer: "Layer 1".into(),
        bitmap_name: "Painting".into(),
    }
}

/// Starts a `kind` stroke at `start`.
fn begin(doc: &Document, scene: SceneId, start: Pos2, kind: BlendKind) -> BlendStroke {
    let settings = BlendSettings::for_kind(kind);
    let begun = if kind == BlendKind::RasterBrush {
        BlendStroke::paint(doc, scene, 0, start, settings, paint())
    } else {
        BlendStroke::begin(doc, scene, 0, start, kind, settings)
    };
    begun.expect("on it")
}

fn samples(from: (f64, f64), to: (f64, f64), count: usize) -> Vec<InputSample> {
    (0..=count)
        .map(|i| {
            let t = i as f64 / count as f64;
            let (x, y) = (from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t);
            InputSample::mouse(x, y).expect("finite")
        })
        .collect()
}

fn blend(c: &mut Criterion) {
    let mut group = c.benchmark_group("blend");
    group.sample_size(20);
    for (w, h) in [(1920, 1080), (4096, 4096)] {
        let (doc, scene) = document(w, h);
        let size = format!("{w}x{h}");
        let start = Pos2::new(400.0, 400.0);
        for kind in BlendKind::ALL {
            let begin = || {
                let mut stroke = begin(&doc, scene, start, kind);
                // Already under way, as a stroke is after its first frame.
                stroke.extend(&samples((400.0, 400.0), (430.0, 400.0), 6));
                stroke
            };
            let stretch = samples((430.0, 400.0), (460.0, 400.0), 6);
            let name = format!("{}_frame", kind.name());
            group.bench_function(BenchmarkId::new(name, &size), |b| {
                b.iter_batched(
                    begin,
                    |mut stroke| {
                        stroke.extend(&stretch);
                        black_box(stroke.changed())
                    },
                    BatchSize::LargeInput,
                );
            });
        }
        let settings = BlendSettings::for_kind(BlendKind::Blur);
        group.bench_function(BenchmarkId::new("begin", &size), |b| {
            b.iter(|| {
                black_box(BlendStroke::begin(
                    &doc,
                    scene,
                    0,
                    start,
                    BlendKind::Blur,
                    settings,
                ))
            });
        });
        let mut stroke =
            BlendStroke::begin(&doc, scene, 0, start, BlendKind::Blur, settings).expect("on it");
        stroke.extend(&samples((400.0, 400.0), (460.0, 400.0), 12));
        group.bench_function(BenchmarkId::new("edit", &size), |b| {
            b.iter(|| black_box(stroke.edit(&doc, scene)));
        });
    }
    // The Raster Brush on an empty layer starts on a clear bitmap the size of the stage.
    let layer = Layer::new(LayerId::new(), "Layer 1", vec![]).expect("layer");
    let scene = Scene::new(SceneId::new(), "Scene 1", Timeline::new(vec![layer]));
    let empty_scene = scene.id();
    let rate = FrameRate::new(24, 1).expect("rate");
    let empty = Document::new(DocumentId::new(), rate, vec![scene], vec![]).expect("document");
    let start = Pos2::new(400.0, 400.0);
    let settings = BlendSettings::for_kind(BlendKind::RasterBrush);
    group.bench_function(
        BenchmarkId::new("raster-brush_begin_new", "1920x1080"),
        |b| {
            b.iter(|| {
                black_box(BlendStroke::paint(
                    &empty,
                    empty_scene,
                    0,
                    start,
                    settings,
                    paint(),
                ))
            });
        },
    );
    group.finish();
}

criterion_group!(benches, blend);
criterion_main!(benches);
