//! The `set_bitmap_compression` tool through the MCP server: the wiring of
//! `flare_tools::library::set_bitmap_compression`, one undo step, what `get_info` then says, and
//! the refusals that change nothing.
#![allow(clippy::expect_used)]

mod common;

use std::num::NonZeroU32;

use common::fixture_of;
use flare_core::{BitmapId, DocumentId, FrameRate, InstanceId, LayerId, SceneId, Transform2D};
use flare_document::{
    Bitmap, BitmapInstance, Content, Document, FrameSpan, Layer, Scene, Timeline,
};
use serde_json::json;

/// One smooth 96x64 picture on frame 0, as a frame of video is.
fn document() -> (Document, BitmapId) {
    let rgba = (0..64u32)
        .flat_map(|y| (0..96u32).flat_map(move |x| [(x * 2) as u8, (y * 3) as u8, 90, 255]))
        .collect();
    let bitmap = Bitmap::new(BitmapId::new(), "frame", 96, 64, rgba).expect("bitmap");
    let id = bitmap.id();
    let shown = BitmapInstance::new(InstanceId::new(), id, Transform2D::IDENTITY);
    let span = FrameSpan::new(0, NonZeroU32::new(1).expect("1"), Content::Bitmap(shown));
    let layer = Layer::new(LayerId::new(), "video", vec![span]).expect("layer");
    let document = Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![Scene::new(SceneId::new(), "S", Timeline::new(vec![layer]))],
        vec![],
    )
    .expect("document")
    .with_bitmaps(vec![bitmap])
    .expect("bitmaps");
    (document, id)
}

#[tokio::test]
async fn bitmaps_become_photos_in_one_undo_step_and_get_info_says_so() {
    let (document, id) = document();
    let f = fixture_of(&document, true, false, vec![]).await;
    let info = f.ok("get_info", json!({})).await;
    assert_eq!(
        (
            info["bitmaps"]["count"].as_u64(),
            info["bitmaps"]["photos"].as_u64()
        ),
        (Some(1), Some(0))
    );
    let done = f
        .ok(
            "set_bitmap_compression",
            json!({ "compression": "photo", "quality": 80 }),
        )
        .await;
    assert_eq!(done["changed"], 1);
    assert!(done["bitmap_bytes_after"].as_u64() < done["bitmap_bytes_before"].as_u64());
    let info = f.ok("get_info", json!({})).await;
    assert_eq!(info["bitmaps"]["photos"], 1);
    assert!(info["bitmaps"]["stored_bytes"].as_u64() < info["bitmaps"]["pixel_bytes"].as_u64());
    f.ok("undo", json!({})).await;
    assert_eq!(
        f.ok("get_info", json!({})).await["bitmaps"]["photos"],
        0,
        "one undo step"
    );
    let named = f
        .ok(
            "set_bitmap_compression",
            json!({ "compression": "photo", "bitmaps": [id.as_uuid().to_string()] }),
        )
        .await;
    assert_eq!(named["changed"], 1);
    let back = f
        .ok(
            "set_bitmap_compression",
            json!({ "compression": "lossless" }),
        )
        .await;
    assert_eq!(back["changed"], 1);
}

#[tokio::test]
async fn a_refusal_changes_nothing() {
    let (document, _) = document();
    let f = fixture_of(&document, true, false, vec![]).await;
    let before = f.revision().await;
    for (arguments, says) in [
        (json!({ "compression": "jpeg" }), "photo\" or \"lossless"),
        (
            json!({ "compression": "lossless", "quality": 50 }),
            "quality is for photo",
        ),
        (json!({ "compression": "photo", "quality": 0 }), "1 to 100"),
        (
            json!({ "compression": "photo", "bitmaps": ["nope"] }),
            "not a bitmap id",
        ),
        (
            json!({ "compression": "photo", "bitmaps": [BitmapId::new().as_uuid().to_string()] }),
            "no bitmap",
        ),
    ] {
        let (is_error, out) = f.tool("set_bitmap_compression", arguments).await;
        assert!(is_error, "{out}");
        let words = out.as_str().map_or_else(|| out.to_string(), str::to_owned);
        assert!(words.contains(says), "{words}");
    }
    assert_eq!(f.revision().await, before);
    // Without write access nothing is changed either.
    let read_only = fixture_of(&document, false, false, vec![]).await;
    let (is_error, _) = read_only
        .tool("set_bitmap_compression", json!({ "compression": "photo" }))
        .await;
    assert!(is_error);
}
