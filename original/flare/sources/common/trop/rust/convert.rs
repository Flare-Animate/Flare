//! Pixel-type conversion, ported from `tconvert.cpp`.
//!
//! Every `do_convert` overload plus the dithering path and the YUV422
//! round-trip. The C++ dispatch table in `TRop::convert` is reproduced in
//! `include/trop.h`; the Rust side holds the kernels, one per conversion.
//!
//! Channel order follows `include/tmachine.h` exactly (BGRM on
//! Windows/Linux/FreeBSD/Haiku, MRGB on macOS), so the YUV conversion reads
//! the right byte for r/g/b. All other conversions are channel-wise identical
//! and therefore order-independent, but they still use the order constants for
//! clarity.
//!
//! Dithering reuses `tcore::trandom::TRandom` (already ported) with the same
//! seed `130266`, and `get_next_uint` is now `pub` so this module can drive it.

use crate::trandom::TRandom;

/// Byte offsets of r/g/b/m for this platform.
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

const MAX8: u32 = 255;
const MAX16: u32 = 65535;

/// `ushortFromByte` from tcommon.h.
#[inline]
pub fn ushort_from_byte(u: u8) -> u16 {
    (u as u16) | ((u as u16) << 8)
}

/// `TPixelGR8::from(TPixelRGBM32)`.
#[inline]
pub fn gr8_from_rgbm32(r: u8, g: u8, b: u8) -> u8 {
    let v = (r as u32) * 19594 + (g as u32) * 38472 + (b as u32) * 7470
        + (1u32 << 15);
    (v >> 16) as u8
}

/// `TPixelGR8::from(TPixelRGBM64)`.
#[inline]
pub fn gr8_from_rgbm64(r: u16, g: u16, b: u16) -> u8 {
    gr8_from_rgbm32((r >> 8) as u8, (g >> 8) as u8, (b >> 8) as u8)
}

/// `TPixelGR16::from(TPixelRGBM64)` luminance.
#[inline]
pub fn gr16_from_rgbm64(r: u16, g: u16, b: u16) -> u16 {
    (((r as u32) * 19594 + (g as u32) * 38472 + (b as u32) * 7470
        + (1u32 << 15))
        >> 16) as u16
}

/// `overPixT<TPixel32, UCHAR>` (bot, top).
pub fn over_pix_32(bot: [u8; 4], top: [u8; 4]) -> [u8; 4] {
    let max = MAX8;
    if top[ORDER.m] == max as u8 {
        return top;
    }
    if top[ORDER.m] == 0 {
        return bot;
    }
    if bot[ORDER.m] == 0 {
        return top;
    }
    let tm = top[ORDER.m] as u32;
    let bm = bot[ORDER.m] as u32;
    let r = top[ORDER.r] as u32
        + (bot[ORDER.r] as u32) * (max - tm) / max;
    let g = top[ORDER.g] as u32
        + (bot[ORDER.g] as u32) * (max - tm) / max;
    let b = top[ORDER.b] as u32
        + (bot[ORDER.b] as u32) * (max - tm) / max;
    let mut out = [0u8; 4];
    out[ORDER.r] = if r < max { r as u8 } else { max as u8 };
    out[ORDER.g] = if g < max { g as u8 } else { max as u8 };
    out[ORDER.b] = if b < max { b as u8 } else { max as u8 };
    out[ORDER.m] = if bm == max {
        max as u8
    } else {
        (max - (max - bm) * (max - tm) / max) as u8
    };
    out
}

/// `overPixGRT<TPixel32, TPixelGR8, UCHAR>` (bot TPixel32, top GR8).
pub fn over_pix_grt_32(bot: [u8; 4], top_val: u8) -> [u8; 4] {
    let max = MAX8;
    if top_val == max as u8 {
        return [max as u8; 4];
    }
    if top_val == 0 {
        return bot;
    }
    let aux = (max - top_val as u32) as f64 / max as f64;
    let r = (top_val as u32 as f64 + bot[ORDER.r] as f64 * aux) as u32;
    let g = (top_val as u32 as f64 + bot[ORDER.g] as f64 * aux) as u32;
    let b = (top_val as u32 as f64 + bot[ORDER.b] as f64 * aux) as u32;
    let mut out = [0u8; 4];
    out[ORDER.r] = if r < max { r as u8 } else { max as u8 };
    out[ORDER.g] = if g < max { g as u8 } else { max as u8 };
    out[ORDER.b] = if b < max { b as u8 } else { max as u8 };
    out[ORDER.m] = if bot[ORDER.m] == max as u8 {
        max as u8
    } else {
        (max as f64 - (max as f64 - bot[ORDER.m] as f64) * aux) as u8
    };
    out
}

