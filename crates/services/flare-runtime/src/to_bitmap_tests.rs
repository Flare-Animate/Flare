#![allow(clippy::expect_used)]

use std::sync::Arc;
use std::time::{Duration, Instant};

use flare_core::{FrameRate, InstanceId, ShapeId, SymbolId, Transform2D};
use flare_document::{Content, Instance, Shape, Stage, Symbol};
use flare_history::EditorSession;
use glamx::DAffine2;

use super::*;
use crate::{Done, PixelJob, Runtime};

const RED: Rgba8 = Rgba8::opaque(255, 0, 0);
const BLUE: Rgba8 = Rgba8::opaque(0, 0, 255);

fn rect(x: f64, y: f64, w: f64, h: f64, color: Rgba8) -> Content {
    Content::Shape(Shape::rect(ShapeId::new(), x, y, w, h, color).expect("rect"))
}

/// A 60 by 40 stage: a blue square on the bottom layer, and on the top one a red square, then an
/// instance of a symbol holding a red square at (40, 10), all on frames 0 to 4.
fn document() -> (EditorSession, SceneId, LayerId) {
    let five = NonZeroU32::new(5).expect("five");
    let symbol = SymbolId::new();
    let inside = Layer::new(
        LayerId::new(),
        "inside",
        vec![FrameSpan::new(0, five, rect(0.0, 0.0, 10.0, 10.0, RED))],
    )
    .expect("layer");
    let instance = Content::Instance(Instance::new(
        InstanceId::new(),
        symbol,
        Transform2D::new(DAffine2::from_translation(glamx::DVec2::new(40.0, 10.0)))
            .expect("matrix"),
    ));
    let top = LayerId::new();
    let span = FrameSpan::new(0, five, rect(10.0, 10.0, 10.0, 10.0, RED))
        .with_extras(vec![instance])
        .expect("two elements");
    let layers = vec![
        Layer::new(top, "top", vec![span]).expect("layer"),
        Layer::new(
            LayerId::new(),
            "bottom",
            vec![FrameSpan::new(0, five, rect(0.0, 0.0, 60.0, 40.0, BLUE))],
        )
        .expect("layer"),
    ];
    let scene = SceneId::new();
    let document = Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![Scene::new(scene, "S", Timeline::new(layers))],
        vec![Symbol::new(symbol, "square", Timeline::new(vec![inside]))],
    )
    .expect("document")
    .with_stage(Stage::new(60, 40, Rgba8::WHITE).expect("stage"));
    (EditorSession::new(document), scene, top)
}

fn both(top: LayerId) -> [SpanRef; 2] {
    let at = SpanRef {
        layer: top,
        start: 0,
        element: 0,
    };
    [at, at.with_element(1)]
}

fn stage_picture(document: &Document, scene: SceneId) -> Vec<u8> {
    let frame = flare_eval::evaluate_frame(document, scene, 0).expect("frame");
    flare_render::render(&frame, 60, 40, Rgba8::WHITE)
        .expect("render")
        .rgba()
        .to_vec()
}

#[test]
fn the_stage_looks_the_same_after_the_selection_becomes_one_bitmap() {
    let (mut session, scene, top) = document();
    let before = stage_picture(session.document(), scene);
    let edit = convert_to_bitmap(session.document(), scene, 0, &both(top), 1, "Bitmap 1")
        .expect("converts");
    session.apply(edit.commands).expect("applies");
    let doc = session.document();
    let span = doc.scenes()[0].timeline().layers()[0]
        .span_at(0)
        .expect("span");
    assert_eq!(
        span.element_count(),
        1,
        "the square and the instance became one bitmap"
    );
    let Some(Content::Bitmap(placed)) = span.element(0) else {
        panic!("a bitmap");
    };
    let bitmap = doc.bitmap(placed.bitmap()).expect("in the library");
    assert_eq!(
        (bitmap.width(), bitmap.height()),
        (40, 10),
        "trimmed to the two squares"
    );
    // The blue layer under it was not drawn in: between the squares the bitmap is clear.
    let at = |x: u32| &bitmap.pixels()[(5 * 40 + x) as usize * 4..][..4];
    assert_eq!(at(5), [255, 0, 0, 255]);
    assert_eq!(at(20)[3], 0, "the bottom layer shows through, not drawn in");
    assert_eq!(at(35), [255, 0, 0, 255], "the symbol's square");
    assert_eq!(
        stage_picture(doc, scene),
        before,
        "what the stage shows is unchanged"
    );
}

#[test]
fn twice_the_scale_is_twice_the_pixels_each_way_and_still_looks_the_same() {
    let (mut session, scene, top) = document();
    let before = stage_picture(session.document(), scene);
    let edit =
        convert_to_bitmap(session.document(), scene, 0, &both(top)[..1], 2, "B").expect("converts");
    session.apply(edit.commands).expect("applies");
    let bitmap = &session.document().bitmaps()[0];
    assert_eq!((bitmap.width(), bitmap.height()), (20, 20));
    assert_eq!(stage_picture(session.document(), scene), before);
}

#[test]
fn a_refusal_from_the_plan_comes_through() {
    let (session, scene, top) = document();
    let other = SpanRef {
        layer: LayerId::new(),
        ..both(top)[0]
    };
    assert_eq!(
        convert_to_bitmap(session.document(), scene, 0, &[both(top)[0], other], 1, "B"),
        Err(ToBitmapError::NotOneKeyframe)
    );
}

#[test]
fn the_pixels_worker_makes_the_edit_for_the_document_it_was_given() {
    for threads in [false, true] {
        let mut runtime = Runtime::inline();
        if threads {
            runtime.start_threads(Arc::new(|| {}));
        }
        let (session, scene, top) = document();
        let document = session.snapshot();
        runtime.submit_pixels(PixelJob::ToBitmap {
            id: 7,
            document: document.clone(),
            scene,
            frame: 0,
            selection: both(top).to_vec(),
            scale: 1,
            name: "Bitmap 1".into(),
        });
        let start = Instant::now();
        let done = loop {
            if let Some(done) = runtime.poll().into_iter().next() {
                break done;
            }
            assert!(start.elapsed() < Duration::from_secs(10), "no result");
            std::thread::yield_now();
        };
        match done {
            Done::ToBitmapMade {
                id: 7,
                document: made_for,
                result,
            } => {
                assert!(Arc::ptr_eq(&made_for, &document));
                assert!(result.expect("converts").commands.len() >= 2);
            }
            _ => panic!("not the conversion"),
        }
    }
}
