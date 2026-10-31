//! The browser build's hand-off to Ruffle: what the Rust side gives the page's player, and what
//! it does when the player says no. The player here is a fake that records what it is given; the
//! real one is checked in `scripts/tests/test_web_ruffle.py` against the real `flare-ruffle.js`.
#![cfg(target_arch = "wasm32")]
#![allow(clippy::expect_used)]

use flare_web::ruffle::play_in;
use wasm_bindgen::prelude::*;
use wasm_bindgen_test::*;

#[wasm_bindgen(inline_js = r#"
module.exports.recording_player = () => ({
  played: [],
  play(bytes) { this.played.push(bytes); return Promise.resolve(); },
});
module.exports.refusing_player = (message) => ({
  play() { return Promise.reject(new Error(message)); },
});
module.exports.throwing_player = (message) => ({
  play() { throw new Error(message); },
});
module.exports.played_count = (player) => player.played.length;
module.exports.played_at = (player, index) => player.played[index];
"#)]
extern "C" {
    fn recording_player() -> JsValue;
    fn refusing_player(message: &str) -> JsValue;
    fn throwing_player(message: &str) -> JsValue;
    fn played_count(player: &JsValue) -> usize;
    fn played_at(player: &JsValue, index: usize) -> Vec<u8>;
}

fn movie() -> Vec<u8> {
    use flare_core::{DocumentId, FrameRate};
    let document = flare_document::Document::new(
        DocumentId::new(),
        FrameRate::new(12, 1).expect("rate"),
        vec![],
        vec![],
    )
    .expect("document");
    flare_publish::publish_swf(&document, &flare_publish::PublishOptions::default())
        .expect("publish")
        .bytes
}

#[wasm_bindgen_test]
async fn the_published_movie_reaches_the_player_unchanged() {
    let swf = movie();
    assert!(swf.starts_with(b"CWS") || swf.starts_with(b"FWS"));
    let player = recording_player();
    play_in(&player, &swf).await.expect("played");
    assert_eq!(played_count(&player), 1);
    assert_eq!(played_at(&player, 0), swf);
}

#[wasm_bindgen_test]
async fn a_player_that_refuses_or_throws_is_an_error_with_its_message() {
    let refused = play_in(&refusing_player("no wasm for you"), &movie()).await;
    assert!(refused.expect_err("refused").contains("no wasm for you"));
    let thrown = play_in(&throwing_player("exploded"), &movie()).await;
    assert!(thrown.expect_err("thrown").contains("exploded"));
}

#[wasm_bindgen_test]
async fn a_page_without_a_player_says_so() {
    let nothing = play_in(&JsValue::from(js_sys::Object::new()), &movie()).await;
    assert!(nothing.expect_err("none").contains("no movie player"));
}