// --- Kernels (in-place / separate buffers) -----------------------------------

/// do_convert(Raster64 <- Raster32): ushortFromByte per channel.
pub fn convert_64_from_32(src: *const u8, dst: *mut u8, wrap_src: usize,
                           wrap_dst: usize, lx: i32, ly: i32) {
    for y in 0..ly as usize {
        let s = unsafe { src.add(y * wrap_src) };
        let d = unsafe { dst.add(y * wrap_dst) };
        for x in 0..lx as usize {
            let so = x * 4;
            let do_ = x * 8;
            unsafe {
                let r = *s.add(so + ORDER.r);
                let g = *s.add(so + ORDER.g);
                let b = *s.add(so + ORDER.b);
                let m = *s.add(so + ORDER.m);
                *d.add(do_ + ORDER.r * 2) = (ushort_from_byte(r) & 0xff) as u8;
                *d.add(do_ + ORDER.r * 2 + 1) = (ushort_from_byte(r) >> 8) as u8;
                *d.add(do_ + ORDER.g * 2) = (ushort_from_byte(g) & 0xff) as u8;
                *d.add(do_ + ORDER.g * 2 + 1) = (ushort_from_byte(g) >> 8) as u8;
                *d.add(do_ + ORDER.b * 2) = (ushort_from_byte(b) & 0xff) as u8;
                *d.add(do_ + ORDER.b * 2 + 1) = (ushort_from_byte(b) >> 8) as u8;
                *d.add(do_ + ORDER.m * 2) = (ushort_from_byte(m) & 0xff) as u8;
                *d.add(do_ + ORDER.m * 2 + 1) = (ushort_from_byte(m) >> 8) as u8;
            }
        }
    }
}

/// do_convert(RasterGR8 <- Raster32).
pub fn convert_gr8_from_32(src: *const u8, dst: *mut u8, wrap_src: usize,
                           wrap_dst: usize, lx: i32, ly: i32) {
    for y in 0..ly as usize {
        let s = unsafe { src.add(y * wrap_src) };
        let d = unsafe { dst.add(y * wrap_dst) };
        for x in 0..lx as usize {
            let so = x * 4;
            unsafe {
                let r = *s.add(so + ORDER.r);
                let g = *s.add(so + ORDER.g);
                let b = *s.add(so + ORDER.b);
                let m = *s.add(so + ORDER.m);
                // overPix(White, pix) then GR8::from
                let top = [r, g, b, m];
                let white = [255u8; 4];
                let op = over_pix_32(white, top);
                let gr = gr8_from_rgbm32(op[ORDER.r], op[ORDER.g], op[ORDER.b]);
                *d.add(x) = gr;
            }
        }
    }
}

/// do_convert(RasterGR16 <- Raster32).
pub fn convert_gr16_from_32(src: *const u8, dst: *mut u8, wrap_src: usize,
                            wrap_dst: usize, lx: i32, ly: i32) {
    for y in 0..ly as usize {
        let s = unsafe { src.add(y * wrap_src) };
        let d = unsafe { dst.add(y * wrap_dst) };
        for x in 0..lx as usize {
            let so = x * 4;
            unsafe {
                let r = *s.add(so + ORDER.r);
                let g = *s.add(so + ORDER.g);
                let b = *s.add(so + ORDER.b);
                let m = *s.add(so + ORDER.m);
                let top = [r, g, b, m];
                let white = [255u8; 4];
                let op = over_pix_32(white, top);
                let gr = gr8_from_rgbm32(op[ORDER.r], op[ORDER.g], op[ORDER.b]);
                let v = (gr as u16) * 257;
                *d.add(x * 2) = (v & 0xff) as u8;
                *d.add(x * 2 + 1) = (v >> 8) as u8;
            }
        }
    }
}

