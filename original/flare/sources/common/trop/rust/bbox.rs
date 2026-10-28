//! Non-transparent bounding box of a raster, ported from `bbox.cpp`.
//!
//! Returns the tight bounding box of the non-transparent pixels, or an empty
//! rect when the raster is entirely transparent.
//!
//! Details preserved from the C++:
//!
//! * The empty rect is `TRectT<int>()` = `(0, 0, -1, -1)`, matching
//!   `isEmpty()`'s `x0 > x1 || y0 > y1`. A full raster's bounds are
//!   `(0, 0, lx-1, ly-1)`.
//! * There is a fast path: if the top-left *and* bottom-right pixels are both
//!   opaque, the bounds are already tight and the scan is skipped. This is
//!   checked before anything else.
//! * The x scan is a **running** bound, not a per-row minimum. `bbox.x0`
//!   starts at `lx` and the inner loop stops as soon as `x == bbox.x0`, so
//!   later rows are only scanned up to the bound found so far; likewise
//!   `bbox.x1` starts at `-1`, which makes the descending loop start from the
//!   far right on the first row. Folding these into a plain per-row min/max
//!   changes nothing here, but the asymmetry (x0 seeded high, x1 seeded
//!   low) is load-bearing and is kept.
//! * `TPixelCM32` transparency is `!isPureInk() && (getPaint() == 0 &&
//!   isPurePaint())`, over a single `TUINT32`: tone in bits 0-7, paint in
//!   8-19, ink in 20-31, with `getMaxTone() == 255`.

/// The C++ `TRect`, returned as four ints.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
}

impl Rect {
    /// `TRectT<int>()`, the empty rect.
    pub const EMPTY: Rect = Rect { x0: 0, y0: 0, x1: -1, y1: -1 };
    /// `TRasterT::getBounds()`.
    pub fn bounds(lx: i32, ly: i32) -> Rect {
        Rect { x0: 0, y0: 0, x1: lx - 1, y1: ly - 1 }
    }
    /// `TRectT<int>::isEmpty()`.
    pub fn is_empty(&self) -> bool {
        self.x0 > self.x1 || self.y0 > self.y1
    }
}

/// `TPixelCM32::isTransparent`.
#[inline]
fn cm32_is_transparent(v: u32) -> bool {
    let tone = v & 0xFF;
    let paint = (v >> 8) & 0xFFF;
    let is_pure_ink = tone == 0;
    let is_pure_paint = tone == 255; // getMaxTone()
    !is_pure_ink && paint == 0 && is_pure_paint
}

/// Shared body. `transparent(row, x)` reports whether pixel `x` of `row` is
/// transparent; `row_ptr` yields the first byte of row `y`.
fn compute(
    lx: i32,
    ly: i32,
    mut row_ptr: impl FnMut(usize) -> *const u8,
    stride: usize,
    transparent: impl Fn(*const u8, i32) -> bool,
) -> Rect {
    let mut bbox = Rect::bounds(lx, ly);
    if lx <= 0 || ly <= 0 {
        return bbox;
    }

    // Fast path: opaque at both opposite corners means the bounds are tight.
    if !transparent(unsafe { row_ptr(0) }, 0)
        && !transparent(unsafe { row_ptr(ly as usize - 1) }, lx - 1)
    {
        return bbox;
    }

    // First row containing anything.
    let mut y = 0usize;
    while y < ly as usize {
        let row = unsafe { row_ptr(y) };
        let mut any = false;
        for x in 0..lx {
            if !transparent(row, x) {
                any = true;
                break;
            }
        }
        if any {
            break;
        }
        y += 1;
    }
    if y == ly as usize {
        return Rect::EMPTY;
    }
    bbox.y0 = y as i32;

    // Last row containing anything, stopping at y0.
    let mut yb = ly as usize - 1;
    while yb > bbox.y0 as usize {
        let row = unsafe { row_ptr(yb) };
        let mut any = false;
        for x in 0..lx {
            if !transparent(row, x) {
                any = true;
                break;
            }
        }
        if any {
            break;
        }
        yb -= 1;
    }
    bbox.y1 = yb as i32;

    // Running x bounds. See the note at the top of the file.
    bbox.x0 = lx;
    bbox.x1 = -1;
    for yy in bbox.y0..=bbox.y1 {
        let row = unsafe { row_ptr(yy as usize) };
        let mut x = 0;
        while x < bbox.x0 && transparent(row, x) {
            x += 1;
        }
        bbox.x0 = x;

        let mut x = lx - 1;
        while x > bbox.x1 && transparent(row, x) {
            x -= 1;
        }
        bbox.x1 = x;
    }
    let _ = stride;
    bbox
}

