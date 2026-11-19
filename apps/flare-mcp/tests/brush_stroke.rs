//! The `brush_stroke` tool through the MCP server: the wiring of `flare_tools::brush`, one undo
//! step, write access, and the refusals that change nothing. What an outline is, is tested in the
//! library.
#![allow(clippy::expect_used)]

mod common;

use common::{Fixture, fixture};
use flare_document::Content;
use serde_json::json;

/// The fills of the shapes on the first layer's keyframe at frame 0 in the saved project.
async fn saved_fills(f: &Fixture) -> Vec<[u8; 4]> {
    f.ok("save", json!({})).await;
    let doc = flare_storage::load(&f.path).expect("load");
    let layer = &doc.scenes()[0].timeline().layers()[0];
    let Some(span) = layer.span_at(0) else {
        return Vec::new();
    };
    (0..span.element_count())
        .filter_map(|i| match span.element(i) {
            Some(Content::Shape(shape)) => {
                let c = shape.fill();
                Some([c.r, c.g, c.b, c.a])
            }
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn strokes_are_laid_in_their_color_as_one_undo_step() {
    let f = fixture(true, false).await;
    let before = f.revision().await;
    let done = f
        .ok(
            "brush_stroke",
            json!({
                "strokes": [
                    [[2.0, 5.0], [20.0, 5.0], [38.0, 5.0]],
                    [[2.0, 15.0, 0.4], [38.0, 15.0, 1.0]],
                    [[3.0, 3.0], [3.0, 3.0]],
                ],
                "size": 4,
                "color": "00ff0080",
                "flow": 50,
                "profile": "ink",
                "expected_revision": before,
            }),
        )
        .await;
    assert_eq!(done["drawn"], 2);
    assert_eq!(done["skipped"], json!([2]));
    assert!(done["revision"].as_u64() > Some(before));
    // Half the given alpha, for half the flow.
    assert_eq!(saved_fills(&f).await, vec![[0, 255, 0, 64]; 2]);
    f.ok("undo", json!({})).await;
    assert!(saved_fills(&f).await.is_empty());
}

#[tokio::test]
async fn refusals_say_why_and_change_nothing() {
    let read_only = fixture(false, false).await;
    let (refused, message) = read_only
        .tool(
            "brush_stroke",
            json!({ "strokes": [[[0.0, 0.0], [9.0, 9.0]]] }),
        )
        .await;
    assert!(
        refused && message.to_string().contains("read-only"),
        "{message}"
    );
    let f = fixture(true, false).await;
    let before = f.revision().await;
    let line = json!([[[0.0, 0.0], [9.0, 9.0]]]);
    for (arguments, says) in [
        (json!({ "strokes": [] }), "no strokes"),
        (
            json!({ "strokes": [[[1.0, 1.0], [1.0, 1.0]]] }),
            "draws nothing",
        ),
        (
            json!({ "strokes": [[[1.0], [2.0, 2.0]]] }),
            "point 0 of stroke 0",
        ),
        (json!({ "strokes": [[[1.0, 1.0, 3.0]]] }), "pressure"),
        (json!({ "strokes": line, "size": 0 }), "size must be"),
        (json!({ "strokes": line, "flow": 120 }), "flow must be"),
        (
            json!({ "strokes": line, "stabilizer": 99 }),
            "stabilizer must be",
        ),
        (
            json!({ "strokes": line, "profile": "airbrush" }),
            "no brush profile",
        ),
        (json!({ "strokes": line, "color": "green" }), "color"),
        (json!({ "strokes": line, "duration": 0 }), "duration"),
        (
            json!({ "strokes": line, "layer": "00000000-0000-0000-0000-000000000000" }),
            "layer",
        ),
        (
            json!({ "strokes": line, "expected_revision": before + 5 }),
            "revision",
        ),
    ] {
        let (refused, message) = f.tool("brush_stroke", arguments.clone()).await;
        assert!(refused, "{arguments}");
        assert!(message.to_string().contains(says), "{arguments}: {message}");
    }
    assert_eq!(f.revision().await, before);
}
