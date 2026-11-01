//! A pen whose pressure comes from a native source beside the window's own events.

use super::*;
use flare_input::PointerKind;
use std::time::Instant;

#[test]
fn with_no_pen_the_device_is_a_mouse_at_full_pressure() {
    let ws = Workspace::untitled().expect("workspace");
    assert_eq!(ws.pointer_reading(None), (1.0, PointerKind::Mouse));
}

#[test]
fn a_pen_on_the_surface_reports_its_pressure_over_a_touch_force_and_over_a_mouse() {
    let ws = Workspace::untitled().expect("workspace");
    let feed = ws.pen_feed();
    feed.press(0.3, Instant::now());
    assert_eq!(ws.pointer_reading(None), (0.3, PointerKind::Pen));
    assert_eq!(ws.pointer_reading(Some(0.9)), (0.3, PointerKind::Pen));
    feed.lift();
    assert_eq!(ws.pointer_reading(None), (1.0, PointerKind::Mouse));
    assert_eq!(
        ws.pointer_reading(Some(0.9)),
        (0.9, PointerKind::Pen),
        "the window's own touch force still works where there is one"
    );
}