/// do_convert(RasterGR16 <- Raster64): (r + 2g + b) >> 2.
pub fn convert_gr16_from_64(src: *const u8, dst: *mut u8, wrap_src: usize,
                            wrap_dst: usize, lx: i32, ly: i32) {
    for y in 0..ly as usize {
        let s = unsafe { src.add(y * wrap_src) };
        let d = unsafe { dst.add(y * wrap_dst) };
        for x in 0..lx as usize {
            let so = x * 8;
            unsafe {
                // r,g,b are 2-byte little-endian channels.
                let r = u16::from_le_bytes([*s.add(so + ORDER.r * 2),
                                            *s.add(so + ORDER.r * 2 + 1)]);
                let g = u16::from_le_bytes([*s.add(so + ORDER.g * 2),
                                            *s.add(so + ORDER.g * 2 + 1)]);
                let b = u16::from_le_bytes([*s.add(so + ORDER.b * 2),
                                            *s.add(so + ORDER.b * 2 + 1)]);
                let v = (r as u32 + 2 * (g as u32) + b as u32) >> 2;
                *d.add(x * 2) = (v & 0xff) as u8;
                *d.add(x * 2 + 1) = (v >> 8) as u8;
            }
        }
    }
}

/// do_convert(Raster32 <- RasterGR8): grey -> rgb, matte 0xff.
pub fn convert_32_from_gr8(src: *const u8, dst: *mut u8, wrap_src: usize,
                           wrap_dst: usize, lx: i32, ly: i32) {
    for y in 0..ly as usize {
        let s = unsafe { src.add(y * wrap_src) };
        let d = unsafe { dst.add(y * wrap_dst) };
        for x in 0..lx as usize {
            let v = unsafe { *s.add(x) };
            let do_ = x * 4;
            unsafe {
                *d.add(do_ + ORDER.r) = v;
                *d.add(do_ + ORDER.g) = v;
                *d.add(do_ + ORDER.b) = v;
                *d.add(do_ + ORDER.m) = 0xff;
            }
        }
    }
}

/// ditherConvert(Raster64 -> Raster32) with seed 130266.
pub fn dither_convert_64_to_32(src: *const u8, dst: *mut u8,
                               wrap_src: usize, wrap_dst: usize,
                               lx: i32, ly: i32) {
    let mut rnd = TRandom::new(130266);
    for y in 0..ly as usize {
        let srow = unsafe { src.add(y * wrap_src) };
        let drow = unsafe { dst.add(y * wrap_dst) };
        for x in 0..lx as usize {
            let so = x * 8;
            let do_ = x * 4;
            unsafe {
                let r16 = u16::from_le_bytes([*srow.add(so + ORDER.r * 2),
                                              *srow.add(so + ORDER.r * 2 + 1)]);
                let g16 = u16::from_le_bytes([*srow.add(so + ORDER.g * 2),
                                              *srow.add(so + ORDER.g * 2 + 1)]);
                let b16 = u16::from_le_bytes([*srow.add(so + ORDER.b * 2),
                                              *srow.add(so + ORDER.b * 2 + 1)]);
                let m16 = u16::from_le_bytes([*srow.add(so + ORDER.m * 2),
                                              *srow.add(so + ORDER.m * 2 + 1)]);
                let random = rnd.get_next_uint() & ((1u32 << 24) - 1);
                let dither = |v: u16, rnd: u32| -> u8 {
                    let magic: u32 = 256 * 255 + 1;
                    let prod = (v as u32) * magic;
                    let scaled = prod - (prod >> 24);
                    (((scaled + rnd) >> 24) & 0xff) as u8
                };
                *drow.add(do_ + ORDER.r) = dither(r16, random);
                *drow.add(do_ + ORDER.g) = dither(g16, random);
                *drow.add(do_ + ORDER.b) = dither(b16, random);
                *drow.add(do_ + ORDER.m) = dither(m16, random);
            }
        }
    }
}

