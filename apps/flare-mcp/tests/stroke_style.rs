//! Stroke styles through the MCP server.
#![allow(clippy::expect_used)]

mod common;

use common::fixture;
use serde_json::json;

#[tokio::test]
async fn draw_rect_takes_a_stroke_style_and_refuses_what_it_cannot_use() {
    let f = fixture(true, false).await;
    let rect = |extra: serde_json::Value| {
        let mut args = json!({ "x": 2, "y": 2, "width": 30, "height": 10, "stroke": "FF0000" });
        args.as_object_mut()
            .expect("object")
            .extend(extra.as_object().expect("object").clone());
        args
    };
    f.ok("draw_rect", rect(json!({ "stroke_style": "Dashed" })))
        .await;
    for style in ["hatched", "Ragged", "herringbone", "ROPE", "zigzag"] {
        f.ok("draw_rect", rect(json!({ "stroke_style": style })))
            .await;
    }

    let before = f.revision().await;
    let (bad, message) = f
        .tool("draw_rect", rect(json!({ "stroke_style": "wavy" })))
        .await;
    assert!(bad && message.to_string().contains("wavy"), "{message}");
    assert!(
        message.to_string().contains("zigzag"),
        "the error lists the names: {message}"
    );
    let (bad, message) = f
        .tool(
            "draw_rect",
            json!({ "x": 2, "y": 2, "width": 3, "height": 3, "stroke_style": "dotted" }),
        )
        .await;
    assert!(bad && message.to_string().contains("stroke"), "{message}");
    assert_eq!(f.revision().await, before, "a refused call changes nothing");
}
