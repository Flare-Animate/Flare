//! The Dock icon through AppKit. AppKit is reached through Objective-C, which is why this module
//! alone in the crate may use `unsafe`.
#![allow(unsafe_code)]

use objc2::{AllocAnyThread, MainThreadMarker};
use objc2_app_kit::{NSApplication, NSImage};
use objc2_foundation::NSData;

/// Sets the application's Dock icon to `png`. Off the main thread, or with a picture AppKit
/// cannot read, nothing changes.
pub(crate) fn set_dock_icon(png: &[u8]) {
    let Some(main_thread) = MainThreadMarker::new() else {
        return;
    };
    let data = NSData::with_bytes(png);
    let Some(image) = NSImage::initWithData(NSImage::alloc(), &data) else {
        return;
    };
    let app = NSApplication::sharedApplication(main_thread);
    // SAFETY: on the main thread (checked above), with a valid image; the setter is marked unsafe
    // only because a `None` argument may not be accepted, and `Some` is passed.
    unsafe { app.setApplicationIconImage(Some(&image)) };
}
