#![allow(clippy::expect_used)]

use x11rb::protocol::xinput::{
    DeviceClass, DeviceClassData, DeviceClassDataValuator, DeviceId, DeviceType, Fp3232,
    ValuatorMode, XIDeviceInfo,
};
use x11rb::protocol::xproto::Atom;

use crate::tablet_events::{MAX_DEVICES, State, Update, axis_value, devices};

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

fn mouse(id: DeviceId) -> XIDeviceInfo {
    device(
        id,
        DeviceType::SLAVE_POINTER,
        vec![valuator(0, 3, 0, 0), valuator(1, 4, 0, 0)],
    )
}

fn state(infos: &[XIDeviceInfo]) -> State {
    let mut state = State::default();
    state.set_devices(devices(infos, PRESSURE));
    state
}

#[test]
fn a_pen_is_a_pointer_with_a_pressure_axis_and_a_mouse_is_a_pointer_without() {
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
    let state = state(&[mouse(9), master, off, no_range, tablet(6)]);
    assert_eq!(
        state.pens(),
        vec!["pen".to_string()],
        "only device 6 senses pressure"
    );
    assert!(state.has_pen());
    assert!(!State::default().has_pen());
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
fn pressure_is_a_fraction_of_the_devices_own_range_and_is_kept_when_other_axes_move() {
    let mut state = state(&[tablet(6)]);
    // Axes 0, 1 and 2 present; pressure 1000 of 2000.
    assert_eq!(
        state.handle(6, &[0b111], &[fp(1), fp(1), fp(1000)]),
        Update::Pen(0.5)
    );
    // An event that moved x and y only: the pen is still at its last pressure.
    assert_eq!(state.handle(6, &[0b011], &[fp(5), fp(5)]), Update::Pen(0.5));
    assert_eq!(
        state.handle(6, &[0b111], &[fp(5), fp(5), fp(2000)]),
        Update::Pen(1.0)
    );
}

#[test]
fn a_pen_that_has_not_pressed_yet_is_hovering_at_no_pressure() {
    let mut state = state(&[tablet(6)]);
    assert_eq!(state.handle(6, &[0b011], &[fp(1), fp(1)]), Update::Pen(0.0));
}

#[test]
fn another_pointer_moving_means_the_pen_is_not_the_device_in_use() {
    let mut state = state(&[tablet(6), mouse(9)]);
    assert_eq!(state.handle(6, &[0b100], &[fp(2000)]), Update::Pen(1.0));
    assert_eq!(state.handle(9, &[0b011], &[fp(3), fp(3)]), Update::Mouse);
}

#[test]
fn a_master_or_unknown_device_changes_nothing() {
    let mut state = state(&[tablet(6)]);
    assert_eq!(state.handle(2, &[0b100], &[fp(2000)]), Update::Keep);
    assert_eq!(state.handle(99, &[0b100], &[fp(2000)]), Update::Keep);
}

#[test]
fn a_pen_that_goes_away_forgets_its_pressure_and_one_that_comes_back_starts_over() {
    let mut state = state(&[tablet(6)]);
    state.handle(6, &[0b100], &[fp(2000)]);
    state.set_devices(devices(&[mouse(9)], PRESSURE));
    assert_eq!(
        state.handle(6, &[0b100], &[fp(2000)]),
        Update::Keep,
        "no longer a device"
    );
    state.set_devices(devices(&[tablet(6)], PRESSURE));
    assert_eq!(state.handle(6, &[0b011], &[fp(1), fp(1)]), Update::Pen(0.0));
}

#[test]
fn hostile_values_neither_panic_nor_leave_the_range() {
    let mut state = state(&[tablet(6)]);
    for value in [i32::MIN, -1, 0, i32::MAX] {
        let update = state.handle(6, &[u32::MAX; 40], &vec![fp(value); 2000]);
        assert!(
            matches!(update, Update::Pen(p) if (0.0..=1.0).contains(&p)),
            "{value}"
        );
    }
    let huge = Fp3232 {
        integral: i32::MAX,
        frac: u32::MAX,
    };
    assert!(matches!(
        state.handle(6, &[0b100], &[huge]),
        Update::Pen(1.0)
    ));
    assert_eq!(
        state.handle(6, &[], &[]),
        Update::Pen(1.0),
        "an empty event keeps the last"
    );
}

#[test]
fn a_server_with_thousands_of_devices_is_cut_short() {
    let many: Vec<XIDeviceInfo> = (0..5000u16).map(mouse).collect();
    assert_eq!(devices(&many, PRESSURE).len(), MAX_DEVICES);
}