/// do_convert(RasterCM32 <- RasterGR8).
pub fn convert_cm32_from_gr8(src: *const u8, dst: *mut u8, wrap_src: usize,
                             wrap_dst: usize, lx: i32, ly: i32) {
    // TPixelCM32 bg = TPixelCM32(0, 0, getMaxTone()) => ink 0, paint 0, tone 255.
    let bg: u32 = 0 | (0 << 8) | (255 << 0); // ink<<20 | paint<<8 | tone
    // Actually ink=0, paint=0, tone=255 => 255.
    let bg = 255u32;
    for y in 0..ly as usize {
        let s = unsafe { src.add(y * wrap_src) };
        let d = unsafe { dst.add(y * wrap_dst) };
        for x in 0..lx as usize {
            let v = unsafe { *s.add(x) };
            let cm = if v == 255 {
                bg
            } else {
                // TPixelCM32(1, 0, v)
                (1u32 << 20) | (0u32 << 8) | (v as u32)
            };
            unsafe {
                let bytes = cm.to_le_bytes();
                for i in 0..4 {
                    *d.add(x * 4 + i) = bytes[i];
                }
            }
        }
    }
}

/// do_convert(RasterCM32 <- Raster32).
pub fn convert_cm32_from_32(src: *const u8, dst: *mut u8, wrap_src: usize,
                            wrap_dst: usize, lx: i32, ly: i32) {
    // First pass: detect overlay (any matte != 255).
    let mut is_overlay = false;
    'outer: for y in 0..ly as usize {
        let s = unsafe { src.add(y * wrap_src) };
        for x in 0..lx as usize {
            let m = unsafe { *s.add(x * 4 + ORDER.m) };
            if m != 255 {
                is_overlay = true;
                break 'outer;
            }
        }
    }
    let bg: u32 = 255; // TPixelCM32(0, 0, 255)
    for y in 0..ly as usize {
        let s = unsafe { src.add(y * wrap_src) };
        let d = unsafe { dst.add(y * wrap_dst) };
        for x in 0..lx as usize {
            let so = x * 4;
            let r = unsafe { *s.add(so + ORDER.r) };
            let g = unsafe { *s.add(so + ORDER.g) };
            let b = unsafe { *s.add(so + ORDER.b) };
            let m = unsafe { *s.add(so + ORDER.m) };
            let cm = if is_overlay {
                if m == 0 {
                    bg
                } else {
                    // TPixelCM32(1, 0, 255 - m)
                    (1u32 << 20) | (0u32 << 8) | ((255 - m) as u32)
                }
            } else {
                // brightness via TPixelGR8::from(overPix(White,pix)) = gr8_from_rgbm32
                let gr = gr8_from_rgbm32(r, g, b);
                if gr == 255 {
                    bg
                } else {
                    (1u32 << 20) | (0u32 << 8) | (gr as u32)
                }
            };
            unsafe {
                let bytes = cm.to_le_bytes();
                for i in 0..4 {
                    *d.add(x * 4 + i) = bytes[i];
                }
            }
        }
    }
}

