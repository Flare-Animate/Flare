//! The save tools write Animate's formats: a `.fla` the server opened is saved back as a `.fla`,
//! and a project can be saved under a new name in any format.
#![allow(clippy::expect_used)]

mod common;

use common::{Fixture, serve};
use serde_json::json;
use std::path::Path;

const FLA: &[u8] = include_bytes!("../../../crates/io/flare-import/tests/fixtures/sample.fla");

async fn on_a_fla(write: bool) -> Fixture {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("work.fla");
    std::fs::write(&path, FLA).expect("write");
    serve(dir, path, write, true, vec![]).await
}

fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .expect("dir")
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[tokio::test]
async fn save_writes_an_unchanged_fla_back_as_the_original_and_an_edited_one_with_the_edit() {
    let f = on_a_fla(true).await;
    let saved = f.ok("save", json!({})).await;
    assert_eq!(saved["format"], "fla");
    assert_eq!(saved["lossless"]["exact"], true);
    assert_eq!(std::fs::read(&f.path).expect("read"), FLA);

    f.ok(
        "draw_rect",
        json!({ "x": 2, "y": 2, "width": 10, "height": 10, "color": "FF0000" }),
    )
    .await;
    let saved = f.ok("save", json!({})).await;
    assert_eq!(saved["lossless"]["exact"], false);
    assert_eq!(saved["rebuilt"], json!(["DOMDocument.xml"]));
    let reopened = flare_runtime::load(&f.path).expect("reopens");
    let layer = &reopened.document.scenes()[0].timeline().layers()[0];
    assert_eq!(layer.span_at(0).map(|s| s.element_count()), Some(1));
    assert!(
        !names(f.dir.path()).iter().any(|n| n.ends_with(".flare")),
        "no project file appears beside the .fla"
    );
}

#[tokio::test]
async fn save_as_makes_the_new_file_the_project_in_the_format_its_name_says() {
    let f = on_a_fla(true).await;
    let xfl = f.ok("save_as", json!({ "name": "copy.xfl" })).await;
    assert_eq!(xfl["format"], "xfl");
    let stub = f.dir.path().join("exports/copy/copy.xfl");
    assert!(stub.is_file(), "{xfl}");
    // Saving now writes to the new file.
    f.ok(
        "draw_rect",
        json!({ "x": 2, "y": 2, "width": 10, "height": 10, "color": "00FF00" }),
    )
    .await;
    f.ok("save", json!({})).await;
    assert_eq!(
        std::fs::read(&f.path).expect("read"),
        FLA,
        "the old .fla is untouched"
    );
    assert!(
        flare_runtime::load(&stub)
            .expect("reopens")
            .document
            .scenes()[0]
            .timeline()
            .layers()[0]
            .span_at(0)
            .is_some_and(|s| s.element_count() == 1)
    );
    let plain = f.ok("save_as", json!({ "name": "plain" })).await;
    assert_eq!(plain["format"], "fla", "no ending gets .fla");
    let flare = f.ok("save_as", json!({ "name": "p.flare" })).await;
    assert_eq!(flare["format"], "flare");
    assert!(flare_storage::load(Path::new(flare["saved"].as_str().expect("path"))).is_ok());
}

#[tokio::test]
async fn save_as_refuses_to_replace_and_a_read_only_server_cannot_save() {
    let f = on_a_fla(true).await;
    f.ok("save_as", json!({ "name": "once.fla" })).await;
    let (error, message) = f.tool("save_as", json!({ "name": "once.fla" })).await;
    assert!(error);
    assert!(message.to_string().contains("already exists"), "{message}");
    let (error, _) = f.tool("save_as", json!({ "name": "../escape.fla" })).await;
    assert!(error, "a name cannot leave the export directory");
    let read_only = on_a_fla(false).await;
    for call in [("save", json!({})), ("save_as", json!({ "name": "x.fla" }))] {
        let (error, message) = read_only.tool(call.0, call.1).await;
        assert!(error, "{} must need write access", call.0);
        assert!(message.to_string().contains("read-only"), "{message}");
    }
    assert_eq!(std::fs::read(&read_only.path).expect("read"), FLA);
}
