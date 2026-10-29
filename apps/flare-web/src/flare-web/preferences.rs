//! Preferences kept in the browser's local storage. Storage can be missing or refuse a write (a
//! private window, a full quota), and then the settings simply last as long as the page.

const KEY: &str = "flare.preferences";

fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

pub fn load() -> Option<String> {
    storage()?.get_item(KEY).ok().flatten()
}

pub fn save(json: &str) {
    if let Some(storage) = storage() {
        let _ = storage.set_item(KEY, json);
    }
}
