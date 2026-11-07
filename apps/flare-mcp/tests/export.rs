//! The tools that write the project out in other formats, and their confinement to the export
//! directory.
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

#[tokio::test]
async fn each_export_tool_writes_its_format_into_the_export_directory() {
    let f = drawn().await;

    let gif = f.ok("export_gif", json!({ "name": "loop" })).await;
    assert!(gif["path"].as_str().expect("p").ends_with("loop.gif"));
    assert!(bytes(&gif["path"]).starts_with(b"GIF89a"));

    let sheet = f
        .ok("export_sprite_sheet", json!({ "name": "sheet" }))
        .await;
    assert!(bytes(&sheet["path"]).starts_with(b"\x89PNG"));
    let atlas = String::from_utf8(bytes(&sheet["atlas"])).expect("utf8");
    assert!(atlas.contains("\"frame_00000\"") && atlas.contains("sheet.png"));

    let mp4 = f.ok("export_mp4", json!({ "name": "video" })).await;
    // Compared by path components: Windows separates them with a backslash.
    assert!(std::path::Path::new(mp4["path"].as_str().expect("p")).ends_with("exports/video.mp4"));
    assert_eq!(&bytes(&mp4["path"])[4..8], b"ftyp");

    let swf = f.ok("publish_swf", json!({ "name": "movie" })).await;
    // Compared by path components: Windows separates them with a backslash.
    assert!(std::path::Path::new(swf["path"].as_str().expect("p")).ends_with("exports/movie.swf"));
    assert!(matches!(&bytes(&swf["path"])[..3], b"CWS" | b"ZWS"));
    assert_eq!(swf["frames"], 1);
    let plain = f
        .ok(
            "publish_swf",
            json!({ "name": "plain", "no_compress": true }),
        )
        .await;
    assert_eq!(&bytes(&plain["path"])[..3], b"FWS");

    let html = f
        .ok(
            "publish_html5",
            json!({ "name": "page", "responsive": true }),
        )
        .await;
    let page = String::from_utf8(bytes(&html["path"])).expect("utf8");
    assert!(page.to_ascii_lowercase().contains("<canvas"));

    let xfl = f.ok("export_xfl", json!({ "name": "folder" })).await;
    assert_eq!(xfl["format"], "xfl");
    assert!(std::path::Path::new(xfl["path"].as_str().expect("p")).is_dir());
    let fla = f.ok("export_xfl", json!({ "name": "zipped.fla" })).await;
    assert_eq!(fla["format"], "fla");
    assert!(bytes(&fla["path"]).starts_with(b"PK"));
}

#[tokio::test]
async fn an_exported_project_imports_back_into_a_new_project_and_leaves_the_open_one_alone() {
    let f = drawn().await;
    let before = f.revision().await;
    f.ok("export_xfl", json!({ "name": "round.fla" })).await;
    let imported = f
        .ok(
            "import_project",
            json!({ "source": "round.fla", "name": "back" }),
        )
        .await;
    assert!(
        imported["path"]
            .as_str()
            .expect("p")
            .ends_with("back.flare")
    );
    assert_eq!(imported["scenes"], 1);
    let loaded = flare_storage::load(std::path::Path::new(imported["path"].as_str().expect("p")))
        .expect("the imported project opens");
    assert_eq!(loaded.scenes().len(), 1);
    assert_eq!(f.revision().await, before, "the open project is untouched");

    let (is_error, message) = f
        .tool(
            "import_project",
            json!({ "source": "missing.fla", "name": "x" }),
        )
        .await;
    assert!(
        is_error
            && message
                .as_str()
                .expect("m")
                .contains("not in the export directory")
    );
}

#[tokio::test]
async fn export_tools_are_off_without_a_directory_never_overwrite_and_refuse_names_that_escape() {
    let off = fixture(false, false).await;
    for (tool, args) in [
        ("export_gif", json!({ "name": "a" })),
        ("export_mp4", json!({ "name": "a" })),
        ("publish_swf", json!({ "name": "a" })),
        ("publish_html5", json!({ "name": "a" })),
        ("export_xfl", json!({ "name": "a" })),
        ("import_project", json!({ "source": "a", "name": "b" })),
    ] {
        let (is_error, message) = off.tool(tool, args).await;
        assert!(
            is_error && message.as_str().expect("m").contains("disabled"),
            "{tool}: {message}"
        );
    }

    let f = drawn().await;
    for (tool, args) in [
        ("export_gif", json!({ "name": "same" })),
        ("export_mp4", json!({ "name": "same" })),
        ("publish_swf", json!({ "name": "same" })),
        ("publish_html5", json!({ "name": "same" })),
        ("export_xfl", json!({ "name": "same.fla" })),
    ] {
        f.ok(tool, args.clone()).await;
        let (is_error, message) = f.tool(tool, args).await;
        assert!(
            is_error && message.as_str().expect("m").contains("already exists"),
            "{tool} overwrote: {message}"
        );
    }
    for tool in [
        "export_gif",
        "export_mp4",
        "publish_swf",
        "publish_html5",
        "export_xfl",
    ] {
        for name in [
            "../escape",
            "a/b",
            "/abs",
            ".hidden",
            "-flag",
            "",
            "with space",
        ] {
            let (is_error, _) = f.tool(tool, json!({ "name": name })).await;
            assert!(is_error, "{tool} {name:?} must be rejected");
        }
    }
    let (is_error, _) = f
        .tool(
            "import_project",
            json!({ "source": "../p.flare", "name": "x" }),
        )
        .await;
    assert!(
        is_error,
        "an import cannot reach outside the export directory"
    );
}

