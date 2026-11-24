//! `merge` on the MCP drawing tools: Merge Drawing, as with Object Drawing off on the desktop.
#![allow(clippy::expect_used)]

mod common;

use common::fixture;
use serde_json::{Value, json};

fn pixel(path: &str, x: u32, y: u32) -> [u8; 4] {
    let file = std::io::BufReader::new(std::fs::File::open(path).expect("open"));
    let mut reader = png::Decoder::new(file).read_info().expect("info");
    let mut buf = vec![0; reader.output_buffer_size().expect("size")];
    let info = reader.next_frame(&mut buf).expect("frame");
    let at = ((y * info.width + x) * 4) as usize;
    [buf[at], buf[at + 1], buf[at + 2], buf[at + 3]]
}

/// The colour at `x, y` of frame 0 as the server renders it.
async fn at(f: &common::Fixture, x: u32, y: u32) -> [u8; 4] {
    let out = f.ok("export_png", json!({ "frame": 0, "name": "f" })).await;
    pixel(out["path"].as_str().expect("path"), x, y)
}

fn counts(info: &Value) -> (u64, u64) {
    let layers = info["scenes"][0]["layers"].as_array().expect("layers");
    layers.iter().fold((0, 0), |(e, m), l| {
        (
            e + l["elements"].as_u64().unwrap_or(0),
            m + l["merged"].as_u64().unwrap_or(0),
        )
    })
}

#[tokio::test]
async fn merged_rectangles_of_one_colour_join_in_one_undo_step() {
    let f = fixture(true, true).await;
    let square = |x: u32| json!({ "x": x, "y": 2, "width": 10, "height": 10, "color": "FF0000", "merge": true });
    f.ok("draw_rect", square(2)).await;
    let joined = f.ok("draw_rect", square(6)).await;
    assert_eq!(joined["elements"], json!([0]), "{joined}");
    assert_eq!(counts(&f.ok("get_info", json!({})).await), (1, 1));
    assert_eq!(
        at(&f, 14, 5).await,
        [255, 0, 0, 255],
        "the second square's part"
    );
    f.ok("undo", json!({})).await;
    assert_eq!(
        counts(&f.ok("get_info", json!({})).await),
        (1, 1),
        "the first, alone"
    );
    assert_eq!(
        at(&f, 14, 5).await[..3],
        [255, 255, 255],
        "undone in one step"
    );
    f.ok(
        "draw_rect",
        json!({ "x": 20, "y": 2, "width": 5, "height": 5 }),
    )
    .await;
    assert_eq!(
        counts(&f.ok("get_info", json!({})).await),
        (2, 1),
        "without merge, a drawing object"
    );
}

#[tokio::test]
async fn a_merged_polygon_and_brush_stroke_list_their_pieces() {
    let f = fixture(true, true).await;
    let polygon = f
        .ok(
            "draw_polygon",
            json!({ "points": [[2, 2], [38, 2], [20, 18]], "color": "00FF00", "stroke": "000000", "merge": true }),
        )
        .await;
    assert_eq!(
        polygon["elements"],
        json!([0, 1]),
        "a fill and a line: {polygon}"
    );
    f.ok(
        "brush_stroke",
        json!({ "strokes": [[[0, 10], [40, 10]]], "color": "00FF00", "stabilizer": 0, "merge": true }),
    )
    .await;
    let (elements, merged) = counts(&f.ok("get_info", json!({})).await);
    assert_eq!(elements, merged, "everything is merged drawing");
}
