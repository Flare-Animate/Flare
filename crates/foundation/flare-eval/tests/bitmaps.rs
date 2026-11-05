#![allow(clippy::expect_used)]

use std::num::NonZeroU32;

use flare_core::{BitmapId, DocumentId, FrameRate, InstanceId, LayerId, SceneId, Transform2D};
use flare_document::{
    Bitmap, BitmapInstance, Content, Document, FrameSpan, Layer, Scene, Timeline,
};
use flare_eval::evaluate_frame;

#[test]
fn a_bitmap_span_is_drawn_only_while_it_lasts() {
    let bitmap = Bitmap::new(BitmapId::new(), "b", 1, 1, vec![9; 4]).expect("bitmap");
    let content = Content::Bitmap(
        BitmapInstance::new(InstanceId::new(), bitmap.id(), Transform2D::IDENTITY)
            .with_opacity(0.5),
    );
    let span = FrameSpan::new(2, NonZeroU32::new(2).expect("nz"), content);
    let layer = Layer::new(LayerId::new(), "L", vec![span]).expect("layer");
    let scene = SceneId::new();
    let document = Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![Scene::new(scene, "S", Timeline::new(vec![layer]))],
        vec![],
    )
    .expect("doc")
    .with_bitmaps(vec![bitmap])
    .expect("bitmaps");
    for (frame, count) in [(0, 0), (2, 1), (3, 1), (4, 0)] {
        let drawn = evaluate_frame(&document, scene, frame).expect("frame");
        assert_eq!(drawn.bitmaps.len(), count, "frame {frame}");
    }
    let drawn = evaluate_frame(&document, scene, 2).expect("frame");
    assert_eq!(drawn.bitmaps[0].opacity, 0.5);
}
