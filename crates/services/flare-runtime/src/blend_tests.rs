#![allow(clippy::expect_used)]

use std::num::NonZeroU32;
use std::time::{Duration, Instant};

use flare_core::{DocumentId, FrameRate, InstanceId, LayerId, Transform2D};
use flare_document::{Bitmap, BitmapInstance, FrameSpan, Layer, Scene, Timeline};
use flare_history::EditorSession;

use super::*;
use crate::{PixelJob, Runtime};

/// A 64x64 picture, black on the left half and white on the right, at frame 0.
fn picture() -> (EditorSession, SceneId) {
    let rgba = (0..64 * 64)
        .flat_map(|i| {
            if i % 64 < 32 {
                [0u8, 0, 0, 255]
            } else {
                [255, 255, 255, 255]
            }
        })
        .collect();
    let bitmap = Bitmap::new(BitmapId::new(), "p", 64, 64, rgba).expect("bitmap");
    let placed = BitmapInstance::new(InstanceId::new(), bitmap.id(), Transform2D::IDENTITY);
    let span = FrameSpan::new(0, NonZeroU32::MIN, Content::Bitmap(placed));
    let layer = Layer::new(LayerId::new(), "Layer 1", vec![span]).expect("layer");
    let scene = SceneId::new();
    let document = Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![Scene::new(scene, "Scene", Timeline::new(vec![layer]))],
        vec![],
    )
    .expect("document")
    .with_bitmaps(vec![bitmap])
    .expect("bitmaps");
    (EditorSession::new(document), scene)
}

fn samples(points: &[(f64, f64)]) -> Vec<InputSample> {
    points
        .iter()
        .map(|(x, y)| InputSample::mouse(*x, *y).expect("finite"))
        .collect()
}

fn begin(runtime: &mut Runtime, session: &EditorSession, scene: SceneId, x: f32) {
    runtime.submit_pixels(PixelJob::Blend(BlendJob::Begin {
        id: 1,
        document: session.snapshot(),
        scene,
        frame: 0,
        start: Pos2::new(x, 4.0),
        kind: BlendKind::Blur,
        settings: BlendSettings {
            size: 12.0,
            ..BlendSettings::for_kind(BlendKind::Blur)
        },
    }));
}

fn step(runtime: &mut Runtime, session: &EditorSession, points: &[(f64, f64)], last: bool) {
    runtime.submit_pixels(PixelJob::Blend(BlendJob::Step {
        id: 1,
        document: session.snapshot(),
        samples: samples(points),
        last,
    }));
}

fn wait(runtime: &mut Runtime) -> Done {
    let start = Instant::now();
    loop {
        if let Some(done) = runtime.poll().into_iter().next() {
            return done;
        }
        assert!(start.elapsed() < Duration::from_secs(10), "no result");
        std::thread::yield_now();
    }
}

/// Plays a drag down the edge in two stretches, applying each edit as the window does and
/// telling the worker; how many edits came back before the stroke ended.
fn drag(runtime: &mut Runtime) -> (usize, EditorSession) {
    let (mut session, scene) = picture();
    begin(runtime, &session, scene, 32.0);
    step(runtime, &session, &[(32.0, 4.0), (32.0, 20.0)], false);
    let mut edits = 0;
    let mut joining = None;
    let mut released = false;
    loop {
        match wait(runtime) {
            Done::BlendEdited {
                id: 1,
                document,
                result,
            } => {
                assert!(
                    Arc::ptr_eq(&document, &session.snapshot()),
                    "made for the shown document"
                );
                joining = session
                    .apply_joining(result.commands, "Blur", joining)
                    .expect("applies");
                edits += 1;
                // The rest of the drag, then its end, then only word that the edit is in.
                if released {
                    step(runtime, &session, &[], true);
                } else {
                    step(runtime, &session, &[(32.0, 40.0), (32.0, 60.0)], true);
                    released = true;
                }
            }
            Done::BlendEnded { id: 1, error } => {
                assert_eq!(error, None);
                return (edits, session);
            }
            _ => panic!("not a stroke's result"),
        }
    }
}

#[test]
fn a_drag_comes_back_as_edits_that_join_one_undo_step_inline_and_on_the_worker() {
    for threads in [false, true] {
        let mut runtime = Runtime::inline();
        if threads {
            runtime.start_threads(Arc::new(|| {}));
        }
        let (edits, mut session) = drag(&mut runtime);
        assert_eq!(
            edits, 2,
            "one edit per stretch, each after the last was applied"
        );
        assert_eq!(
            session.snapshot().bitmaps().len(),
            1,
            "no picture left over"
        );
        assert_eq!(session.undo_labels().len(), 1, "one step");
        assert!(session.undo());
        assert_eq!(session.undo_labels().len(), 0);
    }
}

#[test]
fn a_stroke_off_any_bitmap_ends_with_the_reason() {
    let (session, scene) = picture();
    let mut runtime = Runtime::inline();
    begin(&mut runtime, &session, scene, 500.0);
    match wait(&mut runtime) {
        Done::BlendEnded { id: 1, error } => {
            assert!(error.expect("a reason").contains("bitmap"));
        }
        _ => panic!("expected the end"),
    }
}

