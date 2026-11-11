//! The macOS pen monitor, end to end without a pen or a window: a real AppKit event, made the
//! way a tablet driver makes one, is put on this process's own event queue and taken off it the
//! way an application's event loop does, and the monitor must have fed its pressure.
//!
//! AppKit only works on the main thread, which the test harness does not give its tests, so
//! this file is its own `main` (`harness = false`).

// A test: a failed `expect` is how it fails.
#[cfg(target_os = "macos")]
#[allow(clippy::expect_used)]
mod macos {
    use flare_input::PenFeed;
    use objc2_app_kit::{NSApplication, NSEvent, NSEventMask};
    use objc2_core_foundation::CGPoint;
    use objc2_core_graphics::{CGEvent, CGEventField, CGEventType, CGMouseButton};
    use objc2_foundation::{MainThreadMarker, NSDate, NSDefaultRunLoopMode};

    fn dragged(subtype: i64, pressure: f64, seconds: u64) -> objc2::rc::Retained<NSEvent> {
        let at = CGPoint { x: 40.0, y: 40.0 };
        let event =
            CGEvent::new_mouse_event(None, CGEventType::LeftMouseDragged, at, CGMouseButton::Left)
                .expect("an event");
        CGEvent::set_integer_value_field(Some(&event), CGEventField::MouseEventSubtype, subtype);
        CGEvent::set_double_value_field(Some(&event), CGEventField::MouseEventPressure, pressure);
        // The window server's clock counts nanoseconds.
        CGEvent::set_timestamp(Some(&event), seconds * 1_000_000_000);
        NSEvent::eventWithCGEvent(&event).expect("an AppKit event")
    }

    /// Puts `event` through the application the way its event loop does.
    fn deliver(app: &NSApplication, event: &NSEvent) {
        app.postEvent_atStart(event, true);
        let soon = NSDate::dateWithTimeIntervalSinceNow(1.0);
        // SAFETY: the run loop mode is a constant AppKit owns for the life of the process.
        let mode = unsafe { NSDefaultRunLoopMode };
        let next = app.nextEventMatchingMask_untilDate_inMode_dequeue(
            NSEventMask::Any,
            Some(&soon),
            mode,
            true,
        );
        let next = next.expect("the event comes back off the queue");
        app.sendEvent(&next);
    }

    pub fn run() {
        let mtm = MainThreadMarker::new().expect("the main thread");
        let app = NSApplication::sharedApplication(mtm);
        let feed = PenFeed::default();
        flare_tablet::appkit::watch(feed.clone()).expect("the monitor installs");
        assert_eq!(feed.reading(), None, "no pen until an event says so");

        deliver(&app, &dragged(1, 0.4, 100));
        let pressure = feed.reading().expect("the monitor saw the pen's event");
        assert!((pressure - 0.4).abs() < 0.005, "{pressure}");

        // A plain mouse event in the middle of the pen's own is the pen's motion.
        deliver(&app, &dragged(0, 1.0, 100));
        assert_eq!(feed.reading(), Some(pressure), "the pen is still drawing");

        deliver(&app, &dragged(0, 1.0, 101));
        assert_eq!(feed.reading(), None, "a mouse takes the pen away");
        println!("appkit monitor: ok");
    }
}

fn main() {
    #[cfg(target_os = "macos")]
    macos::run();
}