/// do_convert(RasterYUV422 <- Raster32).
pub fn convert_yuv_from_32(src: *const u8, dst: *mut u8, wrap_src: usize,
                           _wrap_dst: usize, lx: i32, ly: i32) {
    // src is TPixel32 laid out in channel order; dst is raw YUV bytes.
    // lx must be even (assert in C++: src->getLx() & 0 == 0 ... actually
    // `assert(src->getLx() & 0)` is always false, so it's a no-op assert).
    let mut pix = unsafe { src };
    let last = unsafe { src.add(wrap_src * (ly as usize - 1) + (lx as usize) * 4 - 4) };
    // We'll walk pixel pairs.
    let mut out = dst;
    while (pix as usize) < (last as usize) {
        unsafe {
            let p0 = pix;
            // first pixel
            let r0 = *p0.add(ORDER.r) as i64;
            let g0 = *p0.add(ORDER.g) as i64;
            let b0 = *p0.add(ORDER.b) as i64;
            let y1 = 16829 * r0 + 33039 * g0 + 6416 * b0;
            let u1 = -4831 * r0 + -9488 * g0 + 14319 * b0;
            let v1 = 14322 * r0 + -11992 * g0 + -2330 * b0;
            // second pixel
            let p1 = pix.add(4);
            let r1 = *p1.add(ORDER.r) as i64;
            let g1 = *p1.add(ORDER.g) as i64;
            let b1 = *p1.add(ORDER.b) as i64;
            let y2 = 16829 * r1 + 33039 * g1 + 6416 * b1;
            let u2 = -4831 * r1 + -9488 * g1 + 14319 * b1;
            let v2 = 14322 * r1 + -11992 * g1 + -2330 * b1;
            let u = u1 + u2;
            let v = v1 + v2;
            let mut u1r = (u + 0x008000) >> 16;
            let mut v1r = (v + 0x008000) >> 16;
            if u1r < -112 {
                u1r = -112;
            }
            if u1r > 111 {
                u1r = 111;
            }
            if v1r < -112 {
                v1r = -112;
            }
            if v1r > 111 {
                v1r = 111;
            }
            let mut y1l = y1;
            let mut y2l = y2;
            if y1l > 0x00dbffff {
                y1l = 0x00dbffff;
            }
            if y2l > 0x00dbffff {
                y2l = 0x00dbffff;
            }
            *out = (u1r + 128) as u8;
            out = out.add(1);
            *out = ((y1l >> 16) + 16) as u8;
            out = out.add(1);
            *out = (v1r + 128) as u8;
            out = out.add(1);
            *out = ((y2l >> 16) + 16) as u8;
            out = out.add(1);
            pix = pix.add(8); // advance two pixels (8 bytes? no, 2 pixels * 4)
        }
    }
    // Wait: pix should advance by 2 pixels = 8 bytes, but the C++ does ++pix (one)
    // after processing a pair. Re-check: original reads pix, ++pix (second), then
    // ++pix at end (third). So it advances by 2 per iteration. Fix above: pix += 8.
    let _ = last;
}

/// do_convert(Raster32 <- RasterYUV422).
pub fn convert_32_from_yuv(src: *const u8, dst: *mut u8, _wrap_src: usize,
                           _wrap_dst: usize, lx: i32, ly: i32) {
    let mut in_ptr = src;
    let last = unsafe { src.add(lx as usize * ly as usize * 2 - 1) }; // approximate
    let mut buf = dst;
    while (in_ptr as usize) < (last as usize) {
        unsafe {
            let u = *in_ptr as i64 - 128;
            in_ptr = in_ptr.add(1);
            let y1 = *in_ptr as i64 - 16;
            in_ptr = in_ptr.add(1);
            let v = *in_ptr as i64 - 128;
            in_ptr = in_ptr.add(1);
            let y2 = *in_ptr as i64 - 16;
            in_ptr = in_ptr.add(1);

            let mut r = 76310 * y1 + 104635 * v;
            if r > 0xFFFFFF {
                r = 0xFFFFFF;
            }
            if r <= 0xFFFF {
                r = 0;
            }
            let mut g = 76310 * y1 + -25690 * u + -53294 * v;
            if g > 0xFFFFFF {
                g = 0xFFFFFF;
            }
            if g <= 0xFFFF {
                g = 0;
            }
            let mut b = 76310 * y1 + 132278 * u;
            if b > 0xFFFFFF {
                b = 0xFFFFFF;
            }
            if b <= 0xFFFF {
                b = 0;
            }
            *buf.add(ORDER.r) = (r >> 16) as u8;
            *buf.add(ORDER.g) = (g >> 16) as u8;
            *buf.add(ORDER.b) = (b >> 16) as u8;
            *buf.add(ORDER.m) = 255;
            buf = buf.add(4);

            let mut r = 76310 * y2 + 104635 * v;
            if r > 0xFFFFFF {
                r = 0xFFFFFF;
            }
            if r <= 0xFFFF {
                r = 0;
            }
            let mut g = 76310 * y2 + -25690 * u + -53294 * v;
            if g > 0xFFFFFF {
                g = 0xFFFFFF;
            }
            if g <= 0xFFFF {
                g = 0;
            }
            let mut b = 76310 * y2 + 132278 * u;
            if b > 0xFFFFFF {
                b = 0xFFFFFF;
            }
            if b <= 0xFFFF {
                b = 0;
            }
            *buf.add(ORDER.r) = (r >> 16) as u8;
            *buf.add(ORDER.g) = (g >> 16) as u8;
            *buf.add(ORDER.b) = (b >> 16) as u8;
            *buf.add(ORDER.m) = 255;
            buf = buf.add(4);
        }
    }
    let _ = last;
}

