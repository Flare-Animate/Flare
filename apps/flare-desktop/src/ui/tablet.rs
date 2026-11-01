//! Pen pressure on Linux. winit passes a tablet pen to the program as a mouse, so the pen is read
//! from the X server by `flare-platform` and shared with the workspace; this file decides the
//! window system and says which window to listen on.

use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

/// Whether the person turned this off: `FLARE_TABLET=off` keeps the native window system.
#[cfg(target_os = "linux")]
fn disabled() -> bool {
    std::env::var("FLARE_TABLET").is_ok_and(|v| v.eq_ignore_ascii_case("off"))
}

/// Opens the window through X11 when a pen with pressure is plugged in. Wayland would be chosen
/// otherwise, and winit has no tablet support there; XWayland does, since the compositor feeds it
/// from its own tablet protocol. Without a pen, or without an X server to ask, nothing changes.
#[cfg(target_os = "linux")]
pub(super) fn prefer_x11<T: 'static>(builder: &mut winit::event_loop::EventLoopBuilder<T>) {
    use winit::platform::x11::EventLoopBuilderExtX11;
    if !disabled()
        && std::env::var_os("DISPLAY").is_some()
        && flare_platform::tablet::has_pressure_device()
    {
        builder.with_x11();
    }
}

/// The X11 window to listen on, if the window is one.
fn x11_window(window: &Window) -> Option<u32> {
    match window.window_handle().ok()?.as_raw() {
        RawWindowHandle::Xlib(handle) => u32::try_from(handle.window).ok(),
        RawWindowHandle::Xcb(handle) => Some(handle.window.get()),
        _ => None,
    }
}

/// Starts reading the pen's pressure into `feed` when `window` is an X11 window. A window of any
/// other system (macOS, Windows, native Wayland) reports force through winit, or not at all.
pub(super) fn watch(window: &Window, feed: flare_animate::PenFeed) {
    let Some(id) = x11_window(window) else { return };
    if let Err(error) = flare_platform::tablet::watch(id, feed) {
        eprintln!("{error}");
    }
}
