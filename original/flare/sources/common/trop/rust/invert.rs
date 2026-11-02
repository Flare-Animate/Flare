//! Invert operator kernels, ported from `tinvert.cpp`.
//!
//! The C++ operated on named members (`pixIn->r`, `->g`, `->b`, `->m`), which
//! is order-agnostic. This port works on raw bytes, so it must reproduce the
//! machine channel order exactly. `include/tmachine.h` defines:
//!
//! | platform                | macro    | byte layout |
//! |-------------------------|----------|-------------|
//! | Windows, Linux, FreeBSD, Haiku | `TNZ_MACHINE_CHANNEL_ORDER_BGRM` | `[b, g, r, m]` |
//! | macOS                   | `TNZ_MACHINE_CHANNEL_ORDER_MRGB` | `[m, r, g, b]` |
//!
//! `TNZ_LITTLE_ENDIAN` is required unconditionally by tmachine.h, so no
//! byte-swapping is needed.
//!
//! Semantics preserved from the C++:
//!
//! * Colour channels use a **subtractive** invert, `c' = m - c`, not `255 - c`.
//!   Channels are `unsigned char`, so this is computed in `int` and truncated
//!   back on assignment -- i.e. wrapping subtraction mod 256, which is why
//!   `m = 0, r = 200` yields 56 and not -200. Hence `wrapping_sub` below.
//! * Matte uses **bitwise NOT** (`~m`) for the integer pixel types, but
//!   `1.0 - m` for `TPixelF`. These are genuinely different operations.
//! * `TPixelGR8` has a single `value` byte and ignores the channel flags
//!   entirely; it always computes `255 - value`.
//! * `TRop::invert` returns early when no colour channel is selected, so
//!   `invMatte` on its own is a no-op. `unpack_flags` reproduces that.
//! * The fast path (all three channels, no matte) is a branch-per-pixel
//!   choice in the original; keeping the split matters only for speed, not
//!   results, so the split is preserved but both paths must agree.

/// Byte offsets of the r/g/b/m channels for this platform.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChannelOrder {
    pub r: usize,
    pub g: usize,
    pub b: usize,
    pub m: usize,
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd", target_os = "haiku"))]
pub const ORDER: ChannelOrder = ChannelOrder { b: 0, g: 1, r: 2, m: 3 };

#[cfg(target_os = "macos")]
pub const ORDER: ChannelOrder = ChannelOrder { m: 0, r: 1, g: 2, b: 3 };

/// Which channels to invert. Mirrors the four `bool` parameters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Flags {
    pub r: bool,
    pub g: bool,
    pub b: bool,
    pub m: bool,
}

impl Flags {
    /// `TRop::invert` bails out unless at least one colour channel is set.
    /// Returns None for the no-op case.
    pub fn effective(self) -> Option<Flags> {
        if self.r || self.g || self.b {
            Some(self)
        } else {
            None
        }
    }

    /// True when the branch-free form applies: all three colours, no matte.
    pub fn fast(self) -> bool {
        self.r && self.g && self.b && !self.m
    }
}

/// Shared traversal: `ly` rows of `lx` pixels, advancing `stride` pixels/bytes
/// per row. `pixels` points at row 0.
fn for_each_pixel(
    pixels: *mut u8,
    stride: usize,
    lx: usize,
    ly: usize,
    bytes_per_pixel: usize,
    mut f: impl FnMut(*mut u8),
) {
    if lx == 0 || ly == 0 {
        return;
    }
    for y in 0..ly {
        let mut p = unsafe { pixels.add(y * stride) };
        for _ in 0..lx {
            f(p);
            p = unsafe { p.add(bytes_per_pixel) };
        }
    }
}

/// TPixel32: four `unsigned char` in machine order.
pub fn invert_u8(pixels: *mut u8, stride: usize, lx: usize, ly: usize, flags: Flags) {
    let Some(f) = flags.effective() else { return };
    for_each_pixel(pixels, stride, lx, ly, 4, |p| unsafe {
        let px = p as *mut u8;
        if f.fast() {
            let m = *px.add(ORDER.m);
            *px.add(ORDER.r) = m.wrapping_sub(*px.add(ORDER.r));
            *px.add(ORDER.g) = m.wrapping_sub(*px.add(ORDER.g));
            *px.add(ORDER.b) = m.wrapping_sub(*px.add(ORDER.b));
        } else {
            let m = *px.add(ORDER.m);
            if f.r {
                *px.add(ORDER.r) = m.wrapping_sub(*px.add(ORDER.r));
            }
            if f.g {
                *px.add(ORDER.g) = m.wrapping_sub(*px.add(ORDER.g));
            }
            if f.b {
                *px.add(ORDER.b) = m.wrapping_sub(*px.add(ORDER.b));
            }
            if f.m {
                // ~m on unsigned char, not a numeric negation.
                *px.add(ORDER.m) = !m;
            }
        }
    });
}