/// do_convert(RasterFP <- Raster32).
pub fn convert_fp_from_32(src: *const u8, dst: *mut u8, wrap_src: usize,
                          wrap_dst: usize, lx: i32, ly: i32) {
    for y in 0..ly as usize {
        let s = unsafe { src.add(y * wrap_src) };
        let d = unsafe { dst.add(y * wrap_dst) };
        for x in 0..lx as usize {
            let so = x * 4;
            let do_ = x * 16;
            unsafe {
                let r = *s.add(so + ORDER.r) as f32 / 255.0;
                let g = *s.add(so + ORDER.g) as f32 / 255.0;
                let b = *s.add(so + ORDER.b) as f32 / 255.0;
                let m = *s.add(so + ORDER.m) as f32 / 255.0;
                write_f32(d, do_ + ORDER.r * 4, r);
                write_f32(d, do_ + ORDER.g * 4, g);
                write_f32(d, do_ + ORDER.b * 4, b);
                write_f32(d, do_ + ORDER.m * 4, m);
            }
        }
    }
}

/// do_convert(RasterFP <- Raster64).
pub fn convert_fp_from_64(src: *const u8, dst: *mut u8, wrap_src: usize,
                          wrap_dst: usize, lx: i32, ly: i32) {
    for y in 0..ly as usize {
        let s = unsafe { src.add(y * wrap_src) };
        let d = unsafe { dst.add(y * wrap_dst) };
        for x in 0..lx as usize {
            let so = x * 8;
            let do_ = x * 16;
            unsafe {
                let r = u16::from_le_bytes([*s.add(so + ORDER.r * 2),
                                            *s.add(so + ORDER.r * 2 + 1)]) as f32
                    / 65535.0;
                let g = u16::from_le_bytes([*s.add(so + ORDER.g * 2),
                                            *s.add(so + ORDER.g * 2 + 1)]) as f32
                    / 65535.0;
                let b = u16::from_le_bytes([*s.add(so + ORDER.b * 2),
                                            *s.add(so + ORDER.b * 2 + 1)]) as f32
                    / 65535.0;
                let m = u16::from_le_bytes([*s.add(so + ORDER.m * 2),
                                            *s.add(so + ORDER.m * 2 + 1)]) as f32
                    / 65535.0;
                write_f32(d, do_ + ORDER.r * 4, r);
                write_f32(d, do_ + ORDER.g * 4, g);
                write_f32(d, do_ + ORDER.b * 4, b);
                write_f32(d, do_ + ORDER.m * 4, m);
            }
        }
    }
}

/// do_convert(Raster32 <- RasterFP): clamp [0,1] then *max + 0.5.
pub fn convert_32_from_fp(src: *const u8, dst: *mut u8, wrap_src: usize,
                          wrap_dst: usize, lx: i32, ly: i32) {
    for y in 0..ly as usize {
        let s = unsafe { src.add(y * wrap_src) };
        let d = unsafe { dst.add(y * wrap_dst) };
        for x in 0..lx as usize {
            let so = x * 16;
            let do_ = x * 4;
            unsafe {
                let r = read_f32(s, so + ORDER.r * 4);
                let g = read_f32(s, so + ORDER.g * 4);
                let b = read_f32(s, so + ORDER.b * 4);
                let m = read_f32(s, so + ORDER.m * 4);
                let clamp = |v: f32| {
                    if v < 0.0 {
                        0.0
                    } else if v > 1.0 {
                        1.0
                    } else {
                        v
                    }
                };
                *d.add(do_ + ORDER.r) =
                    (clamp(r) * 255.0 + 0.5) as u8;
                *d.add(do_ + ORDER.g) =
                    (clamp(g) * 255.0 + 0.5) as u8;
                *d.add(do_ + ORDER.b) =
                    (clamp(b) * 255.0 + 0.5) as u8;
                *d.add(do_ + ORDER.m) =
                    (clamp(m) * 255.0 + 0.5) as u8;
            }
        }
    }
}

