//! The `convert_to_bitmap` tool through the MCP server: the wiring of
//! `flare_runtime::convert_to_bitmap`, one undo step, and the refusals that change nothing.
#![allow(clippy::expect_used)]

mod common;

use std::num::NonZeroU32;

use common::fixture_of;
use flare_core::{DocumentId, FrameRate, LayerId, Rgba8, SceneId, ShapeId};
use flare_document::{Content, Document, FrameSpan, Layer, Scene, Shape, Stage, Timeline};
use serde_json::json;

/// A 100 by 60 stage with a red 30 by 20 rectangle at (10, 10) on frames 0 to 4.
fn document() -> Document {
    let rect = Shape::rect(
        ShapeId::new(),
        10.0,
        10.0,
        30.0,
        20.0,
        Rgba8::opaque(255, 0, 0),
    )
    .expect("rect");
    let span = FrameSpan::new(0, NonZeroU32::new(5).expect("5"), Content::Shape(rect));
    let layer = Layer::new(LayerId::new(), "Layer 1", vec![span]).expect("layer");
    Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![Scene::new(SceneId::new(), "S", Timeline::new(vec![layer]))],
        vec![],
    )
    .expect("document")
    .with_stage(Stage::new(100, 60, Rgba8::WHITE).expect("stage"))
}

#[tokio::test]
async fn a_keyframe_becomes_one_bitmap_that_the_raster_brush_paints_into_and_undo_puts_back() {
    let f = fixture_of(&document(), true, false, vec![]).await;
    let before = f.revision().await;
    let done = f
        .ok(
            "convert_to_bitmap",
            json!({ "frame": 2, "expected_revision": before }),
        )
        .await;
    assert_eq!(done["converted"], 1);
    assert_eq!(
        (done["width"].as_u64(), done["height"].as_u64()),
        (Some(30), Some(20))
    );
    // The Raster Brush paints into the converted picture rather than making a new one.
    let painted = f
        .ok(
            "raster_brush",
            json!({ "points": [[12.0, 20.0], [38.0, 20.0]], "color": "0000ff", "size": 6 }),
        )
        .await;
    assert!(painted["bitmap"].as_str().is_some());
    assert_eq!(painted["element"], done["element"]);
    f.ok("undo", json!({})).await;
    f.ok("undo", json!({})).await;
    f.ok("save", json!({})).await;
    let doc = flare_storage::load(&f.path).expect("load");
    let span = doc.scenes()[0].timeline().layers()[0]
        .span_at(0)
        .expect("span");
    assert!(
        matches!(span.element(0), Some(Content::Shape(_))),
        "the rectangle is back"
    );
}

#[tokio::test]
async fn refusals_say_why_and_change_nothing() {
    let f = fixture_of(&document(), true, false, vec![]).await;
    let before = f.revision().await;
    for (arguments, says) in [
        (json!({ "scale": 9 }), "scale"),
        (json!({ "elements": [3] }), "not in the document"),
        (json!({ "frame": 50 }), "no keyframe"),
        (
            json!({ "layer": "6f1c2d3e-4a5b-4c6d-8e7f-001122334455" }),
            "no layer",
        ),
        (json!({ "expected_revision": before + 5 }), "revision"),
    ] {
        let (refused, message) = f.tool("convert_to_bitmap", arguments.clone()).await;
        assert!(refused, "{arguments} was refused");
        assert!(message.to_string().contains(says), "{arguments}: {message}");
    }
    assert_eq!(f.revision().await, before);
}
