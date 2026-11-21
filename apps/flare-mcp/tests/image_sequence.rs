//! Numbered pictures in the export directory become a project with a keyframe each, through the
//! same reader as `flare import-sequence` and File > Import > Image Sequence.
#![allow(clippy::expect_used)]

mod common;

use common::fixture;
use serde_json::json;

fn png(value: u8) -> Vec<u8> {
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, 1, 1);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .expect("header")
        .write_image_data(&[value, 0, 0, 255])
        .expect("data");
    out
}

#[tokio::test]
async fn a_folder_of_pictures_becomes_a_project_and_what_is_wrong_is_refused() {
    let f = fixture(false, true).await;
    let folder = f.dir.path().join("exports").join("walk");
    std::fs::create_dir_all(&folder).expect("folder");
    for (name, value) in [("w_10.png", 1), ("w_2.png", 2), ("w_1.png", 3)] {
        std::fs::write(folder.join(name), png(value)).expect("write");
    }
    let made = f
        .ok(
            "import_image_sequence",
            json!({ "sources": ["walk"], "name": "walk" }),
        )
        .await;
    assert_eq!(made["pictures"], 3);
    assert!(f.dir.path().join("exports").join("walk.flare").exists());
    for (arguments, why) in [
        (
            json!({ "sources": ["walk"], "name": "walk" }),
            "already exists",
        ),
        (
            json!({ "sources": ["absent"], "name": "x" }),
            "not in the export",
        ),
        (json!({ "sources": ["../walk"], "name": "y" }), ""),
    ] {
        let (failed, message) = f.tool("import_image_sequence", arguments).await;
        assert!(failed && message.to_string().contains(why), "{message}");
    }
}
