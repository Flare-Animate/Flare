//! `transparent` on the export tools: the formats with an alpha channel leave the stage colour
//! out, and the others refuse it and write nothing.
#![allow(clippy::expect_used)]

mod common;

use common::fixture;
use serde_json::json;

fn alpha_at(path: &serde_json::Value, x: u32, y: u32) -> u8 {
    let bytes = std::fs::read(path.as_str().expect("path")).expect("png");
    let mut reader = png::Decoder::new(std::io::Cursor::new(bytes))
        .read_info()
        .expect("info");
    let mut pixels = vec![0; reader.output_buffer_size().expect("size")];
    let info = reader.next_frame(&mut pixels).expect("frame");
    pixels[((y * info.width + x) * 4 + 3) as usize]
}

#[tokio::test]
async fn transparent_exports_leave_the_stage_out_and_the_formats_without_alpha_refuse() {
    let f = fixture(true, true).await;
    f.ok(
        "draw_rect",
        json!({ "x": 0, "y": 0, "width": 10, "height": 10, "color": "FF0000" }),
    )
    .await;

    let clear = f
        .ok(
            "export_still",
            json!({ "name": "clear", "transparent": true }),
        )
        .await;
    let stage = f.ok("export_still", json!({ "name": "stage" })).await;
    assert_eq!(alpha_at(&clear["path"], 5, 5), 255, "the square");
    assert_eq!(alpha_at(&clear["path"], 30, 15), 0, "the empty stage");
    assert_eq!(
        alpha_at(&stage["path"], 30, 15),
        255,
        "over the stage colour"
    );

    let apng = f
        .ok(
            "export_apng",
            json!({ "name": "loop", "transparent": true }),
        )
        .await;
    assert_eq!(alpha_at(&apng["path"], 30, 15), 0);

    let gif = f
        .ok("export_gif", json!({ "name": "loop", "transparent": true }))
        .await;
    let gif = std::fs::read(gif["path"].as_str().expect("path")).expect("gif");
    assert!(
        gif.windows(4)
            .any(|w| w[..3] == [0x21, 0xF9, 0x04] && w[3] & 1 == 1),
        "the GIF has a see-through colour"
    );

    for (tool, arguments) in [
        ("export_mp4", json!({ "name": "film", "transparent": true })),
        (
            "export_still",
            json!({ "name": "photo", "format": "jpeg", "transparent": true }),
        ),
        (
            "export_movie",
            json!({ "name": "film", "format": "avi", "transparent": true }),
        ),
    ] {
        let (failed, said) = f.tool(tool, arguments).await;
        assert!(failed, "{tool}");
        assert!(
            said.to_string().contains("transparent background"),
            "{tool}: {said}"
        );
    }
    let exports = f.dir.path().join("exports");
    for name in ["film.mp4", "photo.jpg", "film.avi"] {
        assert!(!exports.join(name).exists(), "{name}: nothing is written");
    }
}
