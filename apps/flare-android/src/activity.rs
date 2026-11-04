//! The NativeActivity entry of the app: where a phone differs from a desk, and then the
//! desktop shell takes over.
//!
//! Compiled only for the android target (it cannot be exercised on a machine without the
//! loader; see the crate's own note and docs/SURFACES.md). Everything under it is tested
//! elsewhere: [`crate::open`] here, the shell in `flare-desktop`.

use flare_animate::{MAX_LAYOUT_BYTES, MAX_PREFERENCES_BYTES};
use flare_platform::dirs;
use std::path::PathBuf;
use winit::platform::android::activity::AndroidApp;

/// The system loads the app here. A start that goes wrong is announced in the log (the
/// loader names this process's streams in the log of a debug build); there is no window to
/// show a box in.
#[unsafe(no_mangle)]
pub fn android_main(app: AndroidApp) {
    if let Err(error) = start(app) {
        eprintln!("flare-android: {error:#}");
    }
}

fn start(app: AndroidApp) -> anyhow::Result<()> {
    // The phone gives the app one directory of its own, found nowhere else: everything saved
    // and recovered is under it, where the desktop would use its platform's own places. Once,
    // before anything is asked for a path.
    if let Some(data) = app.internal_data_path() {
        dirs::set_app_root(PathBuf::from(data));
    }
    let saved = |name: &str| {
        app.internal_data_path()
            .map(|root| PathBuf::from(root).join(name))
            .and_then(|path| dirs::read_layout(&path, MAX_LAYOUT_BYTES))
    };
    let layout = saved("layout.json");
    let preferences = saved("preferences.json");
    // One workspace, like a plain desktop launch: the project named by the launcher (there is
    // none) and the work recovered from the last run (the shell offers it, not this start).
    flare_shell::ui::run_on_activity(crate::open::start(layout, preferences)?, app)
}