/// TPixel64: four `unsigned short` in machine order.
pub fn invert_u16(pixels: *mut u8, stride_bytes: usize, lx: usize, ly: usize, flags: Flags) {
    let Some(f) = flags.effective() else { return };
    for_each_pixel(pixels, stride_bytes, lx, ly, 8, |p| unsafe {
        let px = p as *mut u16;
        let at = |i: usize| px.add(i);
        if f.fast() {
            let m = *at(ORDER.m);
            *at(ORDER.r) = m.wrapping_sub(*at(ORDER.r));
            *at(ORDER.g) = m.wrapping_sub(*at(ORDER.g));
            *at(ORDER.b) = m.wrapping_sub(*at(ORDER.b));
        } else {
            let m = *at(ORDER.m);
            if f.r {
                *at(ORDER.r) = m.wrapping_sub(*at(ORDER.r));
            }
            if f.g {
                *at(ORDER.g) = m.wrapping_sub(*at(ORDER.g));
            }
            if f.b {
                *at(ORDER.b) = m.wrapping_sub(*at(ORDER.b));
            }
            if f.m {
                *at(ORDER.m) = !m;
            }
        }
    });
}

/// TPixelF: four `float` in machine order. Matte inverts as `1.0 - m`.
pub fn invert_f32(pixels: *mut u8, stride_bytes: usize, lx: usize, ly: usize, flags: Flags) {
    let Some(f) = flags.effective() else { return };
    for_each_pixel(pixels, stride_bytes, lx, ly, 16, |p| unsafe {
        let px = p as *mut f32;
        let at = |i: usize| px.add(i);
        if f.fast() {
            let m = *at(ORDER.m);
            *at(ORDER.r) = m - *at(ORDER.r);
            *at(ORDER.g) = m - *at(ORDER.g);
            *at(ORDER.b) = m - *at(ORDER.b);
        } else {
            let m = *at(ORDER.m);
            if f.r {
                *at(ORDER.r) = m - *at(ORDER.r);
            }
            if f.g {
                *at(ORDER.g) = m - *at(ORDER.g);
            }
            if f.b {
                *at(ORDER.b) = m - *at(ORDER.b);
            }
            if f.m {
                *at(ORDER.m) = 1.0 - m;
            }
        }
    });
}

/// TPixelGR8: one `unsigned char`. Always `255 - value`; the channel flags do
/// not apply, matching the C++ which only ever calls the no-flags overload for
/// this type.
pub fn invert_gr8(pixels: *mut u8, stride_bytes: usize, lx: usize, ly: usize) {
    for_each_pixel(pixels, stride_bytes, lx, ly, 1, |p| unsafe {
        *p = 255 - *p;
    });
}

// ---------------------------------------------------------------------------
// C ABI, called by TRop::invert() in include/trop.h
// ---------------------------------------------------------------------------

/// `wrap`/`lx`/`ly` are in pixels, as TRasterT reports them.
#[no_mangle]
pub extern "C" fn flare_trop_invert_u8(
    pixels: *mut u8,
    wrap: i32,
    lx: i32,
    ly: i32,
    inv_r: bool,
    inv_g: bool,
    inv_b: bool,
    inv_m: bool,
) {
    let (wrap, lx, ly) = (wrap.max(0) as usize, lx.max(0) as usize, ly.max(0) as usize);
    invert_u8(
        pixels,
        wrap * 4,
        lx,
        ly,
        Flags { r: inv_r, g: inv_g, b: inv_b, m: inv_m },
    );
}

#[no_mangle]
pub extern "C" fn flare_trop_invert_u16(
    pixels: *mut u8,
    wrap: i32,
    lx: i32,
    ly: i32,
    inv_r: bool,
    inv_g: bool,
    inv_b: bool,
    inv_m: bool,
) {
    let (wrap, lx, ly) = (wrap.max(0) as usize, lx.max(0) as usize, ly.max(0) as usize);
    invert_u16(
        pixels,
        wrap * 8,
        lx,
        ly,
        Flags { r: inv_r, g: inv_g, b: inv_b, m: inv_m },
    );
}

