//! Downloading the Google Fonts families text asks for, with the page's `fetch`: the desktop's
//! downloader (`flare-webfonts`) needs sockets a page does not have. The files come from the
//! addresses the catalog names (`flare_text::google`), which allow a page to read them; the
//! browser's own cache keeps them, and the workspace checks they are the family's fonts.

use js_sys::{ArrayBuffer, Uint8Array};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

fn describe(error: &JsValue) -> String {
    error
        .dyn_ref::<js_sys::Error>()
        .map(|e| String::from(e.message()))
        .or_else(|| error.as_string())
        .unwrap_or_else(|| "the download failed".to_owned())
}

/// The files of the family named `family`, downloaded one after another.
pub async fn download(family: &str) -> Result<Vec<Vec<u8>>, String> {
    let known = flare_text::google::family_named(family)
        .ok_or_else(|| format!("{family} is not a Google Fonts family"))?;
    let window = web_sys::window().ok_or("the page has no window")?;
    let mut files = Vec::new();
    for url in known.urls() {
        let response = JsFuture::from(window.fetch_with_str(&url))
            .await
            .map_err(|e| describe(&e))?
            .dyn_into::<web_sys::Response>()
            .map_err(|_| "the download gave no answer".to_owned())?;
        if !response.ok() {
            return Err(format!("{url} answered {}", response.status()));
        }
        // A size the server states is checked before the body is read.
        let stated = response
            .headers()
            .get("content-length")
            .ok()
            .flatten()
            .and_then(|length| length.parse::<usize>().ok());
        if stated.is_some_and(|length| length > flare_text::MAX_FONT_BYTES) {
            return Err(format!("{url} is too large"));
        }
        let buffer = JsFuture::from(response.array_buffer().map_err(|e| describe(&e))?)
            .await
            .map_err(|e| describe(&e))?
            .dyn_into::<ArrayBuffer>()
            .map_err(|_| "the download gave no bytes".to_owned())?;
        if buffer.byte_length() as usize > flare_text::MAX_FONT_BYTES {
            return Err(format!("{url} is too large"));
        }
        files.push(Uint8Array::new(&buffer).to_vec());
    }
    Ok(files)
}
