//! Pen pressure on Linux and macOS. winit passes a tablet pen to the program as a mouse, so the
//! pen is read from the system by `flare-tablet` (the X server, or AppKit's events) and shared
//! with the workspace; this file decides the window system and says which window to listen on.

#[cfg(not(target_os = "macos"))]
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

/// Whether the person turned this off: `FLARE_TABLET=off` keeps the native window system on
/// Linux, and leaves the pen a mouse on macOS.
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn disabled() -> bool {
    std::env::var("FLARE_TABLET").is_ok_and(|v| v.eq_ignore_ascii_case("off"))
}

/// Opens the window through X11 when a pen is plugged in. Wayland would be chosen
/// otherwise, and winit has no tablet support there; XWayland does, since the compositor feeds it
/// from its own tablet protocol. Without a pen, or without an X server to ask, nothing changes.
#[cfg(target_os = "linux")]
pub(super) fn prefer_x11<T: 'static>(builder: &mut winit::event_loop::EventLoopBuilder<T>) {
    use winit::platform::x11::EventLoopBuilderExtX11;
    if !disabled() && std::env::var_os("DISPLAY").is_some() && flare_tablet::probe::pen_attached() {
        builder.with_x11();
    }
}

/// Whether the window is an X11 one. On native Wayland `$DISPLAY` is XWayland, which sees none
/// of this window's input, so the pen reader must not start there.
#[cfg(not(target_os = "macos"))]
fn is_x11(window: &Window) -> bool {
    window.window_handle().is_ok_and(|handle| {
        matches!(
            handle.as_raw(),
            RawWindowHandle::Xlib(_) | RawWindowHandle::Xcb(_)
        )
    })
}

/// Starts reading the pen's pressure into `feed` when `window` is an X11 window. A window of any
/// other system (Windows, native Wayland) reports force through winit, or not at all.
#[cfg(not(target_os = "macos"))]
pub(super) fn watch(window: &Window, feed: flare_animate::PenFeed) {
    if !is_x11(window) {
        return;
    }
    // Said on the terminal so a person whose pen shows no pressure can tell whether it was found.
    match flare_tablet::x11::watch(feed) {
        Ok(pens) if pens.is_empty() => {}
        Ok(pens) => eprintln!("pen pressure from: {}", pens.join(", ")),
        Err(error) => eprintln!("{error}"),
    }
}

/// Starts reading the pen's pressure into `feed` from AppKit's events. It is called on the main
/// thread once `window` exists, which is when AppKit can be asked; the events are the
/// program's, not one window's.
#[cfg(target_os = "macos")]
pub(super) fn watch(_window: &Window, feed: flare_animate::PenFeed) {
    if disabled() {
        return;
    }
    if let Err(error) = flare_tablet::appkit::watch(feed) {
        eprintln!("{error}");
    }
}