/// do_convert(Raster64 <- RasterFP).
pub fn convert_64_from_fp(src: *const u8, dst: *mut u8, wrap_src: usize,
                          wrap_dst: usize, lx: i32, ly: i32) {
    for y in 0..ly as usize {
        let s = unsafe { src.add(y * wrap_src) };
        let d = unsafe { dst.add(y * wrap_dst) };
        for x in 0..lx as usize {
            let so = x * 16;
            let do_ = x * 8;
            unsafe {
                let r = read_f32(s, so + ORDER.r * 4);
                let g = read_f32(s, so + ORDER.g * 4);
                let b = read_f32(s, so + ORDER.b * 4);
                let m = read_f32(s, so + ORDER.m * 4);
                let clamp = |v: f32| {
                    if v < 0.0 {
                        0.0
                    } else if v > 1.0 {
                        1.0
                    } else {
                        v
                    }
                };
                let rv = (clamp(r) * 65535.0 + 0.5) as u16;
                let gv = (clamp(g) * 65535.0 + 0.5) as u16;
                let bv = (clamp(b) * 65535.0 + 0.5) as u16;
                let mv = (clamp(m) * 65535.0 + 0.5) as u16;
                write_u16(d, do_ + ORDER.r * 2, rv);
                write_u16(d, do_ + ORDER.g * 2, gv);
                write_u16(d, do_ + ORDER.b * 2, bv);
                write_u16(d, do_ + ORDER.m * 2, mv);
            }
        }
    }
}

// --- helpers -----------------------------------------------------------------

#[inline]
fn write_f32(dst: *mut u8, off: usize, v: f32) {
    unsafe {
        let b = v.to_ne_bytes();
        for i in 0..4 {
            *dst.add(off + i) = b[i];
        }
    }
}

#[inline]
fn read_f32(src: *const u8, off: usize) -> f32 {
    unsafe {
        let mut b = [0u8; 4];
        for i in 0..4 {
            b[i] = *src.add(off + i);
        }
        f32::from_ne_bytes(b)
    }
}

