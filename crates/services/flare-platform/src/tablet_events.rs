//! Reading a pen's pressure out of the X server's XInput2 events: which devices have a pressure
//! axis, where in an event's packed values that axis is, and what a press, a move and a release
//! mean for the shared feed. Nothing here talks to the server, so all of it is tested without one.

use std::time::Instant;

use flare_input::{PenFeed, PressureAxis};
use x11rb::protocol::xinput::{
    ButtonPressEvent, DeviceClassData, DeviceId, DeviceType, Fp3232, XIDeviceInfo,
};
use x11rb::protocol::xproto::Atom;

/// A device that reports pressure, and where.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Pen {
    pub(crate) device: DeviceId,
    /// The name the server gives the device, for telling a person what was found.
    pub(crate) name: String,
    /// Which valuator of the device's events carries pressure.
    axis: u16,
    range: PressureAxis,
}

/// The pointer devices among `devices` that have a pressure axis. `label` is the server's atom
/// for "Abs Pressure", which is how every driver names that axis.
pub(crate) fn pens(devices: &[XIDeviceInfo], label: Atom) -> Vec<Pen> {
    devices
        .iter()
        .filter(|d| d.enabled && d.type_ == DeviceType::SLAVE_POINTER)
        .filter_map(|d| {
            d.classes.iter().find_map(|class| match &class.data {
                DeviceClassData::Valuator(v) if v.label == label => {
                    PressureAxis::new(fixed(&v.min), fixed(&v.max)).map(|range| Pen {
                        device: d.deviceid,
                        name: String::from_utf8_lossy(&d.name).into_owned(),
                        axis: v.number,
                        range,
                    })
                }
                _ => None,
            })
        })
        .collect()
}

/// A 32.32 fixed point number.
fn fixed(value: &Fp3232) -> f64 {
    f64::from(value.integral) + f64::from(value.frac) / 4_294_967_296.0
}

/// Valuator `axis` of an event: the mask says which axes the event carries, and the values hold
/// only those, in order. An event that moved other axes only has no pressure in it.
pub(crate) fn axis_value(mask: &[u32], values: &[Fp3232], axis: u16) -> Option<f64> {
    let (word, bit) = (usize::from(axis / 32), u32::from(axis % 32));
    if mask.get(word)? >> bit & 1 == 0 {
        return None;
    }
    let before: u32 = mask[..word].iter().map(|w| w.count_ones()).sum::<u32>()
        + (mask[word] & ((1u32 << bit) - 1)).count_ones();
    values.get(before as usize).map(fixed)
}

/// What a pen is doing, between the server's events.
#[derive(Debug, Default)]
pub(crate) struct Tracker {
    down: bool,
    /// The last pressure seen: a press whose event does not carry one keeps it.
    last: f32,
}

/// The primary button, which is the pen's tip.
const TIP: u32 = 1;

impl Tracker {
    fn pressure(&mut self, event: &ButtonPressEvent, pens: &[Pen]) {
        let Some(pen) = pens.iter().find(|p| p.device == event.deviceid) else {
            return;
        };
        if let Some(raw) = axis_value(&event.valuator_mask, &event.axisvalues, pen.axis) {
            self.last = pen.range.normalize(raw);
        }
    }

    /// The pen moved; while it is down that is a stroke, else it is only hovering.
    pub(crate) fn moved(
        &mut self,
        event: &ButtonPressEvent,
        pens: &[Pen],
        feed: &PenFeed,
        now: Instant,
    ) {
        if !pens.iter().any(|p| p.device == event.deviceid) {
            return;
        }
        self.pressure(event, pens);
        if self.down {
            feed.press(self.last, now);
        } else {
            feed.hover(self.last, now);
        }
    }

    /// A button went down or up; only the tip changes whether the pen is drawing.
    pub(crate) fn button(
        &mut self,
        event: &ButtonPressEvent,
        pressed: bool,
        pens: &[Pen],
        feed: &PenFeed,
        now: Instant,
    ) {
        if event.detail != TIP || !pens.iter().any(|p| p.device == event.deviceid) {
            return;
        }
        self.pressure(event, pens);
        self.down = pressed;
        if pressed {
            feed.press(self.last, now);
        } else {
            self.last = 0.0;
            feed.lift();
        }
    }
}
