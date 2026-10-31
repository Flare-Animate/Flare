//! Test Movie in the page: the published SWF goes to Ruffle, which the page runs as WebAssembly
//! (`src/lib/flare-ruffle.js`, registered as `globalThis.flareRuffle`). The desktop app starts an
//! external Ruffle for the same request; here the player is part of the page.

use js_sys::{Function, Promise, Reflect, Uint8Array};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

fn describe(error: &JsValue) -> String {
    error
        .dyn_ref::<js_sys::Error>()
        .map(|e| String::from(e.message()))
        .or_else(|| error.as_string())
        .unwrap_or_else(|| "an unknown error".to_owned())
}

/// Plays `swf` in the page's player, from `host` (the registered `flareRuffle`). Resolves when
/// the player has the movie, or says why it could not.
pub async fn play_in(host: &JsValue, swf: &[u8]) -> Result<(), String> {
    let play = Reflect::get(host, &JsValue::from_str("play"))
        .ok()
        .and_then(|f| f.dyn_into::<Function>().ok())
        .ok_or_else(|| "the page has no movie player".to_owned())?;
    let promise = play
        .call1(host, &Uint8Array::from(swf))
        .map_err(|e| describe(&e))?;
    JsFuture::from(Promise::resolve(&promise))
        .await
        .map(|_| ())
        .map_err(|e| describe(&e))
}

/// The player the page registered as `globalThis.flareRuffle`.
pub fn registered() -> Result<JsValue, String> {
    Reflect::get(&js_sys::global(), &JsValue::from_str("flareRuffle"))
        .ok()
        .filter(|v| !v.is_undefined() && !v.is_null())
        .ok_or_else(|| "the page did not load its movie player".to_owned())
}
