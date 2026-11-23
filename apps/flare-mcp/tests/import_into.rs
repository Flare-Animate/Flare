//! Bringing a Flash movie into the open project through the MCP server, as File > Import >
//! Import to Stage and Import to Library.
#![allow(clippy::expect_used)]

mod common;

use common::fixture;
use serde_json::json;

#[tokio::test]
async fn a_published_movie_comes_back_in_on_a_layer_and_then_in_the_library() {
    let f = fixture(true, true).await;
    f.ok(
        "draw_rect",
        json!({ "x": 2, "y": 2, "width": 8, "height": 8, "color": "FF0000", "duration": 4 }),
    )
    .await;
    f.ok("publish_swf", json!({ "name": "intro" })).await;

    let placed = f
        .ok(
            "import_into_project",
            json!({ "source": "intro.swf", "frame": 1 }),
        )
        .await;
    assert_eq!(placed["name"], "intro", "{placed}");
    let info = f.ok("get_info", json!({})).await;
    let layers = info["scenes"][0]["layers"].as_array().expect("layers");
    assert_eq!(layers[0]["name"], "intro");
    assert_eq!(layers[0]["id"], placed["layer"]);

    let library = f
        .ok(
            "import_into_project",
            json!({ "source": "intro.swf", "library": true }),
        )
        .await;
    assert_eq!(library["name"], "intro 2", "{library}");
    assert!(library["layer"].is_null());
    let after = f.ok("get_info", json!({})).await;
    assert_eq!(
        after["scenes"][0]["layers"].as_array().map(Vec::len),
        Some(layers.len()),
        "the library import placed nothing"
    );
}

#[tokio::test]
async fn a_missing_or_broken_file_is_refused_and_a_read_only_server_refuses_to_write() {
    let f = fixture(true, true).await;
    let (is_error, message) = f
        .tool("import_into_project", json!({ "source": "nothing.swf" }))
        .await;
    assert!(is_error, "{message}");
    let exports = f.dir.path().join("exports");
    std::fs::create_dir_all(&exports).expect("exports");
    std::fs::write(exports.join("broken.swf"), b"FWS broken").expect("write");
    let before = f.revision().await;
    let (is_error, message) = f
        .tool("import_into_project", json!({ "source": "broken.swf" }))
        .await;
    assert!(is_error, "{message}");
    assert!(message.to_string().contains("cannot import"), "{message}");
    assert_eq!(f.revision().await, before, "nothing changed");
    let (is_error, message) = f
        .tool("import_into_project", json!({ "source": "../escape.swf" }))
        .await;
    assert!(is_error, "{message}");

    let read_only = fixture(false, true).await;
    let (is_error, _) = read_only
        .tool("import_into_project", json!({ "source": "intro.swf" }))
        .await;
    assert!(is_error);
}
