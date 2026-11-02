//! Whether a pen is plugged in, from the kernel's list of input devices. It is asked before the
//! window exists, to choose the window system, so it cannot ask the X server (there may not be
//! one running yet under Wayland) and needs no permission: the list is world readable.

/// `BTN_TOOL_PEN` (linux/input-event-codes.h): the key the stylus of a tablet, of a pen display
/// or of a pen-enabled touchscreen reports.
const BTN_TOOL_PEN: u32 = 0x140;
/// `ABS_X` and `ABS_Y`: the absolute axes every pen reports.
const ABS_X: u32 = 0x00;
const ABS_Y: u32 = 0x01;

/// Whether a pen is among the devices `/proc/bus/input/devices` lists in `text`. A pen has
/// `BTN_TOOL_PEN` in its `B: KEY=` bitmap and `ABS_X` and `ABS_Y` in its `B: ABS=` bitmap. The
/// key alone is not enough: a virtual keyboard (a remote desktop's, for one) claims every key
/// code and has no absolute axes, and taking it for a pen would move the window to XWayland for
/// nothing.
pub(crate) fn has_pen(text: &str) -> bool {
    text.split("\n\n").any(|device| {
        let bitmap = |name: &str| device.lines().find_map(|line| line.strip_prefix(name));
        bitmap("B: KEY=").is_some_and(|keys| bit_set(keys, BTN_TOOL_PEN))
            && bitmap("B: ABS=").is_some_and(|axes| bit_set(axes, ABS_X) && bit_set(axes, ABS_Y))
    })
}

/// Bit `bit` of a bitmap in that file: hexadecimal words of the kernel's `long`, the most
/// significant first. A short or malformed bitmap has no bits set.
fn bit_set(bitmap: &str, bit: u32) -> bool {
    let word = usize::try_from(bit / usize::BITS).unwrap_or(usize::MAX);
    bitmap
        .split_whitespace()
        .rev()
        .nth(word)
        .and_then(|w| u64::from_str_radix(w, 16).ok())
        .is_some_and(|w| w >> (bit % usize::BITS) & 1 == 1)
}

/// Whether a pen is plugged in now. No list (not Linux, or no permission) is no pen.
pub fn pen_attached() -> bool {
    std::fs::read_to_string("/proc/bus/input/devices").is_ok_and(|text| has_pen(&text))
}
