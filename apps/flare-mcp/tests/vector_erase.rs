//! The `vector_erase` tool through the MCP server: the wiring of `flare_tools::eraser::vector`,
//! each type, one undo step, and the refusals that change nothing.
#![allow(clippy::expect_used)]

mod common;

use common::fixture;
use serde_json::json;

async fn shapes(f: &common::Fixture) -> u64 {
    let info = f.ok("get_info", json!({})).await;
    info["scenes"][0]["layers"]
        .as_array()
        .expect("layers")
        .iter()
        .filter_map(|l| l["elements"].as_u64())
        .sum()
}

#[tokio::test]
async fn each_type_takes_whole_shapes_as_one_step() {
    let f = fixture(true, false).await;
    for x in [0.0, 100.0, 200.0] {
        f.ok(
            "draw_rect",
            json!({ "x": x, "y": 0.0, "width": 40.0, "height": 40.0 }),
        )
        .await;
    }
    assert_eq!(shapes(&f).await, 3);
    // Holds the first, crosses the second.
    let boxed = f
        .ok(
            "vector_erase",
            json!({ "type": "rectangular", "points": [[-5.0, -5.0], [120.0, 50.0]] }),
        )
        .await;
    assert_eq!(boxed["erased"], true);
    assert_eq!(shapes(&f).await, 2, "only the one wholly inside went");
    f.ok("undo", json!({})).await;
    assert_eq!(shapes(&f).await, 3, "one undo step puts it back");
    f.ok(
        "vector_erase",
        json!({ "points": [[100.0, 0.0], [110.0, -5.0]], "size": 4.0 }),
    )
    .await;
    assert_eq!(
        shapes(&f).await,
        2,
        "the tip touched the second, which went whole"
    );
    f.ok(
        "vector_erase",
        json!({ "type": "freehand", "points": [[190.0, -10.0], [250.0, -10.0], [250.0, 50.0], [190.0, 50.0]] }),
    )
    .await;
    assert_eq!(shapes(&f).await, 1);
}

#[tokio::test]
async fn nothing_taken_a_wrong_request_or_no_write_access_is_refused() {
    let f = fixture(true, false).await;
    f.ok(
        "draw_rect",
        json!({ "x": 0.0, "y": 0.0, "width": 40.0, "height": 40.0 }),
    )
    .await;
    for (args, says) in [
        (json!({ "points": [[400.0, 400.0]] }), "no shape is taken"),
        (
            json!({ "type": "rectangular", "points": [[0.0, 0.0]] }),
            "needs 2",
        ),
        (
            json!({ "type": "polyline", "points": [[0.0, 0.0]] }),
            "no type",
        ),
        (json!({ "points": [[0.0, 0.0]], "layer": "nope" }), "layer"),
    ] {
        let (refused, text) = f.tool("vector_erase", args.clone()).await;
        assert!(refused && text.to_string().contains(says), "{args}: {text}");
    }
    assert_eq!(shapes(&f).await, 1, "nothing changed");
    let read_only = fixture(false, false).await;
    let (refused, _) = read_only
        .tool("vector_erase", json!({ "points": [[0.0, 0.0]] }))
        .await;
    assert!(refused, "erasing needs write access");
}

#[tokio::test]
async fn segment_takes_a_stroke_out_to_where_another_crosses_it() {
    let f = fixture(true, false).await;
    // Two thin strokes, as the Brush draws them: one across, one down through it at x = 48..52.
    for (x, y, width, height) in [(0.0, -2.0, 100.0, 4.0), (48.0, -50.0, 4.0, 100.0)] {
        f.ok(
            "draw_rect",
            json!({ "x": x, "y": y, "width": width, "height": height }),
        )
        .await;
    }
    let boxed = json!({ "type": "rectangular", "points": [[-5.0, -5.0], [55.0, 5.0]] });
    let (refused, _) = f.tool("vector_erase", boxed.clone()).await;
    assert!(refused, "the stroke across runs out of the box");
    let taken = f
        .ok(
            "vector_erase",
            json!({ "points": [[80.0, 0.0]], "size": 4.0 }),
        )
        .await;
    assert_eq!(taken["type"], "segment", "segment is the default");
    assert_eq!(shapes(&f).await, 2, "both strokes are still there");
    f.ok("vector_erase", boxed).await;
    assert_eq!(
        shapes(&f).await,
        1,
        "the stroke across now ends at the far side of the other"
    );
}
