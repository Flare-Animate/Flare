//! The pen's pressure in the page. The browser's pointer events carry it, and eframe's web backend
//! passes a pen on as a mouse, so `src/lib/pen-pressure.js` reports each pointer event here and the
//! workspace reads it the way it reads a native tablet: through the shared `PenFeed`.

use std::cell::RefCell;

use flare_input::{PenFeed, pen_pressure};
use wasm_bindgen::prelude::wasm_bindgen;

thread_local! {
    /// The workspace's pen feed, set once when the workspace starts. The page is one thread, so a
    /// thread-local is the whole of the sharing.
    static FEED: RefCell<Option<PenFeed>> = const { RefCell::new(None) };
}

/// Hands the workspace's pen feed to the reports the page sends.
pub fn install(feed: PenFeed) {
    FEED.with(|slot| *slot.borrow_mut() = Some(feed));
}

/// Called by the page for each pointer event over the canvas: `pointer_type` is the event's
/// `pointerType` and `pressure` its pressure. Anything but a pen clears the feed, so the next
/// stroke is a mouse's full width again.
#[wasm_bindgen(js_name = flarePenReport)]
pub fn report(pointer_type: &str, pressure: f64) {
    FEED.with(|slot| {
        if let Some(feed) = slot.borrow().as_ref() {
            feed.set(pen_pressure(pointer_type, pressure));
        }
    });
}
