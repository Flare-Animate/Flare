//! The window shell as a library: what the `flare-desktop` binary shows, so another front end
//! can run the very same implementation instead of a second one (the Android build runs this
//! shell on the phone's activity). What the operating system provides is `flare-platform`'s.

#![forbid(unsafe_code)]

pub mod ui;