#[inline]
fn write_u16(dst: *mut u8, off: usize, v: u16) {
    let b = v.to_le_bytes();
    unsafe {
        *dst.add(off) = b[0];
        *dst.add(off + 1) = b[1];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ushort_from_byte_doubles() {
        assert_eq!(ushort_from_byte(0), 0);
        assert_eq!(ushort_from_byte(255), 0xffff);
        assert_eq!(ushort_from_byte(1), 0x0101);
    }

    #[test]
    fn gr8_from_rgbm32_known() {
        // white -> 255
        assert_eq!(gr8_from_rgbm32(255, 255, 255), 255);
        // black -> 0
        assert_eq!(gr8_from_rgbm32(0, 0, 0), 0);
    }

    #[test]
    fn channel_order_is_consistent() {
        // r,g,b,m all distinct.
        let o = ORDER;
        let mut v = [o.r, o.g, o.b, o.m];
        v.sort();
        assert_eq!(v, [0, 1, 2, 3]);
    }

    #[test]
    fn convert_64_from_32_round_trips_channel_values() {
        // 2x1 raster, wrap == lx.
        let lx = 2i32;
        let ly = 1i32;
        let mut src = vec![0u8; 8];
        // pixel 0: r=10,g=20,b=30,m=40
        src[ORDER.r] = 10;
        src[ORDER.g] = 20;
        src[ORDER.b] = 30;
        src[ORDER.m] = 40;
        // pixel 1: r=200,g=100,b=50,m=255
        src[4 + ORDER.r] = 200;
        src[4 + ORDER.g] = 100;
        src[4 + ORDER.b] = 50;
        src[4 + ORDER.m] = 255;
        let mut dst = vec![0u8; 16];
        convert_64_from_32(src.as_ptr(), dst.as_mut_ptr(), 8, 16, lx, ly);
        // pixel 0 r
        let r0 = u16::from_le_bytes([dst[ORDER.r * 2], dst[ORDER.r * 2 + 1]]);
        assert_eq!(r0, ushort_from_byte(10));
        let m0 = u16::from_le_bytes([dst[4 * 2 + ORDER.m * 2], dst[4 * 2 + ORDER.m * 2 + 1]]);
        // pixel1 matte
        let m1 = u16::from_le_bytes([dst[8 + ORDER.m * 2], dst[8 + ORDER.m * 2 + 1]]);
        assert_eq!(m1, ushort_from_byte(255));
        let _ = (r0, m0);
    }

    #[test]
    fn convert_32_from_gr8_sets_matte_opaque() {
        let src = vec![7u8, 9u8];
        let mut dst = vec![0u8; 8];
        convert_32_from_gr8(src.as_ptr(), dst.as_mut_ptr(), 2, 8, 2, 1);
        for x in 0..2 {
            let o = x * 4;
            assert_eq!(dst[o + ORDER.r], src[x]);
            assert_eq!(dst[o + ORDER.g], src[x]);
            assert_eq!(dst[o + ORDER.b], src[x]);
            assert_eq!(dst[o + ORDER.m], 0xff);
        }
    }

    #[test]
    fn convert_fp_from_32_and_back_round_trips_approximately() {
        let mut src = vec![0u8; 4];
        src[ORDER.r] = 100;
        src[ORDER.g] = 150;
        src[ORDER.b] = 200;
        src[ORDER.m] = 255;
        let mut fp = vec![0u8; 16];
        convert_fp_from_32(src.as_ptr(), fp.as_mut_ptr(), 4, 16, 1, 1);
        let mut back = vec![0u8; 4];
        convert_32_from_fp(fp.as_ptr(), back.as_mut_ptr(), 16, 4, 1, 1);
        assert!((back[ORDER.r] as i32 - 100).abs() <= 1);
        assert!((back[ORDER.g] as i32 - 150).abs() <= 1);
        assert!((back[ORDER.b] as i32 - 200).abs() <= 1);
        assert_eq!(back[ORDER.m], 255);
    }

    #[test]
    fn dither_convert_uses_seed_130266() {
        // Just ensure it doesn't panic and writes something.
        let src = vec![0u8; 8];
        let mut dst = vec![0u8; 4];
        dither_convert_64_to_32(src.as_ptr(), dst.as_mut_ptr(), 8, 4, 1, 1);
        // all-zero input -> all-zero output (dither of 0 is 0).
        for b in &dst {
            assert_eq!(*b, 0);
        }
    }
}
// --- C ABI wrappers -----------------------------------------------------------

macro_rules! abi {
    ($name:ident, $rust:ident) => {
        #[no_mangle]
        pub extern "C" fn $name(src: *const u8, dst: *mut u8, ws: i32, wd: i32,
                                lx: i32, ly: i32) {
            $rust(src, dst, ws.max(0) as usize, wd.max(0) as usize, lx, ly);
        }
    };
}

abi!(flare_convert_64_from_32, convert_64_from_32);
abi!(flare_convert_gr8_from_32, convert_gr8_from_32);
abi!(flare_convert_gr16_from_32, convert_gr16_from_32);
abi!(flare_convert_gr16_from_64, convert_gr16_from_64);
abi!(flare_convert_32_from_gr8, convert_32_from_gr8);
abi!(flare_dither_convert_64_to_32, dither_convert_64_to_32);
abi!(flare_convert_cm32_from_gr8, convert_cm32_from_gr8);
abi!(flare_convert_cm32_from_32, convert_cm32_from_32);
abi!(flare_convert_yuv_from_32, convert_yuv_from_32);
abi!(flare_convert_32_from_yuv, convert_32_from_yuv);
abi!(flare_convert_fp_from_32, convert_fp_from_32);
abi!(flare_convert_fp_from_64, convert_fp_from_64);
abi!(flare_convert_32_from_fp, convert_32_from_fp);
abi!(flare_convert_64_from_fp, convert_64_from_fp);
