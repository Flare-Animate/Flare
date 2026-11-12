//! The `magic_wand` tool through the MCP server: the wiring of `flare_tools::magic_wand`, a dry
//! run that needs no write access, one undo step, and the refusals that change nothing.
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

const WHITE: [u8; 4] = [255, 255, 255, 255];
const BLACK: [u8; 4] = [0, 0, 0, 255];

/// A 4x2 bitmap, white on the left and black on the right, at the stage's corner on frame 0.
fn picture() -> Document {
    let (layer, scene) = (LayerId::new(), SceneId::new());
    let layer_value = Layer::new(layer, "L", vec![]).expect("layer");
    let empty = Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![Scene::new(scene, "S", Timeline::new(vec![layer_value]))],
        vec![],
    )
    .expect("document");
    let rgba = [WHITE, WHITE, BLACK, BLACK].repeat(2).concat();
    let bitmap = Bitmap::new(BitmapId::new(), "b", 4, 2, rgba).expect("bitmap");
    let content = Content::Bitmap(BitmapInstance::new(
        InstanceId::new(),
        bitmap.id(),
        Transform2D::IDENTITY,
    ));
    let edits = vec![
        Command::AddBitmap(bitmap),
        Command::InsertSpan {
            scene,
            layer,
            span: FrameSpan::new(0, NonZeroU32::new(2).expect("nz"), content),
        },
    ];
    apply_all(&empty, edits).expect("applied").expect("changed")
}

/// The alpha of each pixel of the bitmap in the saved project.
async fn saved_alphas(f: &Fixture) -> Vec<u8> {
    f.ok("save", json!({})).await;
    let doc = flare_storage::load(&f.path).expect("load");
    let Content::Bitmap(placed) = doc.scenes()[0].timeline().layers()[0].spans()[0].content()
    else {
        panic!("not a bitmap");
    };
    let pixels = doc
        .bitmap(placed.bitmap())
        .expect("bitmap")
        .pixels()
        .to_vec();
    pixels.as_chunks::<4>().0.iter().map(|p| p[3]).collect()
}

#[tokio::test]
async fn clears_the_picked_pixels_as_one_undo_step() {
    let f = fixture_of(&picture(), true, false, vec![]).await;
    let before = f.revision().await;
    let done = f
        .ok(
            "magic_wand",
            json!({ "points": [[0.5, 0.5]], "smoothing": "pixels", "expected_revision": before }),
        )
        .await;
    assert_eq!(
        (done["picked"].as_u64(), done["cleared"].as_bool()),
        (Some(4), Some(true))
    );
    assert!(done["revision"].as_u64() > Some(before));
    assert_eq!(saved_alphas(&f).await, [0, 0, 255, 255, 0, 0, 255, 255]);
    f.ok("undo", json!({})).await;
    assert_eq!(saved_alphas(&f).await, [255; 8]);
}

#[tokio::test]
async fn keep_clears_the_rest_and_a_dry_run_needs_no_write_access() {
    let read_only = fixture_of(&picture(), false, false, vec![]).await;
    let counted = read_only
        .ok(
            "magic_wand",
            json!({ "points": [[0.5, 0.5], [3.5, 1.5]], "dry_run": true }),
        )
        .await;
    assert_eq!(
        (counted["picked"].as_u64(), counted["cleared"].as_bool()),
        (Some(8), Some(false))
    );
    let (refused, message) = read_only
        .tool("magic_wand", json!({ "points": [[0.5, 0.5]] }))
        .await;
    assert!(
        refused && message.to_string().contains("read-only"),
        "{message}"
    );
    let f = fixture_of(&picture(), true, false, vec![]).await;
    f.ok(
        "magic_wand",
        json!({ "points": [[3.5, 0.5]], "keep": true, "smoothing": "pixels" }),
    )
    .await;
    assert_eq!(saved_alphas(&f).await, [0, 0, 255, 255, 0, 0, 255, 255]);
}

#[tokio::test]
async fn refusals_say_why_and_change_nothing() {
    let f = fixture_of(&picture(), true, false, vec![]).await;
    let before = f.revision().await;
    let many: Vec<[f64; 2]> = vec![[0.5, 0.5]; 65];
    for (arguments, says) in [
        (json!({ "points": [[40.0, 40.0]] }), "no bitmap"),
        (json!({ "points": [] }), "at least one point"),
        (json!({ "points": many }), "at most 64"),
        (
            json!({ "points": [[0.5, 0.5]], "smoothing": "blurry" }),
            "smoothing",
        ),
        (
            json!({ "points": [[0.5, 0.5]], "threshold": 201 }),
            "threshold",
        ),
        (
            json!({ "points": [[0.5, 0.5]], "expected_revision": before + 5 }),
            "revision",
        ),
    ] {
        let (refused, message) = f.tool("magic_wand", arguments.clone()).await;
        assert!(refused, "{arguments} was refused");
        assert!(message.to_string().contains(says), "{arguments}: {message}");
    }
    assert_eq!(f.revision().await, before);
}
