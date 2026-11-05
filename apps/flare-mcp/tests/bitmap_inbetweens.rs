//! The `bitmap_inbetweens` tool through the MCP server: files in the export directory, and the
//! refusals that write nothing.
#![allow(clippy::expect_used)]

mod common;

use std::num::NonZeroU32;

use common::{Fixture, fixture_of};
use flare_commands::{Command, apply_all};
use flare_core::{BitmapId, DocumentId, FrameRate, InstanceId, LayerId, SceneId, Transform2D};
use flare_document::{
    Bitmap, BitmapInstance, Content, Document, FrameSpan, Layer, Scene, Timeline,
};
use serde_json::json;

/// A document with a solid 4x4 bitmap keyframe at frame 0 and at frame 4.
fn bitmap_document() -> Document {
    let (layer, scene) = (LayerId::new(), SceneId::new());
    let layer_value = Layer::new(layer, "L", vec![]).expect("layer");
    let empty = Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![Scene::new(scene, "S", Timeline::new(vec![layer_value]))],
        vec![],
    )
    .expect("document");
    let bitmap =
        Bitmap::new(BitmapId::new(), "b", 4, 4, [200, 30, 30, 255].repeat(16)).expect("bitmap");
    let mut edits = vec![Command::AddBitmap(bitmap.clone())];
    for start in [0, 4] {
        let content = Content::Bitmap(BitmapInstance::new(
            InstanceId::new(),
            bitmap.id(),
            Transform2D::IDENTITY,
        ));
        edits.push(Command::InsertSpan {
            scene,
            layer,
            span: FrameSpan::new(start, NonZeroU32::new(2).expect("nz"), content),
        });
    }
    apply_all(&empty, edits).expect("applied").expect("changed")
}

async fn server(exports: bool) -> Fixture {
    fixture_of(&bitmap_document(), false, exports, vec![]).await
}

#[tokio::test]
async fn writes_named_pngs_to_the_export_directory() {
    let f = server(true).await;
    let made = f
        .ok(
            "bitmap_inbetweens",
            json!({ "from": 0, "to": 4, "count": 3, "name": "mid" }),
        )
        .await;
    let frames = made["frames"].as_array().expect("frames");
    assert_eq!(frames.len(), 3);
    for path in frames {
        let path = path.as_str().expect("path");
        assert!(std::path::Path::new(path).exists(), "{path} was written");
    }
    assert!(frames[2].as_str().expect("path").ends_with("mid-003.png"));
    let (refused, _) = f
        .tool(
            "bitmap_inbetweens",
            json!({ "from": 0, "to": 4, "count": 3, "name": "mid" }),
        )
        .await;
    assert!(refused, "existing files are never overwritten");
}

#[tokio::test]
async fn refuses_without_an_export_directory_or_a_keyframe() {
    let off = server(false).await;
    let (disabled, _) = off
        .tool(
            "bitmap_inbetweens",
            json!({ "from": 0, "to": 4, "count": 1, "name": "x" }),
        )
        .await;
    assert!(disabled, "exports are off");

    let f = server(true).await;
    let (bad_count, _) = f
        .tool(
            "bitmap_inbetweens",
            json!({ "from": 0, "to": 4, "count": 0, "name": "x" }),
        )
        .await;
    assert!(bad_count);
    let (no_keyframe, _) = f
        .tool(
            "bitmap_inbetweens",
            json!({ "from": 0, "to": 2, "count": 1, "name": "x" }),
        )
        .await;
    assert!(no_keyframe, "frame 2 starts no keyframe");
}
