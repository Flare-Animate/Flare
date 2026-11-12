#![allow(clippy::expect_used)]

use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::{Duration, Instant};

use emath::Pos2;
use flare_core::{BitmapId, DocumentId, FrameRate, InstanceId, LayerId, SceneId, Transform2D};
use flare_document::{
    Bitmap, BitmapInstance, Content, Document, FrameSpan, Layer, Scene, Timeline,
};
use flare_tools::magic_wand::{Combine, Smoothing, WandSettings};

use crate::{Done, PixelJob, Runtime};

/// A 4x2 picture, white on the left and black on the right, on a layer at frame 0.
fn picture() -> (Arc<Document>, SceneId) {
    let id = BitmapId::new();
    let rgba = [
        [255u8, 255, 255, 255],
        [255, 255, 255, 255],
        [0, 0, 0, 255],
        [0, 0, 0, 255],
    ]
    .repeat(2)
    .concat();
    let bitmap = Bitmap::new(id, "p", 4, 2, rgba).expect("bitmap");
    let placed = BitmapInstance::new(InstanceId::new(), id, Transform2D::IDENTITY);
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
    (Arc::new(document), scene)
}

fn pick_job(id: u64, document: &Arc<Document>, scene: SceneId, x: f32) -> PixelJob {
    PixelJob::WandPick {
        id,
        document: Arc::clone(document),
        scene,
        frame: 0,
        point: Pos2::new(x, 0.5),
        settings: WandSettings::default(),
        combine: Combine::Replace,
        current: None,
    }
}

/// Picks the white half, then clears it, both handed to `runtime`; the results in order.
fn pick_then_clear(runtime: &mut Runtime) -> (usize, Result<Vec<flare_commands::Command>, String>) {
    let (document, scene) = picture();
    runtime.submit_pixels(pick_job(1, &document, scene, 0.5));
    let picked = match wait(runtime) {
        Done::WandPicked { id: 1, result } => result.expect("picked").expect("some pixels"),
        _ => panic!("expected the pick"),
    };
    assert_eq!(
        picked.preview, picked.selection.mask,
        "small enough to be its own picture"
    );
    let count = picked.selection.mask.count();
    runtime.submit_pixels(PixelJob::WandClear {
        id: 2,
        document: Arc::clone(&document),
        scene,
        selection: picked.selection,
        smoothing: Smoothing::Pixels,
    });
    match wait(runtime) {
        Done::WandCleared {
            id: 2,
            document: from,
            result,
        } => {
            assert!(
                Arc::ptr_eq(&from, &document),
                "says which document the commands are for"
            );
            (count, result)
        }
        _ => panic!("expected the clear"),
    }
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

#[test]
fn the_wand_picks_and_clears_inline_and_on_the_pixels_worker_alike() {
    let mut inline = Runtime::inline();
    let (count, commands) = pick_then_clear(&mut inline);
    assert_eq!(count, 4);
    assert!(!commands.expect("commands").is_empty());
    let mut threads = Runtime::inline();
    threads.start_threads(Arc::new(|| {}));
    let (count, commands) = pick_then_clear(&mut threads);
    assert_eq!(count, 4);
    assert!(!commands.expect("commands").is_empty());
}

#[test]
fn a_wand_click_off_any_bitmap_comes_back_as_a_message() {
    let (document, scene) = picture();
    let mut runtime = Runtime::inline();
    runtime.submit_pixels(pick_job(7, &document, scene, 50.0));
    match wait(&mut runtime) {
        Done::WandPicked { id: 7, result } => {
            assert!(result.expect_err("no bitmap").contains("bitmap"));
        }
        _ => panic!("expected the pick"),
    }
}
