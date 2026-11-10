//! Pen pressure on macOS. winit gives a tablet pen to the program as a mouse, with no pressure,
//! so the pen is read from AppKit directly: a local event monitor sees each pointer event of
//! this program before the window does, and puts what it says about the pen into a [`PenFeed`]
//! that the stage reads. The events go on to the window unchanged.
//!
//! AppKit is reached through Objective-C, which is why this module alone may use `unsafe`.
#![allow(unsafe_code)]

use std::ptr::NonNull;

use block2::RcBlock;
use flare_input::PenFeed;
use objc2_app_kit::{NSEvent, NSEventMask};

use crate::TabletError;
use crate::appkit_events::{Source, Tracker, Update, is_mouse, source};

/// What `event` says about the pen, given what `tracker` saw before it. `subtype`, `pressure`
/// and `isEnteringProximity` raise an Objective-C exception on an event of the wrong kind, so
/// each is asked only of the kind that has it.
pub(crate) fn read(event: &NSEvent, tracker: &mut Tracker) -> Update {
    let event_type = event.r#type().0;
    let subtype = if is_mouse(event_type) {
        event.subtype().0
    } else {
        0
    };
    let Some(source) = source(event_type, subtype) else {
        return Update::Keep;
    };
    let pressure = if source == Source::Pen {
        event.pressure()
    } else {
        0.0
    };
    let entering = source == Source::Proximity && event.isEnteringProximity();
    let update = tracker.apply(source, pressure, entering, event.timestamp());
    if log() {
        eprintln!("tablet event: type {event_type} subtype {subtype}: {update:?}");
    }
    update
}

/// Whether every pointer event is to be written to the terminal (`FLARE_TABLET=log`), to see
/// what a tablet's driver sends when its pressure does not show.
fn log() -> bool {
    static LOG: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *LOG.get_or_init(|| std::env::var("FLARE_TABLET").is_ok_and(|v| v.eq_ignore_ascii_case("log")))
}

/// Whether a pen's event has been seen yet.
static SEEN: std::sync::Once = std::sync::Once::new();

/// Starts feeding `feed` with the pen's pressure for as long as the process lives. Call it on
/// the main thread once the application exists (a window has been made); AppKit then runs the
/// monitor on the main thread, for this program's own events only.
pub fn watch(feed: PenFeed) -> Result<(), TabletError> {
    let mask = NSEventMask::LeftMouseDown
        | NSEventMask::LeftMouseUp
        | NSEventMask::LeftMouseDragged
        | NSEventMask::MouseMoved
        | NSEventMask::TabletPoint
        | NSEventMask::TabletProximity;
    let tracker = std::cell::Cell::new(Tracker::default());
    let handler = RcBlock::new(move |event: NonNull<NSEvent>| -> *mut NSEvent {
        let mut seen = tracker.get();
        // SAFETY: AppKit hands the monitor a valid event that lives for the whole call, and it
        // is only read here.
        let update = read(unsafe { event.as_ref() }, &mut seen);
        tracker.set(seen);
        match update {
            Update::Keep => {}
            Update::Pen(pressure) => {
                // Said once on the terminal, so a person whose strokes show no pressure can tell
                // whether the tablet's driver marks its events as a pen's at all.
                SEEN.call_once(|| eprintln!("pen pressure from: AppKit tablet events"));
                feed.set(Some(pressure));
            }
            Update::Mouse => feed.set(None),
        }
        // The same event, so the window gets it as if nobody had looked.
        event.as_ptr()
    });
    // SAFETY: the block returns the event it was given, which is the valid pointer AppKit asks
    // for, and captures only the feed, which may be used from any thread.
    let monitor = unsafe { NSEvent::addLocalMonitorForEventsMatchingMask_handler(mask, &handler) };
    match monitor {
        // AppKit keeps the monitor until it is removed, which is never: it ends with the
        // process. Forgetting the handle says so, whatever dropping it would do.
        Some(monitor) => {
            std::mem::forget(monitor);
            Ok(())
        }
        None => Err(TabletError::Unavailable(
            "AppKit did not install the pointer event monitor".into(),
        )),
    }
}
