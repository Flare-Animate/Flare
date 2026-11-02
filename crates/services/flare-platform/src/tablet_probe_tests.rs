use crate::tablet_probe::has_pen;

/// A device as the kernel lists it: the bitmaps are hexadecimal words of a 64-bit `long`, so
/// bit 0x140 is bit 0 of word 5, and ABS_X and ABS_Y are bits 0 and 1 of word 0.
fn device(name: &str, key: &str, abs: &str) -> String {
    format!(
        "I: Bus=0003 Vendor=0001 Product=0001 Version=0111\nN: Name=\"{name}\"\nB: PROP=0\nB: EV=b\nB: KEY={key}\nB: ABS={abs}\n"
    )
}

// Word 5 holds BTN_TOOL_PEN (bit 0x140 is 320, so word 5 bit 0).
const PEN_KEYS: &str = "1 0 0 0 0 0";
const ALL_KEYS: &str = "ffffffffffffffff ffffffffffffffff ffffffffffffffff ffffffffffffffff ffffffffffffffff ffffffffffffffff 0";
const XY_AND_PRESSURE: &str = "1000003";

#[test]
fn a_device_with_the_pen_key_and_both_absolute_axes_is_a_pen() {
    assert!(has_pen(&device("Tablet Pen", PEN_KEYS, XY_AND_PRESSURE)));
}

#[test]
fn a_virtual_keyboard_that_claims_every_key_is_not_a_pen() {
    assert!(!has_pen(&device("Virtual keyboard", ALL_KEYS, "0")));
}

#[test]
fn the_pen_key_without_both_axes_is_not_a_pen() {
    assert!(!has_pen(&device("Odd", PEN_KEYS, "1")), "only ABS_X");
    assert!(!has_pen(&device("Odd", PEN_KEYS, "")));
}

#[test]
fn a_pen_among_other_devices_is_found_and_none_is_none() {
    let text = format!(
        "{}\n{}",
        device("Mouse", "1f0000 0 0 0 0 0", "0"),
        device("Tablet Pen", PEN_KEYS, XY_AND_PRESSURE)
    );
    assert!(has_pen(&text));
    assert!(!has_pen(""));
    assert!(
        !has_pen("garbage\n\nB: KEY=zz\nB: ABS=zz\n"),
        "malformed bitmaps have no bits"
    );
}
