//! What an AppKit pointer event says about the pen, as plain numbers, so it is decided and
//! tested apart from the system calls in `appkit`.

/// `NSEventSubtypeTabletPoint`: a mouse event that a tablet pen made.
const SUBTYPE_TABLET_POINT: i16 = 1;
/// `NSEventSubtypeTabletProximity`: a mouse event saying a pen came near or went away.
const SUBTYPE_TABLET_PROXIMITY: i16 = 2;
/// `NSEventTypeTabletPoint` and `NSEventTypeTabletProximity`, the tablet's own events.
const TYPE_TABLET_POINT: usize = 23;
const TYPE_TABLET_PROXIMITY: usize = 24;

/// Whether `event_type` is a button or motion event of the pointer (`NSEventTypeLeftMouseDown`
/// to `NSEventTypeRightMouseDragged` without the enter and exit events between them, and the
/// other buttons).
pub(crate) fn is_mouse(event_type: usize) -> bool {
    matches!(event_type, 1..=7 | 25..=27)
}

/// Which device an event came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Source {
    /// A mouse or trackpad: its `pressure` is the button, or a Force Touch, and not a pen's.
    Mouse,
    /// A pen on or over the tablet; the event's `pressure` is the pen's.
    Pen,
    /// A pen coming near the tablet or leaving it; the event says which.
    Proximity,
}

/// The device behind an event of `event_type` (`NSEvent.type`) and `subtype`
/// (`NSEvent.subtype`), or `None` for an event that is not the pointer's (a key, a scroll).
/// A pen drives the pointer, so most of what it does arrives as mouse events marked by their
/// subtype.
pub(crate) fn source(event_type: usize, subtype: i16) -> Option<Source> {
    match (event_type, subtype) {
        (TYPE_TABLET_POINT, _) => Some(Source::Pen),
        (TYPE_TABLET_PROXIMITY, _) => Some(Source::Proximity),
        (t, SUBTYPE_TABLET_POINT) if is_mouse(t) => Some(Source::Pen),
        (t, SUBTYPE_TABLET_PROXIMITY) if is_mouse(t) => Some(Source::Proximity),
        (t, _) if is_mouse(t) => Some(Source::Mouse),
        _ => None,
    }
}

/// What the feed should hold after an event.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Update {
    /// Nothing new: the event says nothing about pressure.
    Keep,
    /// The pen is the device in use, pressing this hard (as AppKit gives it, 0 to 1).
    Pen(f32),
    /// The pen is not the device in use.
    Mouse,
}

/// How long after a pen's event (in seconds) a plain mouse event is still taken to be the
/// pen's. Some tablet drivers send the pressure in the tablet's own events and the motion in
/// plain mouse events between them; without this each of those would take the pen away and the
/// stroke would be drawn at full width. A pen in use sends events many times within this time,
/// and a hand takes longer than this to move from the pen to a mouse.
pub(crate) const PEN_HOLD: f64 = 0.25;

/// Follows which device is in use from one event to the next.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Tracker {
    /// When the pen last sent an event of its own, in the events' clock; `None` once it left.
    last_pen: Option<f64>,
}

impl Tracker {
    /// The update for an event from `source` at the time `at` (`NSEvent.timestamp`, seconds).
    /// `pressure` is read only for a pen's point, and `entering` only for a proximity event: a
    /// pen that arrives has not pressed yet, and one that leaves is no longer the device in
    /// use. A plain mouse event takes the pen away only when the pen has been quiet for
    /// [`PEN_HOLD`]; a time that is not a number, or runs backwards, counts as quiet.
    pub(crate) fn apply(
        &mut self,
        source: Source,
        pressure: f32,
        entering: bool,
        at: f64,
    ) -> Update {
        match source {
            Source::Pen => {
                self.last_pen = Some(at);
                Update::Pen(pressure)
            }
            Source::Proximity if entering => Update::Keep,
            Source::Mouse
                if self
                    .last_pen
                    .is_some_and(|pen| (0.0..PEN_HOLD).contains(&(at - pen))) =>
            {
                Update::Keep
            }
            Source::Proximity | Source::Mouse => {
                self.last_pen = None;
                Update::Mouse
            }
        }
    }
}
