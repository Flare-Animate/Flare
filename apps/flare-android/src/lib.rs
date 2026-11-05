//! `flare-android`: Flare as an Android app.
//!
//! Not a mobile rewrite of the desktop app: the shell the desktop shows (`flare-desktop`'s
//! `ui`) runs here, on the activity's event loop, drawing the same workspace. This crate holds
//! only what a phone gives in place of a desk: the activity to load into, and the app's own
//! data directory standing in for the paths the platform has no convention for.
//!
//! Testing: the start of the app ([`open`]) is tested here on any machine, and the shell
//! itself is `flare-desktop`'s; what cannot be exercised off a device is `activity`, which
//! needs the loader (see docs/SURFACES.md).

// Denied, not forbidden: the one `unsafe` item is the loader's entry point, which the platform
// requires to be `#[unsafe(no_mangle)]`; `forbid` would refuse that allow.
#![deny(unsafe_code)]

/// What the app does before the window opens; the tests drive it on any machine.
pub mod open;

#[cfg(target_os = "android")]
mod activity;