#[test]
fn the_worker_waits_for_its_edit_and_ends_when_the_picture_changed_under_it() {
    let (mut session, scene) = picture();
    let mut runtime = Runtime::inline();
    begin(&mut runtime, &session, scene, 32.0);
    let before = session.snapshot();
    step(&mut runtime, &session, &[(32.0, 4.0), (32.0, 20.0)], false);
    assert!(matches!(wait(&mut runtime), Done::BlendEdited { .. }));
    // More of the drag while that edit is on its way: the worker keeps it and sends nothing yet.
    runtime.submit_pixels(PixelJob::Blend(BlendJob::Step {
        id: 1,
        document: before,
        samples: samples(&[(32.0, 40.0)]),
        last: false,
    }));
    assert!(runtime.poll().is_empty());
    // Something else changes the picture instead of the edit going in.
    let other = flare_tools::blend::rub(
        session.document(),
        scene,
        0,
        BlendKind::Smudge,
        BlendSettings::for_kind(BlendKind::Smudge),
        &samples(&[(10.0, 10.0), (50.0, 10.0)]),
    )
    .expect("smudges");
    session.apply(other.commands).expect("applies");
    step(&mut runtime, &session, &[], false);
    match wait(&mut runtime) {
        Done::BlendEnded { id: 1, error } => {
            assert!(error.expect("a reason").contains("changed"));
        }
        _ => panic!("expected the end"),
    }
}

#[test]
fn a_dropped_stroke_hears_no_more_steps() {
    let (session, scene) = picture();
    let mut runtime = Runtime::inline();
    begin(&mut runtime, &session, scene, 32.0);
    runtime.submit_pixels(PixelJob::Blend(BlendJob::Drop { id: 1 }));
    step(&mut runtime, &session, &[(32.0, 4.0), (32.0, 20.0)], true);
    assert!(runtime.poll().is_empty());
}

/// A 64x64 stage with one empty layer.
fn empty() -> (EditorSession, SceneId, LayerId) {
    let layer = LayerId::new();
    let scene = SceneId::new();
    let document = Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![Scene::new(
            scene,
            "Scene",
            Timeline::new(vec![Layer::new(layer, "Layer 1", vec![]).expect("layer")]),
        )],
        vec![],
    )
    .expect("document")
    .with_stage(
        flare_document::Stage::new(64, 64, flare_core::Rgba8::opaque(255, 255, 255))
            .expect("stage"),
    );
    (EditorSession::new(document), scene, layer)
}

#[test]
fn a_raster_brush_drag_places_its_new_bitmap_once_and_paints_on_inline_and_on_the_worker() {
    for threads in [false, true] {
        let mut runtime = Runtime::inline();
        if threads {
            runtime.start_threads(Arc::new(|| {}));
        }
        let (mut session, scene, layer) = empty();
        runtime.submit_pixels(PixelJob::Blend(BlendJob::BeginPaint {
            id: 1,
            document: session.snapshot(),
            scene,
            frame: 0,
            start: Pos2::new(8.0, 32.0),
            settings: BlendSettings::for_kind(BlendKind::RasterBrush),
            paint: flare_tools::blend::RasterPaint {
                color: flare_core::Rgba8::opaque(0, 0, 255),
                layer: flare_tools::ops::Target::Layer(layer),
                new_layer: "Layer 2".into(),
                bitmap_name: "Painting".into(),
            },
        }));
        step(&mut runtime, &session, &[(8.0, 32.0), (30.0, 32.0)], false);
        let before = session.snapshot();
        let mut joining = None;
        let mut edits = 0;
        loop {
            match wait(&mut runtime) {
                Done::BlendEdited { id: 1, result, .. } => {
                    joining = session
                        .apply_joining(result.commands, "Raster Brush", joining)
                        .expect("applies");
                    edits += 1;
                    if edits == 1 {
                        // The edit that places the bitmap is on its way: more of the drag sent
                        // with the document from before it waits for it, not taken as stale.
                        runtime.submit_pixels(PixelJob::Blend(BlendJob::Step {
                            id: 1,
                            document: before.clone(),
                            samples: samples(&[(56.0, 32.0)]),
                            last: true,
                        }));
                        assert!(runtime.poll().is_empty());
                    }
                    step(&mut runtime, &session, &[], true);
                }
                Done::BlendEnded { id: 1, error } => {
                    assert_eq!(error, None);
                    break;
                }
                _ => panic!("not a stroke's result"),
            }
        }
        assert_eq!(
            edits, 2,
            "the first places the bitmap, the second paints on it"
        );
        let document = session.snapshot();
        assert_eq!(document.bitmaps().len(), 1, "no picture left over");
        let span = document.scenes()[0].timeline().layers()[0]
            .span_at(0)
            .expect("a keyframe");
        assert_eq!(span.element_count(), 1);
        let Some(Content::Bitmap(placed)) = span.element(0) else {
            panic!("a bitmap");
        };
        let bitmap = document.bitmap(placed.bitmap()).expect("in the library");
        let at = |x: usize| &bitmap.pixels()[(32 * 64 + x) * 4..(32 * 64 + x) * 4 + 4];
        assert_eq!(at(20), [0, 0, 255, 255], "the first stretch");
        assert_eq!(at(50), [0, 0, 255, 255], "the second");
        assert_eq!(session.undo_labels().len(), 1, "one step");
    }
}
