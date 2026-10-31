//! The tools that write the project out in other formats, and their confinement to the export
//! directory.
#![allow(clippy::expect_used)]

mod common;

use common::{Fixture, fixture};
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
    assert!(
        gif["path"]
            .as_str()
            .expect("p")
            .ends_with("exports/loop.gif")
    );
    assert!(bytes(&gif["path"]).starts_with(b"GIF89a"));

    let mp4 = f.ok("export_mp4", json!({ "name": "video" })).await;
    assert!(
        mp4["path"]
            .as_str()
            .expect("p")
            .ends_with("exports/video.mp4")
    );
    assert_eq!(&bytes(&mp4["path"])[4..8], b"ftyp");

    let swf = f.ok("publish_swf", json!({ "name": "movie" })).await;
    assert!(
        swf["path"]
            .as_str()
            .expect("p")
            .ends_with("exports/movie.swf")
    );
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
            .ends_with("exports/back.flare")
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
