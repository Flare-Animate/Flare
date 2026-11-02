//! Reading a pen's pressure out of the X server's XInput2 raw events: which devices have a
//! pressure axis, where in an event's packed values that axis is, and whether the pen or a
//! mouse moved last. Nothing here talks to the server, so all of it is tested without one.
//!
//! Raw events are used, selected on the root window, because winit holds the pointer's implicit
//! grab for the whole of a drag and events selected on its window would not reach a second
//! connection then; raw events go to every client that asks, grabbed or not. They do not say
//! which window the pointer is over, which is fine: the stage reads the pressure only while its
//! own canvas is being dragged.

use std::collections::HashMap;

use flare_input::PressureAxis;
use x11rb::protocol::xinput::{DeviceClassData, DeviceId, DeviceType, Fp3232, XIDeviceInfo};
use x11rb::protocol::xproto::Atom;

/// Devices kept: a server could report thousands, and only a handful are ever pens.
pub(crate) const MAX_DEVICES: usize = 256;

/// A pointer device the server has, and whether it can sense pressure.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Device {
    id: DeviceId,
    /// The name the server gives the device, for telling a person what was found.
    name: String,
    /// Which valuator of the device's events carries pressure, and over what range.
    pressure: Option<(u16, PressureAxis)>,
}

/// The pointer devices among `infos` (a pen is one too, with a pressure axis). `label` is the
/// server's atom for "Abs Pressure", which is how every driver names that axis.
pub(crate) fn devices(infos: &[XIDeviceInfo], label: Atom) -> Vec<Device> {
    infos
        .iter()
        .filter(|d| d.enabled)
        .filter(|d| d.type_ == DeviceType::SLAVE_POINTER || d.type_ == DeviceType::FLOATING_SLAVE)
        .take(MAX_DEVICES)
        .map(|d| Device {
            id: d.deviceid,
            name: String::from_utf8_lossy(&d.name).into_owned(),
            pressure: d.classes.iter().find_map(|class| match &class.data {
                DeviceClassData::Valuator(v) if v.label == label => {
                    PressureAxis::new(fixed(&v.min), fixed(&v.max)).map(|range| (v.number, range))
                }
                _ => None,
            }),
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

/// What an event means for who is drawing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Update {
    /// Not from a device this knows (a master pointer, or one added since the last query).
    Keep,
    /// A pen is the device in use, with this pressure.
    Pen(f32),
    /// Some other pointer moved, so the pen is not the device in use.
    Mouse,
}

/// The devices, and the last pressure of each pen.
#[derive(Debug, Default)]
pub(crate) struct State {
    devices: HashMap<DeviceId, Device>,
    last: HashMap<DeviceId, f32>,
}

impl State {
    /// Replaces the device list (at the start, and when the server says devices changed).
    pub(crate) fn set_devices(&mut self, devices: Vec<Device>) {
        self.devices = devices
            .into_iter()
            .take(MAX_DEVICES)
            .map(|d| (d.id, d))
            .collect();
        self.last.retain(|id, _| self.devices.contains_key(id));
    }

    /// The names of the pens, for telling a person what was found.
    pub(crate) fn pens(&self) -> Vec<String> {
        let mut found: Vec<&Device> = self
            .devices
            .values()
            .filter(|d| d.pressure.is_some())
            .collect();
        found.sort_by_key(|d| d.id);
        found.into_iter().map(|d| d.name.clone()).collect()
    }

    /// Whether any device is a pen, which is when the raw events are worth listening to.
    pub(crate) fn has_pen(&self) -> bool {
        self.devices.values().any(|d| d.pressure.is_some())
    }

    /// A raw motion or button event from device `id`, with the values it carries. A pen keeps
    /// its last pressure when the event moved other axes only (a hovering pen has pressure 0).
    pub(crate) fn handle(&mut self, id: DeviceId, mask: &[u32], values: &[Fp3232]) -> Update {
        let Some(device) = self.devices.get(&id) else {
            return Update::Keep;
        };
        let Some((axis, range)) = device.pressure else {
            return Update::Mouse;
        };
        let last = self.last.entry(id).or_insert(0.0);
        if let Some(raw) = axis_value(mask, values, axis) {
            *last = range.normalize(raw);
        }
        Update::Pen(*last)
    }
}