/// `TPixel32` / `TPixel64`: transparent when the matte channel is zero.
///
/// `m_off` is a *channel* index (see `ORDER.m`), so it has to be scaled by the
/// channel width before it is used as a byte offset.
#[inline]
fn matte_is_zero(raw: *const u8, x: i32, bytes: usize, m_off: usize) -> bool {
    let off = x as usize * bytes + m_off * (bytes / 4);
    if bytes == 8 {
        unsafe { *(raw.add(off) as *const u16) == 0 }
    } else {
        unsafe { *raw.add(off) == 0 }
    }
}

// --- C ABI -------------------------------------------------------------------

#[no_mangle]
pub extern "C" fn flare_trop_bbox_u8(
    ras: *const u8, wrap: i32, lx: i32, ly: i32, out: *mut Rect,
) {
    let b = compute(lx, ly, |y| unsafe { ras.add(y * wrap.max(0) as usize * 4) },
                   wrap.max(0) as usize * 4, |row, x| matte_is_zero(row, x, 4, M_OFF));
    unsafe { *out = b };
}

#[no_mangle]
pub extern "C" fn flare_trop_bbox_u16(
    ras: *const u8, wrap: i32, lx: i32, ly: i32, out: *mut Rect,
) {
    let b = compute(lx, ly, |y| unsafe { ras.add(y * wrap.max(0) as usize * 8) },
                   wrap.max(0) as usize * 8, |row, x| matte_is_zero(row, x, 8, M_OFF));
    unsafe { *out = b };
}

#[no_mangle]
pub extern "C" fn flare_trop_bbox_cm32(
    ras: *const u32, wrap: i32, lx: i32, ly: i32, out: *mut Rect,
) {
    let b = compute(
        lx,
        ly,
        |y| unsafe { ras.add(y * wrap.max(0) as usize) as *const u8 },
        wrap.max(0) as usize * 4,
        |row, x| unsafe {
            cm32_is_transparent(*(row.add(x as usize * 4) as *const u32))
        },
    );
    unsafe { *out = b };
}

/// Byte offset of the matte channel, matching `TNZ_MACHINE_CHANNEL_ORDER_*`
/// in `include/tmachine.h`: BGRM on Windows/Linux/FreeBSD/Haiku, MRGB on macOS.
#[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd",
          target_os = "haiku"))]
const M_OFF: usize = 3;
#[cfg(target_os = "macos")]
const M_OFF: usize = 0;

#[cfg(test)]
mod tests {
    use super::*;

    /// An all-transparent raster (every channel, including matte, zero).
    fn raster(lx: i32, ly: i32, wrap: i32) -> Vec<u8> {
        vec![0u8; wrap as usize * 4 * ly as usize]
    }

    fn set_matte(v: &mut [u8], wrap: i32, x: i32, y: i32, value: u8) {
        let stride = wrap as usize * 4;
        v[y as usize * stride + x as usize * 4 + M_OFF] = value;
    }

    #[test]
    fn empty_rect_matches_cpp_default() {
        assert_eq!(Rect::EMPTY, Rect { x0: 0, y0: 0, x1: -1, y1: -1 });
        assert!(Rect::EMPTY.is_empty());
    }

    #[test]
    fn bounds_match_get_bounds() {
        let b = Rect::bounds(10, 5);
        assert_eq!(b, Rect { x0: 0, y0: 0, x1: 9, y1: 4 });
        assert!(!b.is_empty());
    }

    #[test]
    fn fully_opaque_returns_bounds_via_fast_path() {
        let (lx, ly, wrap) = (4, 3, 6);
        let mut v = raster(lx, ly, wrap);
        for x in 0..lx {
            for y in 0..ly {
                set_matte(&mut v, wrap, x, y, 255);
            }
        }
        let r = compute(lx, ly, |y| unsafe { v.as_ptr().add(y * wrap as usize * 4) },
                        wrap as usize * 4,
                        |row, x| matte_is_zero(row, x, 4, M_OFF));
        assert_eq!(r, Rect::bounds(lx, ly));
    }

    #[test]
    fn fully_transparent_returns_empty() {
        let (lx, ly, wrap) = (4, 3, 4);
        let v = raster(lx, ly, wrap);
        let r = compute(lx, ly, |y| unsafe { v.as_ptr().add(y * wrap as usize * 4) },
                        wrap as usize * 4,
                        |row, x| matte_is_zero(row, x, 4, M_OFF));
        assert_eq!(r, Rect::EMPTY);
    }

    #[test]
    fn single_opaque_pixel_gives_tight_box() {
        let (lx, ly, wrap) = (5, 4, 5);
        let mut v = raster(lx, ly, wrap);
        set_matte(&mut v, wrap, 2, 1, 200);
        let r = compute(lx, ly, |y| unsafe { v.as_ptr().add(y * wrap as usize * 4) },
                        wrap as usize * 4,
                        |row, x| matte_is_zero(row, x, 4, M_OFF));
        assert_eq!(r, Rect { x0: 2, y0: 1, x1: 2, y1: 1 });
    }

