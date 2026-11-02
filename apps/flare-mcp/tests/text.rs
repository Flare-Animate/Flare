//! Text objects through the MCP server.
#![allow(clippy::expect_used)]

mod common;

use common::{Fixture, fixture_with_fonts};
use serde_json::json;

async fn with_hack(write: bool) -> Fixture {
    fixture_with_fonts(
        write,
        true,
        vec![epaint_default_fonts::HACK_REGULAR.to_vec()],
    )
    .await
}

/// How many pixels are mostly `wanted`'s color: letters are anti-aliased, so few are exact.
fn colored(path: &str, wanted: [u8; 4]) -> usize {
    let file = std::io::BufReader::new(std::fs::File::open(path).expect("open"));
    let mut reader = png::Decoder::new(file).read_info().expect("info");
    let mut buf = vec![0; reader.output_buffer_size().expect("size")];
    let info = reader.next_frame(&mut buf).expect("frame");
    buf[..info.buffer_size()]
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|px| {
            (0..3).all(|c| {
                if wanted[c] == 255 {
                    px[c] > 200
                } else {
                    px[c] < 100
                }
            })
        })
        .count()
}

#[tokio::test]
async fn text_is_added_drawn_changed_and_taken_apart() {
    let f = with_hack(true).await;
    let added = f
        .ok(
            "add_text",
            json!({ "text": "Hi", "x": 2, "y": 1, "family": "Hack", "size": 14, "color": "FF0000" }),
        )
        .await;
    let shape = added["shape"].as_str().expect("id").to_owned();
    let png = f.ok("export_png", json!({ "frame": 0, "name": "a" })).await;
    let reds = colored(png["path"].as_str().expect("path"), [255, 0, 0, 255]);
    assert!(reds > 10, "the letters are drawn: {reds}");

    let before = f.revision().await;
    f.ok(
        "set_text",
        json!({ "shape": shape, "text": "Ho", "size": 12, "color": "0000FF" }),
    )
    .await;
    assert!(f.revision().await > before);
    let png = f.ok("export_png", json!({ "frame": 0, "name": "b" })).await;
    let path = png["path"].as_str().expect("path");
    assert_eq!(colored(path, [255, 0, 0, 255]), 0, "no longer red");
    assert!(colored(path, [0, 0, 255, 255]) > 10, "now blue");

    f.ok("break_text", json!({ "shape": shape })).await;
    let (is_error, message) = f
        .tool("set_text", json!({ "shape": shape, "size": 9 }))
        .await;
    assert!(is_error && message.as_str().expect("m").contains("not text"));

    // The undo steps are whole: taking apart, then changing, then adding.
    f.ok("undo", json!({})).await;
    f.ok("set_text", json!({ "shape": shape, "size": 9 })).await;
}

#[tokio::test]
async fn text_that_cannot_be_set_is_refused_and_a_read_only_server_does_not_change_it() {
    let f = with_hack(true).await;
    let before = f.revision().await;
    for args in [
        json!({ "text": "  ", "x": 0, "y": 0 }),
        json!({ "text": "x", "x": 0, "y": 0, "size": 0 }),
        json!({ "text": "x", "x": 0, "y": 0, "size": 100000 }),
        json!({ "text": "x", "x": 0, "y": 0, "color": "nope" }),
    ] {
        let (is_error, _) = f.tool("add_text", args.clone()).await;
        assert!(is_error, "{args}");
    }
    let (is_error, _) = f
        .tool(
            "set_text",
            json!({ "shape": "00000000-0000-0000-0000-000000000000", "size": 9 }),
        )
        .await;
    assert!(is_error);
    assert_eq!(f.revision().await, before, "nothing changed");

    let read_only = with_hack(false).await;
    let (is_error, message) = read_only
        .tool("add_text", json!({ "text": "x", "x": 0, "y": 0 }))
        .await;
    assert!(is_error && message.as_str().expect("m").contains("read-only"));
    let listed = read_only.ok("list_fonts", json!({})).await;
    assert!(
        listed["families"]
            .as_array()
            .expect("list")
            .iter()
            .any(|f| f == "Hack"),
        "{listed}"
    );
}
