#![allow(clippy::expect_used)]
//! Real AppKit events, made the way a tablet driver makes them (a mouse event of the window
//! server with the tablet's fields set), read the way the monitor reads them. No window, no
//! application and no pen is needed for this; the monitor being called at all is not covered.

use objc2_app_kit::NSEvent;
use objc2_core_foundation::CGPoint;
use objc2_core_graphics::{CGEvent, CGEventField, CGEventType, CGMouseButton};

use crate::appkit::read;
use crate::appkit_events::{Tracker, Update};

/// `kCGEventMouseSubtypeTabletPoint` and `kCGEventMouseSubtypeTabletProximity`.
const TABLET_POINT: i64 = 1;
const TABLET_PROXIMITY: i64 = 2;

/// What the monitor makes of a mouse event of `kind` with the tablet `subtype` and `pressure`.
fn reading(kind: CGEventType, subtype: i64, pressure: f64) -> Update {
    let at = CGPoint { x: 40.0, y: 40.0 };
    let event = CGEvent::new_mouse_event(None, kind, at, CGMouseButton::Left).expect("an event");
    CGEvent::set_integer_value_field(Some(&event), CGEventField::MouseEventSubtype, subtype);
    CGEvent::set_double_value_field(Some(&event), CGEventField::MouseEventPressure, pressure);
    let event = NSEvent::eventWithCGEvent(&event).expect("an AppKit event");
    read(&event, &mut Tracker::default())
}

fn pen(update: Update) -> f32 {
    match update {
        Update::Pen(pressure) => pressure,
        other => panic!("a pen's pressure expected: {other:?}"),
    }
}

#[test]
fn a_pens_drag_press_and_lift_give_the_pressure_the_driver_set() {
    for (kind, pressure) in [
        (CGEventType::LeftMouseDown, 0.25),
        (CGEventType::LeftMouseDragged, 0.4),
        (CGEventType::LeftMouseDragged, 1.0),
        (CGEventType::LeftMouseUp, 0.0),
        // Hovering over the tablet.
        (CGEventType::MouseMoved, 0.0),
    ] {
        let read = pen(reading(kind, TABLET_POINT, pressure));
        // The window server keeps a mouse event's pressure in 255 steps.
        assert!(
            (f64::from(read) - pressure).abs() < 0.005,
            "{kind:?}: {read}"
        );
    }
}

#[test]
fn a_mouse_is_not_a_pen_even_with_its_button_down() {
    // A mouse's button reads as pressure 1 in AppKit; it must not be taken for a pen pressing.
    assert_eq!(
        reading(CGEventType::LeftMouseDragged, 0, 1.0),
        Update::Mouse
    );
    assert_eq!(reading(CGEventType::MouseMoved, 0, 0.0), Update::Mouse);
}

#[test]
fn a_pen_leaving_the_tablet_is_read_without_asking_a_mouse_event_for_what_it_lacks() {
    // Not entering (the field is unset), so the pen has left.
    assert_eq!(
        reading(CGEventType::MouseMoved, TABLET_PROXIMITY, 0.0),
        Update::Mouse
    );
}