    #[test]
    fn corners_opaque_but_middle_gap_is_measured() {
        // Corners opaque -> fast path does NOT apply because we only check two
        // corners; here top-left and bottom-right are opaque but the true
        // content is still full, so the answer must still be the bounds.
        let (lx, ly, wrap) = (4, 4, 4);
        let mut v = raster(lx, ly, wrap);
        set_matte(&mut v, wrap, 0, 0, 1);
        set_matte(&mut v, wrap, 3, 3, 1);
        set_matte(&mut v, wrap, 1, 1, 1);
        set_matte(&mut v, wrap, 2, 2, 1);
        let r = compute(lx, ly, |y| unsafe { v.as_ptr().add(y * wrap as usize * 4) },
                        wrap as usize * 4,
                        |row, x| matte_is_zero(row, x, 4, M_OFF));
        assert_eq!(r, Rect { x0: 0, y0: 0, x1: 3, y1: 3 });
    }

    #[test]
    fn x_bounds_are_a_running_bound_across_rows() {
        let (lx, ly, wrap) = (6, 3, 6);
        let mut v = raster(lx, ly, wrap);
        // Row 0: content at x=4 and x=5 -> x0=4, x1=5
        set_matte(&mut v, wrap, 4, 0, 9);
        set_matte(&mut v, wrap, 5, 0, 9);
        // Row 1: content only at x=5 (further right) -> must not move x0
        set_matte(&mut v, wrap, 5, 1, 9);
        // Row 2: content at x=5 -> x1 stays 5
        set_matte(&mut v, wrap, 5, 2, 9);
        let r = compute(lx, ly, |y| unsafe { v.as_ptr().add(y * wrap as usize * 4) },
                        wrap as usize * 4,
                        |row, x| matte_is_zero(row, x, 4, M_OFF));
        assert_eq!(r, Rect { x0: 4, y0: 0, x1: 5, y1: 2 });
    }

    #[test]
    fn later_row_can_widen_x1_but_never_narrow_it() {
        let (lx, ly, wrap) = (5, 2, 5);
        let mut v = raster(lx, ly, wrap);
        set_matte(&mut v, wrap, 1, 0, 7);
        set_matte(&mut v, wrap, 3, 1, 7);
        let r = compute(lx, ly, |y| unsafe { v.as_ptr().add(y * wrap as usize * 4) },
                        wrap as usize * 4,
                        |row, x| matte_is_zero(row, x, 4, M_OFF));
        assert_eq!(r, Rect { x0: 1, y0: 0, x1: 3, y1: 1 });
    }

    #[test]
    fn zero_sized_raster_is_empty() {
        let r = compute(0, 4, |_| std::ptr::null(), 0, |_, _| true);
        assert!(r.is_empty());
        let r = compute(4, 0, |_| std::ptr::null(), 0, |_, _| true);
        assert!(r.is_empty());
    }

    #[test]
    fn cm32_transparency_matches_bit_fields() {
        // tone 0 -> isPureInk -> never transparent
        assert!(!cm32_is_transparent(0x0000_0000));
        // tone 255, paint 0 -> transparent
        let v = 255u32 | (0 << 8) | (3 << 20);
        assert!(cm32_is_transparent(v));
        // tone 255, paint != 0 -> opaque
        let v = 255u32 | (5 << 8) | (3 << 20);
        assert!(!cm32_is_transparent(v));
        // tone mid, paint 0 -> not pure ink and not pure paint -> opaque
        let v = 128u32 | (0 << 8) | (3 << 20);
        assert!(!cm32_is_transparent(v));
        // tone mid, paint 0, ink 0: still isPureInk false, isPurePaint false -> opaque
        let v = 64u32;
        assert!(!cm32_is_transparent(v));
    }

    #[test]
    fn u16_matte_zero_detection() {
        let (lx, ly, wrap) = (3, 2, 3);
        let stride = wrap as usize * 8;
        let mut v = vec![0u8; stride * ly as usize];
        // One pixel with non-zero matte in row 1, x=2.
        v[1 * stride + 2 * 8 + M_OFF * 2..1 * stride + 2 * 8 + M_OFF * 2 + 2]
            .copy_from_slice(&1u16.to_ne_bytes());
        let r = compute(lx, ly, |y| unsafe { v.as_ptr().add(y * stride) }, stride,
                        |row, x| matte_is_zero(row, x, 8, M_OFF));
        assert_eq!(r, Rect { x0: 2, y0: 1, x1: 2, y1: 1 });
    }
}