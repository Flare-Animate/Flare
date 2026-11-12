//! The program's icon where the system shows it while the program runs. A packaged app takes its
//! icon from its bundle, but a program run as it was built (`cargo run`, or the bare binary)
//! has none, and macOS puts a generic "exec" picture in the Dock for it; so the Dock is given
//! the icon at start, unless the program is inside a bundle that already gives one.

use std::path::Path;

/// Whether `exe` is the program inside a macOS app bundle (`….app/Contents/MacOS/…`), whose
/// own icon the system already shows.
pub fn in_app_bundle(exe: &Path) -> bool {
    let mut parts = exe.components().rev().skip(1).map(|c| c.as_os_str());
    matches!(
        (parts.next(), parts.next(), parts.next()),
        (Some(macos), Some(contents), Some(app))
            if macos == "MacOS"
                && contents == "Contents"
                && app.to_string_lossy().ends_with(".app")
    )
}

/// Shows `png` as the program's icon in the Dock, on macOS for a program outside a bundle.
/// Elsewhere, and when the picture cannot be read, it does nothing: the generic icon is only a
/// look. Call it on the main thread once the application exists (after the event loop starts).
pub fn show_in_dock(png: &[u8]) {
    #[cfg(target_os = "macos")]
    {
        let bundled = std::env::current_exe().is_ok_and(|exe| in_app_bundle(&exe));
        if !bundled {
            crate::app_icon_macos::set_dock_icon(png);
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = png;
}
