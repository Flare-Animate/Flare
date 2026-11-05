//! A document with a bitmap is refused with a named error rather than published without it.
#![allow(clippy::expect_used)]

use std::num::NonZeroU32;

use flare_core::{BitmapId, DocumentId, FrameRate, InstanceId, LayerId, SceneId, Transform2D};
use flare_document::{
    Bitmap, BitmapInstance, Content, Document, FrameSpan, Layer, Scene, Timeline,
};
use flare_publish::{PublishError, PublishOptions, publish_swf};

#[test]
fn swf_publish_names_the_missing_bitmap_support() {
    let bitmap = Bitmap::new(BitmapId::new(), "b", 1, 1, vec![0; 4]).expect("bitmap");
    let content = Content::Bitmap(BitmapInstance::new(
        InstanceId::new(),
        bitmap.id(),
        Transform2D::IDENTITY,
    ));
    let span = FrameSpan::new(0, NonZeroU32::new(1).expect("nz"), content);
    let layer = Layer::new(LayerId::new(), "L", vec![span]).expect("layer");
    let document = Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![Scene::new(SceneId::new(), "S", Timeline::new(vec![layer]))],
        vec![],
    )
    .expect("document")
    .with_bitmaps(vec![bitmap])
    .expect("bitmaps");
    let error = publish_swf(&document, &PublishOptions { compress: false }).expect_err("refused");
    assert!(matches!(
        error,
        PublishError::BitmapsUnsupported { frame: 0 }
    ));
}
