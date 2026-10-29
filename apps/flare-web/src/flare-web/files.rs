//! Opening and saving in a browser: a file picker to read from and a download to save to.

use js_sys::{Array, Uint8Array};
use wasm_bindgen::JsCast;

/// Offers `bytes` to the person as a download named `name`.
pub fn download(name: &str, mime: &str, bytes: &[u8]) -> Result<(), String> {
    let fail = |what: &str| format!("the browser would not {what}");
    let parts = Array::new();
    parts.push(&Uint8Array::from(bytes));
    let options = web_sys::BlobPropertyBag::new();
    options.set_type(mime);
    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &options)
        .map_err(|_| fail("make the file"))?;
    let url = web_sys::Url::create_object_url_with_blob(&blob).map_err(|_| fail("make a link"))?;
    let document = web_sys::window()
        .and_then(|w| w.document())
        .ok_or_else(|| fail("find the page"))?;
    let anchor = document
        .create_element("a")
        .map_err(|_| fail("make a link"))?
        .dyn_into::<web_sys::HtmlAnchorElement>()
        .map_err(|_| fail("make a link"))?;
    anchor.set_href(&url);
    anchor.set_download(name);
    anchor.click();
    // Browsers fetch the blob after `click` returns, so the address is released later.
    let release = wasm_bindgen::closure::Closure::once_into_js(move || {
        let _ = web_sys::Url::revoke_object_url(&url);
    });
    if let Some(window) = web_sys::window() {
        let _ = window
            .set_timeout_with_callback_and_timeout_and_arguments_0(release.unchecked_ref(), 60_000);
    }
    Ok(())
}

/// Asks for a file with one of `extensions` and reads it whole. `None` when the person cancels.
pub async fn pick(label: &str, extensions: &[&str]) -> Option<(String, Vec<u8>)> {
    let handle = rfd::AsyncFileDialog::new()
        .add_filter(label, extensions)
        .pick_file()
        .await?;
    Some((handle.file_name(), handle.read().await))
}