#[tokio::test]
async fn a_published_movie_imports_back_as_a_project() {
    let f = drawn().await;
    f.ok("publish_swf", json!({ "name": "movie.swf" })).await;
    let imported = f
        .ok(
            "import_project",
            json!({ "source": "movie.swf", "name": "from-movie" }),
        )
        .await;
    assert_eq!(imported["scenes"], 1);
    assert!(imported["symbols"].as_u64().expect("count") >= 1);
    let loaded = flare_storage::load(std::path::Path::new(imported["path"].as_str().expect("p")))
        .expect("the imported project opens");
    // The fixture's stage is 40 by 20.
    assert_eq!((loaded.stage().width(), loaded.stage().height()), (40, 20));
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

#[tokio::test]
async fn export_still_writes_each_format_and_refuses_a_quality_for_the_others() {
    let f = drawn().await;

    let png = f.ok("export_still", json!({ "name": "frame" })).await;
    assert!(png["path"].as_str().expect("p").ends_with("frame.png"));
    assert!(bytes(&png["path"]).starts_with(b"\x89PNG"));

    let jpeg = f
        .ok(
            "export_still",
            json!({ "name": "photo", "format": "jpeg", "quality": 80 }),
        )
        .await;
    assert!(jpeg["path"].as_str().expect("p").ends_with("photo.jpg"));
    assert!(bytes(&jpeg["path"]).starts_with(&[0xFF, 0xD8, 0xFF]));

    let bmp = f
        .ok("export_still", json!({ "name": "pic", "format": "bmp" }))
        .await;
    assert!(bytes(&bmp["path"]).starts_with(b"BM"));

    let (is_error, message) = f
        .tool(
            "export_still",
            json!({ "name": "png_q", "format": "png", "quality": 50 }),
        )
        .await;
    assert!(is_error, "a quality is refused for PNG");
    assert!(message.as_str().expect("m").contains("JPEG only"));

    let (is_error, message) = f
        .tool(
            "export_still",
            json!({ "name": "bad_q", "format": "jpeg", "quality": 0 }),
        )
        .await;
    assert!(is_error, "quality 0 is outside 1 to 100");
    assert!(message.as_str().expect("m").contains("JPEG quality 0"));
}

#[tokio::test]
async fn export_still_writes_the_new_formats_with_their_signatures() {
    let f = drawn().await;
    type Signature = fn(&[u8]) -> bool;
    let cases: [(&str, &str, Signature); 7] = [
        ("tiff", "t", |b| b.starts_with(b"II*\0")),
        ("webp", "w", |b| {
            b.starts_with(b"RIFF") && b.get(8..12) == Some(b"WEBP")
        }),
        ("qoi", "q", |b| b.starts_with(b"qoif")),
        ("pnm", "n", |b| b.starts_with(b"P7")),
        ("exr", "e", |b| b.starts_with(&[0x76, 0x2f, 0x31, 0x01])),
        ("hdr", "h", |b| b.starts_with(b"#?RADIANCE")),
        ("ico", "i", |b| b.starts_with(&[0, 0, 1, 0])),
    ];
    for (format, name, signature) in cases {
        let out = f
            .ok("export_still", json!({ "name": name, "format": format }))
            .await;
        assert!(signature(&bytes(&out["path"])), "{format}: wrong signature");
    }
}

#[tokio::test]
async fn export_svg_writes_a_vector_picture_and_its_notes() {
    let f = drawn().await;
    let out = f.ok("export_svg", json!({ "name": "vec" })).await;
    assert!(out["path"].as_str().expect("p").ends_with("vec.svg"));
    let text = String::from_utf8(bytes(&out["path"])).expect("utf-8");
    assert!(text.starts_with("<svg"), "{text}");
    assert!(out["notes"].is_array(), "the notes are reported");
}

#[tokio::test]
async fn export_psd_writes_a_photoshop_file() {
    let f = drawn().await;
    let out = f.ok("export_psd", json!({ "name": "flat" })).await;
    assert!(out["path"].as_str().expect("p").ends_with("flat.psd"));
    let data = bytes(&out["path"]);
    assert!(data.starts_with(b"8BPS"), "a Photoshop file");
    psd::Psd::from_bytes(&data).expect("the PSD reader accepts it");
}

#[tokio::test]
async fn export_animated_webp_writes_a_webp_and_export_movie_needs_ffmpeg_or_writes_one() {
    let f = drawn().await;
    let webp = f
        .ok("export_animated_webp", json!({ "name": "anim" }))
        .await;
    let data = bytes(&webp["path"]);
    assert!(data.starts_with(b"RIFF") && data.get(8..12) == Some(b"WEBP"));

    let (is_error, message) = f
        .tool("export_movie", json!({ "name": "clip", "format": "mkv" }))
        .await;
    if flare_ffmpeg::locate().is_ok() {
        assert!(!is_error, "{message}");
        assert!(
            f.dir.path().join("exports").join("clip.mkv").is_file(),
            "the movie is in the export directory"
        );
    } else {
        assert!(is_error, "without ffmpeg the movie is refused");
        assert!(message.as_str().expect("m").contains("ffmpeg"));
    }
}