#[no_mangle]
pub extern "C" fn flare_trop_invert_f32(
    pixels: *mut u8,
    wrap: i32,
    lx: i32,
    ly: i32,
    inv_r: bool,
    inv_g: bool,
    inv_b: bool,
    inv_m: bool,
) {
    let (wrap, lx, ly) = (wrap.max(0) as usize, lx.max(0) as usize, ly.max(0) as usize);
    invert_f32(
        pixels,
        wrap * 16,
        lx,
        ly,
        Flags { r: inv_r, g: inv_g, b: inv_b, m: inv_m },
    );
}

#[no_mangle]
pub extern "C" fn flare_trop_invert_gr8(pixels: *mut u8, wrap: i32, lx: i32, ly: i32) {
    let (wrap, lx, ly) = (wrap.max(0) as usize, lx.max(0) as usize, ly.max(0) as usize);
    invert_gr8(pixels, wrap, lx, ly);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a raster whose padding bytes are distinguishable from real data,
    /// so a traversal bug that leaks into padding is caught.
    fn raster(lx: usize, ly: usize, stride_px: usize, bpp: usize) -> (Vec<u8>, usize) {
        let stride = stride_px * bpp;
        let mut buf = vec![0xCDu8; stride * ly];
        for y in 0..ly {
            for x in 0..lx {
                let o = y * stride + x * bpp;
                buf[o..o + bpp].fill(0x40u8);
            }
        }
        (buf, stride)
    }

    #[test]
    fn order_is_a_permutation_of_rgbm() {
        let mut v = [ORDER.r, ORDER.g, ORDER.b, ORDER.m];
        v.sort_unstable();
        assert_eq!(v, [0, 1, 2, 3], "channels must not overlap");
    }

    #[test]
    fn effective_rejects_matte_only() {
        // TRop::invert: `if (!invRed && !invGreen && !invBlue) return;`
        let only_matte = Flags { r: false, g: false, b: false, m: true };
        assert_eq!(only_matte.effective(), None);
        assert_eq!(
            Flags { r: true, g: false, b: false, m: false }.effective(),
            Some(Flags { r: true, g: false, b: false, m: false })
        );
    }

    #[test]
    fn fast_path_flag_is_exclusive() {
        assert!(Flags { r: true, g: true, b: true, m: false }.fast());
        assert!(!Flags { r: true, g: true, b: true, m: true }.fast());
        assert!(!Flags { r: true, g: false, b: true, m: false }.fast());
    }

    #[test]
    fn u8_subtractive_invert_wraps_like_truncation() {
        let (mut buf, stride) = raster(2, 1, 2, 4);
        // m=0x40=64, r=g=b=0x40=64 -> m-r = 0
        let p = buf.as_mut_ptr();
        invert_u8(
            p,
            stride,
            2,
            1,
            Flags { r: true, g: true, b: true, m: false },
        );
        let px = |i: usize| unsafe { *buf.as_ptr().add(i) };
        assert_eq!(px(ORDER.r), 0);
        assert_eq!(px(ORDER.g), 0);
        assert_eq!(px(ORDER.b), 0);
        assert_eq!(px(ORDER.m), 0x40, "matte untouched");
    }

    #[test]
    fn u8_wraps_below_zero_as_unsigned_char_truncation_does() {
        let (mut buf, stride) = raster(1, 1, 1, 4);
        // m=0x40, r=0xC8 (200). 64-200 = -136; as unsigned char that is 120.
        buf[ORDER.m] = 0x40;
        buf[ORDER.r] = 0xC8;
        invert_u8(
            buf.as_mut_ptr(),
            stride,
            1,
            1,
            Flags { r: true, g: false, b: false, m: false },
        );
        assert_eq!(buf[ORDER.r], (64i32 - 200) as u8);
    }

    #[test]
    fn u8_matte_inverts_with_bitwise_not() {
        let (mut buf, stride) = raster(1, 1, 1, 4);
        buf[ORDER.m] = 0x0F;
        invert_u8(
            buf.as_mut_ptr(),
            stride,
            1,
            1,
            Flags { r: true, g: false, b: false, m: true },
        );
        assert_eq!(buf[ORDER.m], !0x0Fu8, "~m, not 255-m");
        assert_eq!(buf[ORDER.m], 0xF0);
    }

    #[test]
    fn u8_matting_alone_is_a_no_op() {
        let (mut buf, stride) = raster(2, 2, 2, 4);
        let before = buf.clone();
        invert_u8(
            buf.as_mut_ptr(),
            stride,
            2,
            2,
            Flags { r: false, g: false, b: false, m: true },
        );
        assert_eq!(buf, before);
    }

    #[test]
    fn u8_visits_every_pixel_and_no_padding() {
        let (mut buf, stride) = raster(3, 2, 5, 4); // stride 5 > lx 3
        for i in 0..buf.len() {
            buf[i] = 0x00;
        }
        for y in 0..2 {
            for x in 0..3 {
                buf[y * stride + x * 4 + ORDER.m] = 0xFF;
            }
        }
        invert_u8(
            buf.as_mut_ptr(),
            stride,
            3,
            2,
            Flags { r: true, g: true, b: true, m: false },
        );
        // Real pixels: 0xFF - 0x00 = 0xFF everywhere.
        for y in 0..2 {
            for x in 0..3 {
                let o = y * stride + x * 4;
                assert_eq!(buf[o + ORDER.r], 0xFF, "pixel {x},{y} red");
                assert_eq!(buf[o + ORDER.g], 0xFF, "pixel {x},{y} green");
                assert_eq!(buf[o + ORDER.b], 0xFF, "pixel {x},{y} blue");
            }
            // Padding beyond lx must be untouched.
            for x in 3..5 {
                assert_eq!(buf[y * stride + x * 4 + ORDER.r], 0x00, "padding {x},{y} touched");
            }
        }
    }

    #[test]
    fn u16_matches_u8_semantics_at_wider_range() {
        let (mut buf, stride) = raster(2, 1, 2, 8);
        let p = buf.as_mut_ptr() as *mut u16;
        unsafe {
            let px = p as *mut u16;
            *px.add(ORDER.m) = 1000;
            *px.add(ORDER.r) = 300;
            *px.add(ORDER.g) = 60000;
            *px.add(ORDER.b) = 1;
        }
        invert_u16(
            buf.as_mut_ptr(),
            stride,
            2,
            1,
            Flags { r: true, g: true, b: true, m: false },
        );
        let px = p as *const u16;
        unsafe {
            assert_eq!(*px.add(ORDER.r), 1000u16 - 300);
            // 1000 - 60000 underflows u16, exactly as C++ truncates.
            assert_eq!(*px.add(ORDER.g), (1000i32 - 60000) as u16);
            assert_eq!(*px.add(ORDER.b), 1000u16 - 1);
            assert_eq!(*px.add(ORDER.m), 1000);
        }
    }

    #[test]
    fn f32_matte_uses_one_minus_m_not_bitwise_not() {
        let (mut buf, stride) = raster(1, 1, 1, 16);
        let p = buf.as_mut_ptr() as *mut f32;
        unsafe {
            *p.add(ORDER.m) = 0.25;
            *p.add(ORDER.r) = 0.10;
        }
        invert_f32(
            buf.as_mut_ptr(),
            stride,
            1,
            1,
            Flags { r: true, g: false, b: false, m: true },
        );
        unsafe {
            assert_eq!(*p.add(ORDER.m), 0.75, "1.0 - m");
            assert!(((*p.add(ORDER.r) - 0.15) as f32).abs() < 1e-6);
        }
    }

    #[test]
    fn gr8_is_255_minus_value() {
        let mut buf = vec![0u8, 10, 200, 255, 128];
        let n = buf.len();
        invert_gr8(buf.as_mut_ptr(), n, n, 1);
        assert_eq!(buf, vec![255, 245, 55, 0, 127]);
    }

    #[test]
    fn zero_sized_raster_is_a_no_op() {
        let (mut buf, stride) = raster(2, 2, 2, 4);
        let before = buf.clone();
        invert_u8(
            buf.as_mut_ptr(),
            stride,
            0,
            0,
            Flags { r: true, g: true, b: true, m: true },
        );
        invert_u8(
            buf.as_mut_ptr(),
            stride,
            2,
            0,
            Flags { r: true, g: true, b: true, m: true },
        );
        invert_u8(
            buf.as_mut_ptr(),
            stride,
            0,
            2,
            Flags { r: true, g: true, b: true, m: true },
        );
        assert_eq!(buf, before);
    }

    #[test]
    fn c_abi_rejects_negative_dimensions_without_panicking() {
        let mut buf = vec![7u8; 64];
        let before = buf.clone();
        flare_trop_invert_u8(buf.as_mut_ptr(), -1, -1, -1, true, true, true, true);
        assert_eq!(buf, before);
    }
}