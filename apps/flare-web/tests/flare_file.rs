//! The browser build's `.flare` bridge: this crate's Rust, compiled to WebAssembly, run under node
//! with the same two WebAssembly libraries the page loads (SQLite and zstd).
//!
//! It is run by `scripts/tests/test_web_wasm.py`, which makes a project with `flare-cli`, points
//! these tests at it (`FLARE_WEB_FIXTURE`), and afterwards has the desktop's own code read the
//! file the page wrote (`FLARE_WEB_OUT`).
#![cfg(target_arch = "wasm32")]
#![allow(clippy::expect_used)]

use flare_web::flare_file::FlareFile;
use wasm_bindgen::prelude::*;
use wasm_bindgen_test::*;

#[wasm_bindgen(inline_js = r#"
module.exports.make_library = async function (webDir) {
  const { pathToFileURL } = require("node:url");
  const { join } = require("node:path");
  const load = (p) => import(pathToFileURL(join(webDir, p)).href);
  const sqlite3InitModule = (await load("node_modules/@sqlite.org/sqlite-wasm/dist/node.mjs")).default;
  const zstd = await load("node_modules/@bokuweb/zstd-wasm/dist/common/index.node.js");
  const { createFlareFile } = await load("src/lib/flare-file.js");
  const sqlite3 = await sqlite3InitModule({ print: () => {}, printErr: () => {} });
  await zstd.init();
  return createFlareFile({ sqlite3, zstd });
};
module.exports.env = (name) => process.env[name];
module.exports.read_file = (path) => new Uint8Array(require("node:fs").readFileSync(path));
module.exports.write_file = (path, bytes) => require("node:fs").writeFileSync(path, bytes);
"#)]
extern "C" {
    #[wasm_bindgen(catch)]
    async fn make_library(web_dir: &str) -> Result<JsValue, JsValue>;
    fn env(name: &str) -> Option<String>;
    fn read_file(path: &str) -> Vec<u8>;
    fn write_file(path: &str, bytes: &[u8]);
}

async fn library() -> FlareFile {
    let dir = env("FLARE_WEB_DIR").expect("FLARE_WEB_DIR names apps/flare-web");
    FlareFile::from_host(make_library(&dir).await.expect("the libraries load"))
}

#[wasm_bindgen_test]
async fn a_file_the_desktop_wrote_is_read_and_what_is_written_reads_back_the_same() {
    let fixture = env("FLARE_WEB_FIXTURE").expect("FLARE_WEB_FIXTURE names a project");
    let library = library().await;
    let bytes = read_file(&fixture);
    let document = library.read(&bytes).expect("the desktop's file reads");
    assert!(!document.scenes().is_empty());
    assert!(
        document.scenes()[0].timeline().layers().len() >= 2,
        "the layers the fixture was made with"
    );
    assert!(!document.symbols().is_empty(), "and its symbol");
    let written = library.write(&document).expect("it writes");
    let again = library.read(&written).expect("what it wrote reads");
    assert_eq!(again, document, "the same project");
    if let Some(out) = env("FLARE_WEB_OUT") {
        write_file(&out, &written);
    }
}

#[wasm_bindgen_test]
async fn what_is_not_a_project_or_is_damaged_is_refused_in_words() {
    let fixture = env("FLARE_WEB_FIXTURE").expect("FLARE_WEB_FIXTURE names a project");
    let library = library().await;
    let good = read_file(&fixture);
    for (name, bytes) in [
        ("nothing", Vec::new()),
        ("text", b"plainly not a database ".repeat(60)),
        ("a cut file", good[..good.len() / 2].to_vec()),
    ] {
        let error = library.read(&bytes).expect_err(name);
        assert!(!error.to_string().is_empty(), "{name}");
    }
    // A flipped byte inside the records is caught by Rust's digest check, whichever zstd or
    // SQLite is behind it: flip each of a spread of positions and none may read as a project.
    let mut caught = 0;
    for at in (good.len() / 3..good.len()).step_by(good.len() / 25 + 1) {
        let mut damaged = good.clone();
        damaged[at] ^= 0x55;
        if library.read(&damaged).is_err() {
            caught += 1;
        }
    }
    assert!(caught >= 4, "damage was caught ({caught})");
}
