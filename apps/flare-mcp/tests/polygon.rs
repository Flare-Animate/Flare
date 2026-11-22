//! The Polygon tool through the MCP server: corners become one shape that renders.
#![allow(clippy::expect_used)]

mod common;

use common::fixture;
use serde_json::json;

fn pixel(path: &str, x: u32, y: u32) -> [u8; 4] {
    let file = std::io::BufReader::new(std::fs::File::open(path).expect("open"));
    let mut reader = png::Decoder::new(file).read_info().expect("info");
    let mut buf = vec![0; reader.output_buffer_size().expect("size")];
    let info = reader.next_frame(&mut buf).expect("frame");
    let at = ((y * info.width + x) * 4) as usize;
    [buf[at], buf[at + 1], buf[at + 2], buf[at + 3]]
}

#[tokio::test]
async fn draw_polygon_makes_one_shape_that_renders_and_undoes_in_one_step() {
    // The fixture's stage is 40 by 20.
    let f = fixture(true, true).await;
    let before = f.revision().await;
    let drawn = f
        .ok(
            "draw_polygon",
            json!({ "points": [[2, 2], [38, 2], [20, 18]], "color": "00FF00", "stroke": "000000" }),
        )
        .await;
    assert_eq!(drawn["corners"], 3, "{drawn}");
    assert!(drawn["revision"].as_u64().expect("revision") > before);
    let out = f
        .ok("export_png", json!({ "frame": 0, "name": "tri" }))
        .await;
    let path = out["path"].as_str().expect("path");
    assert_eq!(pixel(path, 20, 7), [0, 255, 0, 255], "inside");
    assert_eq!(pixel(path, 2, 17)[..3], [255, 255, 255], "outside");
    f.ok("undo", json!({})).await;
    let info = f.ok("get_info", json!({})).await;
    let elements: u64 = info["scenes"][0]["layers"]
        .as_array()
        .expect("layers")
        .iter()
        .filter_map(|l| l["elements"].as_u64())
        .sum();
    assert_eq!(elements, 0, "{info}");
}

#[tokio::test]
async fn too_few_corners_a_line_or_a_style_without_a_stroke_are_refused() {
    let f = fixture(true, false).await;
    let before = f.revision().await;
    for args in [
        json!({ "points": [[0, 0], [5, 5]] }),
        json!({ "points": [[0, 0], [5, 5], [10, 10]] }),
        json!({ "points": [[0, 0], [5, 0], [0, 5]], "stroke_style": "dashed" }),
    ] {
        let (is_error, message) = f.tool("draw_polygon", args).await;
        assert!(is_error, "{message}");
    }
    assert_eq!(f.revision().await, before);
    let read_only = fixture(false, false).await;
    let (is_error, _) = read_only
        .tool(
            "draw_polygon",
            json!({ "points": [[0, 0], [5, 0], [0, 5]] }),
        )
        .await;
    assert!(is_error);
}
