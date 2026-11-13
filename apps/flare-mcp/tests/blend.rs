//! The `blur` and `smudge` tools through the MCP server: the wiring of `flare_tools::blend`, one
//! undo step, write access, and the refusals that change nothing.
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

/// A 64x64 bitmap at the stage's corner on frame 0, black on the left and white on the right.
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
    let rgba = (0..64 * 64)
        .flat_map(|i| {
            if i % 64 < 32 {
                [0u8, 0, 0, 255]
            } else {
                [255; 4]
            }
        })
        .collect();
    let bitmap = Bitmap::new(BitmapId::new(), "b", 64, 64, rgba).expect("bitmap");
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

/// The red of pixel `(x, y)` of the bitmap in the saved project.
async fn saved_red(f: &Fixture, x: usize, y: usize) -> u8 {
    f.ok("save", json!({})).await;
    let doc = flare_storage::load(&f.path).expect("load");
    let Content::Bitmap(placed) = doc.scenes()[0].timeline().layers()[0].spans()[0].content()
    else {
        panic!("not a bitmap");
    };
    doc.bitmap(placed.bitmap()).expect("bitmap").pixels()[(y * 64 + x) * 4]
}

#[tokio::test]
async fn blur_softens_the_edge_as_one_undo_step() {
    let f = fixture_of(&picture(), true, false, vec![]).await;
    let before = f.revision().await;
    let done = f
        .ok(
            "blur",
            json!({ "points": [[32.0, 4.0], [32.0, 60.0]], "size": 16, "expected_revision": before }),
        )
        .await;
    assert_eq!(done["tool"], "blur");
    assert!(done["revision"].as_u64() > Some(before));
    assert!(
        saved_red(&f, 31, 32).await > 20,
        "the black side got lighter"
    );
    f.ok("undo", json!({})).await;
    assert_eq!(saved_red(&f, 31, 32).await, 0);
}

#[tokio::test]
async fn smudge_carries_the_dark_side_into_the_light_with_a_pressure_per_point() {
    let f = fixture_of(&picture(), true, false, vec![]).await;
    f.ok(
        "smudge",
        json!({
            "points": [[20.0, 32.0], [50.0, 32.0]],
            "pressure": [1.0, 0.8],
            "size": 10,
            "stretch": 90,
        }),
    )
    .await;
    assert!(saved_red(&f, 40, 32).await < 200);
}

#[tokio::test]
async fn refusals_say_why_and_change_nothing() {
    let read_only = fixture_of(&picture(), false, false, vec![]).await;
    let (refused, message) = read_only
        .tool("blur", json!({ "points": [[32.0, 4.0], [32.0, 60.0]] }))
        .await;
    assert!(
        refused && message.to_string().contains("read-only"),
        "{message}"
    );
    let f = fixture_of(&picture(), true, false, vec![]).await;
    let before = f.revision().await;
    let stroke = json!([[32.0, 4.0], [32.0, 60.0]]);
    for (tool, arguments, says) in [
        ("blur", json!({ "points": [[500.0, 500.0]] }), "no bitmap"),
        ("blur", json!({ "points": [] }), "at least one point"),
        (
            "blur",
            json!({ "points": stroke, "stretch": 5 }),
            "for smudge",
        ),
        ("blur", json!({ "points": stroke, "size": 0 }), "size"),
        (
            "smudge",
            json!({ "points": stroke, "strength": 150 }),
            "strength",
        ),
        (
            "smudge",
            json!({ "points": stroke, "pressure": [1.0] }),
            "one pressure for each",
        ),
        (
            "smudge",
            json!({ "points": stroke, "pressure": [2.0, 1.0] }),
            "0 to 1",
        ),
        (
            "blur",
            json!({ "points": stroke, "expected_revision": before + 5 }),
            "revision",
        ),
    ] {
        let (refused, message) = f.tool(tool, arguments.clone()).await;
        assert!(refused, "{tool} {arguments} was refused");
        assert!(message.to_string().contains(says), "{arguments}: {message}");
    }
    assert_eq!(f.revision().await, before);
}
