//! Projects imported through the server: each keeps its original, which an export gives back
//! until the project is edited, and the source file is left alone.
#![allow(clippy::expect_used)]

mod common;

use common::{Fixture, fixture, fixture_of};
use serde_json::json;

async fn drawn() -> Fixture {
    let f = fixture(true, true).await;
    f.ok(
        "draw_rect",
        json!({ "x": 2, "y": 2, "width": 10, "height": 10, "color": "FF0000" }),
    )
    .await;
    f
}

fn bytes(path: &serde_json::Value) -> Vec<u8> {
    std::fs::read(path.as_str().expect("path")).expect("file")
}

fn sample_fla() -> Vec<u8> {
    std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../crates/io/flare-import/tests/fixtures/sample.fla"
    ))
    .expect("the fixture")
}

#[tokio::test]
async fn an_imported_project_exports_as_the_original_until_it_is_edited() {
    let original = sample_fla();
    let imported = flare_import::import_bytes(original.clone())
        .expect("imports")
        .document;
    let f = fixture_of(&imported, true, true, vec![]).await;

    let same = f.ok("export_xfl", json!({ "name": "same.fla" })).await;
    assert_eq!(same["lossless"]["exact"], true, "{same}");
    assert_eq!(bytes(&same["path"]), original);

    f.ok("rename_scene", json!({ "name": "Renamed" })).await;
    let edited = f.ok("export_xfl", json!({ "name": "edited.fla" })).await;
    assert_eq!(edited["lossless"]["exact"], false);
    assert!(
        edited["lossless"]["reason"]
            .as_str()
            .expect("r")
            .contains("edited since"),
        "{edited}"
    );
    assert_ne!(bytes(&edited["path"]), original);

    f.ok("undo", json!({})).await;
    let back = f.ok("export_xfl", json!({ "name": "back.fla" })).await;
    assert_eq!(
        bytes(&back["path"]),
        original,
        "undone, it is the original again"
    );
}

#[tokio::test]
async fn a_project_that_was_not_imported_says_its_export_is_rebuilt() {
    let f = drawn().await;
    let out = f.ok("export_xfl", json!({ "name": "new.fla" })).await;
    assert_eq!(out["lossless"]["exact"], false);
    assert!(
        out["lossless"]["reason"]
            .as_str()
            .expect("r")
            .contains("not imported from a file")
    );
}

#[tokio::test]
async fn importing_through_the_server_keeps_the_original_and_leaves_the_source_alone() {
    let f = drawn().await;
    let exports = f.dir.path().join("exports");
    std::fs::create_dir_all(&exports).expect("dir");
    let source = exports.join("src.fla");
    std::fs::write(&source, sample_fla()).expect("write");
    let before = std::fs::metadata(&source)
        .expect("meta")
        .modified()
        .expect("time");
    let imported = f
        .ok(
            "import_project",
            json!({ "source": "src.fla", "name": "imp" }),
        )
        .await;
    assert_eq!(imported["original_kept"], true);
    assert_eq!(std::fs::read(&source).expect("read"), sample_fla());
    assert_eq!(
        std::fs::metadata(&source)
            .expect("meta")
            .modified()
            .expect("time"),
        before
    );
    let loaded = flare_storage::load(std::path::Path::new(imported["path"].as_str().expect("p")))
        .expect("opens");
    assert!(
        loaded.origin().is_some(),
        "the project file kept the source"
    );
}

#[tokio::test]
async fn a_flipaclip_project_in_the_export_directory_imports_by_its_extension() {
    let f = drawn().await;
    let gif = f.ok("export_gif", json!({ "name": "loop" })).await;
    let export_dir = std::path::Path::new(gif["path"].as_str().expect("p"))
        .parent()
        .expect("the export directory")
        .to_path_buf();
    let meta = serde_json::json!({
        "format": 1, "canvasWidth": 8, "canvasHeight": 6, "fps": 12, "frameCount": 2,
        "name": "Sample", "version": 4,
        "layersState": "[{\"id\":1,\"name\":\"Capa 1\"}]",
    })
    .to_string();
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let stored =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    zip.start_file("meta", stored).expect("starts");
    std::io::Write::write_all(&mut zip, meta.as_bytes()).expect("writes");
    let bytes = zip.finish().expect("finishes").into_inner();
    std::fs::write(export_dir.join("sample.fc"), bytes).expect("writes the project");
    let imported = f
        .ok(
            "import_project",
            json!({ "source": "sample.fc", "name": "fc" }),
        )
        .await;
    assert_eq!(imported["scenes"], 1);
    assert!(imported["path"].as_str().expect("p").ends_with("fc.flare"));
}

#[tokio::test]
async fn a_moho_project_imports_with_its_pictures_read_from_the_export_folder() {
    let f = fixture(true, true).await;
    let exports = f.dir.path().join("exports");
    std::fs::create_dir_all(exports.join("art")).expect("art folder");
    let mut png = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png, 1, 1);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().expect("header");
        writer
            .write_image_data(&[255, 255, 255, 255])
            .expect("data");
        writer.finish().expect("finish");
    }
    std::fs::write(exports.join("art/photo.png"), &png).expect("picture");
    std::fs::write(
        exports.join("scene.mohoproj"),
        r#"{"version": 1038, "major_version": 13,
          "project_data": {"width": 40, "height": 20, "fps": 24.0, "end_frame": 1},
          "layers": [
            {"name": "Photo", "type": "ImageLayer", "visible": true, "parent_bone": -2, "image_path": "art/photo.png"},
            {"name": "Escape", "type": "ImageLayer", "visible": true, "parent_bone": -2, "image_path": "../../secret.png"}
          ]}"#,
    )
    .expect("project");
    let imported = f
        .ok(
            "import_project",
            json!({ "source": "scene.mohoproj", "name": "from-moho" }),
        )
        .await;
    let diagnostics = imported["diagnostics"].as_array().expect("diagnostics");
    assert!(
        diagnostics
            .iter()
            .any(|d| d.to_string().contains("leaves the project folder")),
        "{diagnostics:?}"
    );
    let loaded = flare_storage::load(std::path::Path::new(imported["path"].as_str().expect("p")))
        .expect("the imported project opens");
    assert_eq!(loaded.bitmaps().len(), 1, "the picture inside the folder");
}
