#![allow(clippy::expect_used)]

use std::time::{Duration, Instant};

use flare_input::PenFeed;
use x11rb::protocol::xinput::{
    ButtonPressEvent, DeviceClass, DeviceClassData, DeviceClassDataValuator, DeviceId, DeviceType,
    Fp3232, ValuatorMode, XIDeviceInfo,
};
use x11rb::protocol::xproto::Atom;

use crate::tablet_events::{Tracker, axis_value, pens};

const PRESSURE: Atom = 77;

fn fp(whole: i32) -> Fp3232 {
    Fp3232 {
        integral: whole,
        frac: 0,
    }
}

fn valuator(number: u16, label: Atom, min: i32, max: i32) -> DeviceClass {
    DeviceClass {
        len: 0,
        sourceid: 0,
        data: DeviceClassData::Valuator(DeviceClassDataValuator {
            number,
            label,
            min: fp(min),
            max: fp(max),
            value: fp(0),
            resolution: 0,
            mode: ValuatorMode::ABSOLUTE,
        }),
    }
}

fn device(id: DeviceId, type_: DeviceType, classes: Vec<DeviceClass>) -> XIDeviceInfo {
    XIDeviceInfo {
        deviceid: id,
        type_,
        attachment: 2,
        enabled: true,
        name: b"pen".to_vec(),
        classes,
    }
}

/// A tablet: x and y are valuators 0 and 1, pressure is valuator 2.
fn tablet(id: DeviceId) -> XIDeviceInfo {
    device(
        id,
        DeviceType::SLAVE_POINTER,
        vec![
            valuator(0, 1, 0, 30000),
            valuator(1, 2, 0, 20000),
            valuator(2, PRESSURE, 0, 2000),
        ],
    )
}

fn event(device: DeviceId, detail: u32, mask: u32, values: &[i32]) -> ButtonPressEvent {
    ButtonPressEvent {
        deviceid: device,
        detail,
        valuator_mask: vec![mask],
        axisvalues: values.iter().map(|v| fp(*v)).collect(),
        ..Default::default()
    }
}

#[test]
fn only_an_enabled_pointer_with_a_pressure_axis_is_a_pen() {
    let mouse = device(
        9,
        DeviceType::SLAVE_POINTER,
        vec![valuator(0, 1, 0, 100), valuator(1, 2, 0, 100)],
    );
    let master = device(
        2,
        DeviceType::MASTER_POINTER,
        vec![valuator(2, PRESSURE, 0, 2000)],
    );
    let mut off = tablet(7);
    off.enabled = false;
    let no_range = device(
        8,
        DeviceType::SLAVE_POINTER,
        vec![valuator(2, PRESSURE, 0, 0)],
    );
    let found = pens(&[mouse, master, off, no_range, tablet(6)], PRESSURE);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].device, 6);
}

#[test]
fn a_value_is_found_by_counting_the_axes_the_event_carries_before_it() {
    // Axes 0, 2 and 33 are present, so axis 33 is the third value and lives in the second word.
    let mask = [0b101, 0b10];
    let values = [fp(10), fp(20), fp(30)];
    assert_eq!(axis_value(&mask, &values, 0), Some(10.0));
    assert_eq!(axis_value(&mask, &values, 2), Some(20.0));
    assert_eq!(axis_value(&mask, &values, 33), Some(30.0));
    assert_eq!(axis_value(&mask, &values, 1), None, "not carried");
    assert_eq!(axis_value(&mask, &values, 200), None, "past the mask");
    assert_eq!(
        axis_value(&mask, &values[..2], 33),
        None,
        "values cut short"
    );
}

#[test]
fn fractions_of_a_fixed_point_value_count() {
    let half = Fp3232 {
        integral: 4,
        frac: 1 << 31,
    };
    assert_eq!(axis_value(&[1], &[half], 0), Some(4.5));
}

#[test]
fn pressing_the_tip_and_pressing_harder_reach_the_feed_as_a_fraction_of_the_range() {
    let pens = pens(&[tablet(6)], PRESSURE);
    let (feed, mut tracker, now) = (PenFeed::default(), Tracker::default(), Instant::now());
    // Axes 0, 1 and 2 present; pressure 1000 of 2000.
    tracker.button(
        &event(6, 1, 0b111, &[100, 100, 1000]),
        true,
        &pens,
        &feed,
        now,
    );
    assert_eq!(feed.reading(now), Some(0.5));
    tracker.moved(&event(6, 0, 0b111, &[110, 100, 2000]), &pens, &feed, now);
    assert_eq!(
        feed.reading(now + Duration::from_secs(5)),
        Some(1.0),
        "down holds"
    );
}

#[test]
fn a_move_without_pressure_in_it_keeps_the_last_pressure() {
    let pens = pens(&[tablet(6)], PRESSURE);
    let (feed, mut tracker, now) = (PenFeed::default(), Tracker::default(), Instant::now());
    tracker.button(&event(6, 1, 0b111, &[0, 0, 500]), true, &pens, &feed, now);
    tracker.moved(&event(6, 0, 0b011, &[5, 5]), &pens, &feed, now);
    assert_eq!(feed.reading(now), Some(0.25));
}

#[test]
fn hovering_reads_as_the_pen_and_releasing_the_tip_hands_back_to_the_mouse() {
    let pens = pens(&[tablet(6)], PRESSURE);
    let (feed, mut tracker, now) = (PenFeed::default(), Tracker::default(), Instant::now());
    tracker.moved(&event(6, 0, 0b111, &[1, 1, 0]), &pens, &feed, now);
    assert_eq!(feed.reading(now), Some(0.0), "in range, no pressure");
    assert_eq!(
        feed.reading(now + Duration::from_secs(1)),
        None,
        "gone quiet"
    );
    tracker.button(&event(6, 1, 0b111, &[1, 1, 900]), true, &pens, &feed, now);
    tracker.button(&event(6, 1, 0b111, &[1, 1, 0]), false, &pens, &feed, now);
    assert_eq!(feed.reading(now), None);
}

#[test]
fn other_buttons_and_other_devices_change_nothing() {
    let pens = pens(&[tablet(6)], PRESSURE);
    let (feed, mut tracker, now) = (PenFeed::default(), Tracker::default(), Instant::now());
    tracker.button(&event(6, 2, 0b111, &[0, 0, 900]), true, &pens, &feed, now);
    tracker.button(&event(11, 1, 0b111, &[0, 0, 900]), true, &pens, &feed, now);
    tracker.moved(&event(11, 0, 0b111, &[0, 0, 900]), &pens, &feed, now);
    assert_eq!(
        feed.reading(now),
        None,
        "a barrel button or a mouse is not a stroke"
    );
}
