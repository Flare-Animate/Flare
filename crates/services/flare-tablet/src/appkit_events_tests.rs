use crate::appkit_events::{PEN_HOLD, Source, Tracker, Update, source};

const LEFT_DOWN: usize = 1;
const LEFT_UP: usize = 2;
const MOVED: usize = 5;
const LEFT_DRAGGED: usize = 6;
const KEY_DOWN: usize = 10;
const SCROLL: usize = 22;
const TABLET_POINT: usize = 23;
const TABLET_PROXIMITY: usize = 24;

#[test]
fn a_plain_mouse_event_is_a_mouse_whatever_the_button() {
    for event in [LEFT_DOWN, LEFT_UP, 3, 4, MOVED, LEFT_DRAGGED, 7, 25, 26, 27] {
        assert_eq!(source(event, 0), Some(Source::Mouse), "type {event}");
    }
    // Subtypes that are not the tablet's (a touch, for one) are still a mouse.
    assert_eq!(source(LEFT_DRAGGED, 3), Some(Source::Mouse));
}

#[test]
fn a_mouse_event_marked_by_the_tablet_is_the_pen() {
    for event in [LEFT_DOWN, LEFT_UP, MOVED, LEFT_DRAGGED] {
        assert_eq!(source(event, 1), Some(Source::Pen), "type {event}");
        assert_eq!(source(event, 2), Some(Source::Proximity), "type {event}");
    }
}

#[test]
fn the_tablets_own_events_are_the_pen_whatever_their_subtype() {
    assert_eq!(source(TABLET_POINT, 0), Some(Source::Pen));
    assert_eq!(source(TABLET_PROXIMITY, 0), Some(Source::Proximity));
}

#[test]
fn events_that_are_not_the_pointers_say_nothing() {
    // A key's subtype field is not a pointer's, even when the number matches the tablet's.
    assert_eq!(source(KEY_DOWN, 1), None);
    assert_eq!(source(SCROLL, 0), None);
    // Entered and exited sit between the mouse types and carry no pressure.
    assert_eq!(source(8, 1), None);
    assert_eq!(source(9, 0), None);
    assert_eq!(source(0, 0), None);
}

#[test]
fn a_pens_point_gives_its_pressure_and_a_mouse_takes_the_pen_away() {
    let mut tracker = Tracker::default();
    assert_eq!(
        tracker.apply(Source::Pen, 0.4, false, 10.0),
        Update::Pen(0.4)
    );
    // Hovering, or the lift at the end of a stroke.
    assert_eq!(
        tracker.apply(Source::Pen, 0.0, false, 10.1),
        Update::Pen(0.0)
    );
    assert_eq!(
        tracker.apply(Source::Mouse, 1.0, false, 12.0),
        Update::Mouse
    );
    // With no pen before it, a mouse is a mouse at once.
    let mut fresh = Tracker::default();
    assert_eq!(fresh.apply(Source::Mouse, 1.0, false, 0.0), Update::Mouse);
}

#[test]
fn a_pen_that_leaves_is_no_longer_in_use_and_one_that_arrives_changes_nothing_yet() {
    let mut tracker = Tracker::default();
    assert_eq!(
        tracker.apply(Source::Proximity, 0.0, true, 1.0),
        Update::Keep
    );
    assert_eq!(
        tracker.apply(Source::Pen, 0.5, false, 1.0),
        Update::Pen(0.5)
    );
    assert_eq!(
        tracker.apply(Source::Proximity, 0.0, false, 1.01),
        Update::Mouse
    );
    // It left, so the mouse event right after is not held back.
    assert_eq!(
        tracker.apply(Source::Mouse, 1.0, false, 1.02),
        Update::Mouse
    );
}

#[test]
fn plain_mouse_events_between_a_pens_own_do_not_take_the_pen_away() {
    // A driver that sends pressure in tablet events and motion in plain mouse events.
    let mut tracker = Tracker::default();
    let mut at = 5.0;
    for pressure in [0.2, 0.5, 0.8] {
        assert_eq!(
            tracker.apply(Source::Pen, pressure, false, at),
            Update::Pen(pressure)
        );
        assert_eq!(
            tracker.apply(Source::Mouse, 1.0, false, at + 0.004),
            Update::Keep
        );
        at += 0.008;
    }
    // The pen went quiet: the mouse has it from then on, and keeps it.
    let later = at + PEN_HOLD;
    assert_eq!(
        tracker.apply(Source::Mouse, 1.0, false, later),
        Update::Mouse
    );
    assert_eq!(
        tracker.apply(Source::Mouse, 1.0, false, later + 0.001),
        Update::Mouse
    );
}

#[test]
fn a_time_that_is_not_a_number_or_runs_backwards_is_not_held() {
    let mut tracker = Tracker::default();
    tracker.apply(Source::Pen, 0.5, false, 5.0);
    assert_eq!(
        tracker.apply(Source::Mouse, 1.0, false, f64::NAN),
        Update::Mouse
    );
    tracker.apply(Source::Pen, 0.5, false, 5.0);
    assert_eq!(tracker.apply(Source::Mouse, 1.0, false, 4.0), Update::Mouse);
}
