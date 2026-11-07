//! Wick Editor projects through the MCP server: `import_project` reads a `.wick` from the export
//! directory into a new project, and the open project is not changed.
#![allow(clippy::expect_used)]

mod common;

use common::serve;
use serde_json::json;

fn fixture() -> Vec<u8> {
    std::fs::read(format!(
        "{}/../../crates/io/flare-import/tests/fixtures/wick/synthetic.wick",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("the fixture")
}

#[tokio::test]
async fn a_wick_project_imports_through_the_server_into_a_new_project() {
    let dir = tempfile::tempdir().expect("dir");
    let project = dir.path().join("p.flare");
    let document = flare_import::import_bytes(
        std::fs::read(format!(
            "{}/../../crates/io/flare-import/tests/fixtures/flipnote/blank.kwz",
            env!("CARGO_MANIFEST_DIR")
        ))
        .expect("the fixture"),
    )
    .expect("imports")
    .document;
    flare_storage::save(&document, &project).expect("save");
    let f = serve(dir, project, true, true, vec![]).await;
    let exports = f.dir.path().join("exports");
    std::fs::create_dir_all(&exports).expect("dir");
    std::fs::write(exports.join("demo.wick"), fixture()).expect("write");

    let imported = f
        .ok(
            "import_project",
            json!({ "source": "demo.wick", "name": "from_wick" }),
        )
        .await;
    assert_eq!(imported["scenes"], 1, "{imported}");
    let loaded = flare_storage::load(std::path::Path::new(imported["path"].as_str().expect("p")))
        .expect("opens");
    assert_eq!(loaded.scenes()[0].timeline().layers().len(), 1);
    assert_eq!(loaded.symbols().len(), 1, "the clip becomes a symbol");
}
