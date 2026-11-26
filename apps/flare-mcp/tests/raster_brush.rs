//! The `raster_brush` tool through the MCP server: the wiring of `flare_tools::blend::lay_paint`,
//! one undo step, the layer, and the refusals that change nothing.
#![allow(clippy::expect_used)]

mod common;

use common::{Fixture, fixture_of};
use flare_core::{DocumentId, FrameRate, LayerId, Rgba8, SceneId};
use flare_document::{Content, Document, Layer, Scene, Stage, Timeline};
use serde_json::json;

/// A 100 by 60 stage with one empty layer.
fn empty() -> Document {
    let layer = Layer::new(LayerId::new(), "Layer 1", vec![]).expect("layer");
    Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![Scene::new(SceneId::new(), "S", Timeline::new(vec![layer]))],
        vec![],
    )
    .expect("document")
    .with_stage(Stage::new(100, 60, Rgba8::WHITE).expect("stage"))
}

/// The pixel under stage point `(x, y)` of the picture on the first layer in the saved project, and how many
/// pictures the project holds; `None` with no picture there.
async fn saved(f: &Fixture, x: usize, y: usize) -> Option<([u8; 4], usize)> {
    f.ok("save", json!({})).await;
    let doc = flare_storage::load(&f.path).expect("load");
    let span = doc.scenes()[0].timeline().layers()[0].span_at(0)?;
    let Some(Content::Bitmap(placed)) = span.element(span.element_count() - 1) else {
        return None;
    };
    let bitmap = doc.bitmap(placed.bitmap())?;
    // A new painting has `PAINT_DENSITY` pixels to a stage unit; `(x, y)` is a stage point.
    let d = bitmap.width() as usize / 100;
    let (x, y) = (x * d, y * d);
    let i = (y * bitmap.width() as usize + x) * 4;
    let p = &bitmap.pixels()[i..i + 4];
    Some(([p[0], p[1], p[2], p[3]], doc.bitmaps().len()))
}

#[tokio::test]
async fn a_stroke_paints_a_new_picture_and_a_second_mixes_with_it_each_one_undo_step() {
    let f = fixture_of(&empty(), true, false, vec![]).await;
    let before = f.revision().await;
    let done = f
        .ok(
            "raster_brush",
            json!({
                "points": [[5.0, 30.0], [95.0, 30.0]],
                "size": 20,
                "color": "ff0000",
                "mix": 0,
                "expected_revision": before,
            }),
        )
        .await;
    assert_eq!(done["tool"], "raster-brush");
    assert!(done["revision"].as_u64() > Some(before));
    assert_eq!(saved(&f, 50, 30).await, Some(([255, 0, 0, 255], 1)));
    f.ok(
        "raster_brush",
        json!({
            "points": [[10.0, 30.0], [90.0, 30.0]],
            "pressure": [1.0, 1.0],
            "size": 20,
            "color": "0000ff",
            "mix": 50,
            "layer": done["layer"],
        }),
    )
    .await;
    let (mixed, pictures) = saved(&f, 50, 30).await.expect("painted");
    assert_eq!(pictures, 1, "painted into the same picture");
    assert!(
        (100..=155).contains(&mixed[0]) && (100..=155).contains(&mixed[2]),
        "half mixed: {mixed:?}"
    );
    f.ok("undo", json!({})).await;
    assert_eq!(saved(&f, 50, 30).await, Some(([255, 0, 0, 255], 1)));
    f.ok("undo", json!({})).await;
    assert_eq!(
        saved(&f, 50, 30).await,
        None,
        "the first stroke is gone too"
    );
}

#[tokio::test]
async fn refusals_say_why_and_change_nothing() {
    let read_only = fixture_of(&empty(), false, false, vec![]).await;
    let (refused, message) = read_only
        .tool("raster_brush", json!({ "points": [[10.0, 10.0]] }))
        .await;
    assert!(
        refused && message.to_string().contains("read-only"),
        "{message}"
    );
    let f = fixture_of(&empty(), true, false, vec![]).await;
    let before = f.revision().await;
    let stroke = json!([[10.0, 10.0], [50.0, 10.0]]);
    for (arguments, says) in [
        (json!({ "points": [] }), "at least one point"),
        (json!({ "points": stroke, "mix": 150 }), "color mix"),
        (json!({ "points": stroke, "opacity": 150 }), "strength"),
        (json!({ "points": stroke, "color": "nope" }), "Raster Brush"),
        (
            json!({ "points": stroke, "layer": "6f1c2d3e-4a5b-4c6d-8e7f-001122334455" }),
            "no layer",
        ),
        (
            json!({ "points": stroke, "pressure": [1.0] }),
            "one pressure for each",
        ),
        (
            json!({ "points": stroke, "color": "00000000" }),
            "did not change",
        ),
        (
            json!({ "points": stroke, "expected_revision": before + 5 }),
            "revision",
        ),
    ] {
        let (refused, message) = f.tool("raster_brush", arguments.clone()).await;
        assert!(refused, "{arguments} was refused");
        assert!(message.to_string().contains(says), "{arguments}: {message}");
    }
    assert_eq!(f.revision().await, before);
}
