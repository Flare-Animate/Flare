//! Flipnote Studio files through the MCP server: `import_project` reads a `.ppm` and a `.kwz`
//! from the export directory into new projects, and the open project is not changed.

mod common;

use common::serve;
use serde_json::json;

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!(
        "{}/../../crates/io/flare-import/tests/fixtures/flipnote/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("the fixture")
}

#[tokio::test]
async fn flipnote_files_import_through_the_server_into_new_projects() {
    let dir = tempfile::tempdir().expect("dir");
    let project = dir.path().join("p.flare");
    let document = flare_import::import_bytes(fixture("blank.kwz"))
        .expect("imports")
        .document;
    flare_storage::save(&document, &project).expect("save");
    let f = serve(dir, project, true, true, vec![]).await;
    let exports = f.dir.path().join("exports");
    std::fs::create_dir_all(&exports).expect("dir");
    std::fs::write(exports.join("pic.ppm"), fixture("blank.ppm")).expect("write");
    std::fs::write(exports.join("note.kwz"), fixture("blank.kwz")).expect("write");

    let ppm = f
        .ok(
            "import_project",
            json!({ "source": "pic.ppm", "name": "from_ppm" }),
        )
        .await;
    assert_eq!(ppm["scenes"], 1, "{ppm}");
    let loaded =
        flare_storage::load(std::path::Path::new(ppm["path"].as_str().expect("p"))).expect("opens");
    assert_eq!(loaded.scenes()[0].timeline().layers().len(), 3);

    let kwz = f
        .ok(
            "import_project",
            json!({ "source": "note.kwz", "name": "from_kwz" }),
        )
        .await;
    let loaded =
        flare_storage::load(std::path::Path::new(kwz["path"].as_str().expect("p"))).expect("opens");
    assert_eq!(loaded.scenes()[0].timeline().layers().len(), 4);
    assert!(kwz.to_string().contains("sound is not imported"), "{kwz}");

    let (is_error, _) = f
        .tool(
            "import_project",
            json!({ "source": "missing.kwz", "name": "x" }),
        )
        .await;
    assert!(is_error);
}
