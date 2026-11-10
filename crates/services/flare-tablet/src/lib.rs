//! A tablet pen's pressure from the operating system, for the window systems that do not pass it
//! on. `flare-input` holds what a pen reports whatever the system (`PenFeed`, `PressureAxis`);
//! this crate holds how each system is asked, one gated module per system, each writing into a
//! `PenFeed` the stage reads.
//!
//! - [`x11`]: XInput2 raw events from the X server, which winit does not pass on. Under Wayland
//!   it works through XWayland, so the desktop opens its window there when a pen is plugged in
//!   ([`probe`]).
//!
//! - [`appkit`]: a local event monitor on macOS, where winit drops the pressure of a pen's
//!   events.
//!
//! Windows (the pen's barrel button) will be a module here too. These need `unsafe`
//! operating-system calls, which is why this is not `flare-input`, a foundation crate every
//! other crate may use, and why `unsafe` is denied below and not forbidden: the modules that
//! make the calls, and only those, opt out, and what they decide is kept in a safe module beside
//! them.
#![deny(unsafe_code)]

use thiserror::Error;

#[cfg(target_os = "macos")]
pub mod appkit;
#[cfg(target_os = "macos")]
mod appkit_events;
#[cfg(all(unix, not(target_os = "ios")))]
pub mod probe;
#[cfg(all(unix, not(target_os = "ios")))]
pub mod x11;
#[cfg(all(unix, not(target_os = "ios")))]
mod x11_events;

#[cfg(all(test, target_os = "macos"))]
mod appkit_events_tests;
#[cfg(all(test, target_os = "macos"))]
mod appkit_tests;
#[cfg(all(test, unix, not(target_os = "ios")))]
mod probe_tests;
#[cfg(all(test, unix, not(target_os = "ios")))]
mod x11_events_tests;

#[derive(Debug, Error)]
pub enum TabletError {
    /// The system cannot give the pen's pressure (no X server, or one without XInput2 2.1; an
    /// event monitor AppKit would not install).
    #[error("pen pressure is not available: {0}")]
    Unavailable(String),
}
