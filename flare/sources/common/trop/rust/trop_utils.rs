//! Utility kernels from `trop.cpp`, ported to Rust.
//!
//! Covers the self-contained functions: `isOpaque`, `swapRBChannels`,
//! `gammaCorrect`, `gammaCorrectRGBM`, `setChannel`. The remaining `trop.cpp`
//! functions (`copy`, `shrink`, `makeStereoRaster`, `toLinearRGB`, ...) still
//! live in C++ and are tracked in KEEPS_CPP.

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd", target_os = "haiku"))]
const R: usize = 2;
#[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd", target_os = "haiku"))]
const G: usize = 1;
#[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd", target_os = "haiku"))]
const B: usize = 0;
#[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd", target_os = "haiku"))]
const M: usize = 3;
#[cfg(target_os = "macos")]
const R: usize = 1;
#[cfg(target_os = "macos")]
const G: usize = 2;
#[cfg(target_os = "macos")]
const B: usize = 3;
#[cfg(target_os = "macos")]
const M: usize = 0;

/// isOpaque: true if every pixel's matte is max.
pub fn is_opaque_u8(src: *const u8, wrap: usize, lx: i32, ly: i32) -> i32 {
    for y in 0..ly as usize {
        let row = unsafe { src.add(y * wrap) };
        for x in 0..lx as usize {
            if unsafe { *row.add(x * 4 + M) } != 255 { return 0; }
        }
    }
    1
}

pub fn is_opaque_u16(src: *const u8, wrap: usize, lx: i32, ly: i32) -> i32 {
    for y in 0..ly as usize {
        let row = unsafe { src.add(y * wrap) };
        for x in 0..lx as usize {
            let off = x * 8 + M * 2;
            let v = unsafe { u16::from_le_bytes([*row.add(off), *row.add(off + 1)]) };
            if v != 65535 { return 0; }
        }
    }
    1
}

/// swapRBChannels in place on a TPixel32 raster.
pub fn swap_rb_channels(src: *mut u8, wrap: usize, lx: i32, ly: i32) {
    for y in 0..ly as usize {
        let row = unsafe { src.add(y * wrap) };
        for x in 0..lx as usize {
            let off = x * 4;
            unsafe {
                let r = *row.add(off + R);
                let b = *row.add(off + B);
                *row.add(off + R) = b;
                *row.add(off + B) = r;
            }
        }
    }
}

/// gammaCorrect: raster->x = pow(x, gamma) per channel (in place).
pub fn gamma_correct_u8(src: *mut u8, wrap: usize, lx: i32, ly: i32, gamma: f64) {
    for y in 0..ly as usize {
        let row = unsafe { src.add(y * wrap) };
        for x in 0..lx as usize {
            let off = x * 4;
            unsafe {
                for c in 0..4usize {
                    let v = *row.add(off + c) as f64 / 255.0;
                    let nv = v.powf(gamma) * 255.0;
                    *row.add(off + c) = nv.round().min(255.0).max(0.0) as u8;
                }
            }
        }
    }
}

/// gammaCorrectRGBM: per-channel gammas.
pub fn gamma_correct_rgbm_u8(
    src: *mut u8, wrap: usize, lx: i32, ly: i32,
    gr: f64, gg: f64, gb: f64, gm: f64,
) {
    let gammas = [gr, gg, gb, gm];
    let chans = [R, G, B, M];
    for y in 0..ly as usize {
        let row = unsafe { src.add(y * wrap) };
        for x in 0..lx as usize {
            let off = x * 4;
            for i in 0..4usize {
                unsafe {
                    let v = *row.add(off + chans[i]) as f64 / 255.0;
                    let nv = v.powf(gammas[i]) * 255.0;
                    *row.add(off + chans[i]) = nv.round().min(255.0).max(0.0) as u8;
                }
            }
        }
    }
}

/// setChannel: extract one channel into a grayscale raster (matte of dst?).
/// Simplified: copies channel `chan` (0=R,1=G,2=B,3=M) of src into dst's R/G/B/M
/// all equal (grey). chan: 0=R,1=G,2=B,3=M.
pub fn set_channel_u8(
    src: *const u8, dst: *mut u8, wrap_src: usize, wrap_dst: usize,
    lx: i32, ly: i32, chan: i32,
) {
    let ci = match chan { 0 => R, 1 => G, 2 => B, 3 => M, _ => M };
    for y in 0..ly as usize {
        let s = unsafe { src.add(y * wrap_src) };
        let d = unsafe { dst.add(y * wrap_dst) };
        for x in 0..lx as usize {
            let v = unsafe { *s.add(x * 4 + ci) };
            let off = x * 4;
            unsafe {
                *d.add(off + R) = v;
                *d.add(off + G) = v;
                *d.add(off + B) = v;
                *d.add(off + M) = 255;
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn flare_is_opaque_u8(src: *const u8, wrap: i32, lx: i32, ly: i32) -> i32 {
    is_opaque_u8(src, wrap.max(0) as usize, lx, ly)
}
#[no_mangle]
pub extern "C" fn flare_is_opaque_u16(src: *const u8, wrap: i32, lx: i32, ly: i32) -> i32 {
    is_opaque_u16(src, wrap.max(0) as usize, lx, ly)
}
#[no_mangle]
pub extern "C" fn flare_swap_rb_channels(src: *mut u8, wrap: i32, lx: i32, ly: i32) {
    swap_rb_channels(src, wrap.max(0) as usize, lx, ly)
}
#[no_mangle]
pub extern "C" fn flare_gamma_correct_u8(src: *mut u8, wrap: i32, lx: i32, ly: i32, gamma: f64) {
    gamma_correct_u8(src, wrap.max(0) as usize, lx, ly, gamma)
}
#[no_mangle]
pub extern "C" fn flare_gamma_correct_rgbm_u8(
    src: *mut u8, wrap: i32, lx: i32, ly: i32, gr: f64, gg: f64, gb: f64, gm: f64,
) {
    gamma_correct_rgbm_u8(src, wrap.max(0) as usize, lx, ly, gr, gg, gb, gm)
}
#[no_mangle]
pub extern "C" fn flare_set_channel_u8(
    src: *const u8, dst: *mut u8, ws: i32, wd: i32, lx: i32, ly: i32, chan: i32,
) {
    set_channel_u8(src, dst, ws.max(0) as usize, wd.max(0) as usize, lx, ly, chan)
}

/// copyAndSwapRBChannels: returns a new raster with R and B swapped.
pub fn copy_and_swap_rb_channels(
    src: *const u8, dst: *mut u8, wrap_src: usize, wrap_dst: usize,
    lx: i32, ly: i32,
) {
    for y in 0..ly as usize {
        let s = unsafe { src.add(y * wrap_src) };
        let d = unsafe { dst.add(y * wrap_dst) };
        for x in 0..lx as usize {
            let so = x * 4;
            let do_ = x * 4;
            unsafe {
                *d.add(do_ + R) = *s.add(so + B);
                *d.add(do_ + G) = *s.add(so + G);
                *d.add(do_ + B) = *s.add(so + R);
                *d.add(do_ + M) = *s.add(so + M);
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn flare_copy_and_swap_rb_channels(
    src: *const u8, dst: *mut u8, ws: i32, wd: i32, lx: i32, ly: i32,
) {
    copy_and_swap_rb_channels(src, dst, ws.max(0) as usize, wd.max(0) as usize, lx, ly)
}

/// shrink: downsample by taking every `shrink`-th pixel (nearest-neighbor).
/// Operates on raw bytes; pixel_size bytes per pixel.
pub fn shrink(
    src: *const u8, dst: *mut u8, wrap_src: usize, wrap_dst: usize,
    lx_src: i32, ly_src: i32, pixel_size: usize, shrink: i32,
) {
    let lx = (lx_src - 1) / shrink + 1;
    let ly = (ly_src - 1) / shrink + 1;
    for i in 0..ly as usize {
        let bufin = unsafe { src.add((i * shrink as usize) * wrap_src) };
        let bufout = unsafe { dst.add(i * wrap_dst) };
        for j in 0..lx as usize {
            unsafe {
                std::ptr::copy_nonoverlapping(
                    bufin.add(j * shrink as usize * pixel_size),
                    bufout.add(j * pixel_size),
                    pixel_size,
                );
            }
        }
    }
}

/// makeStereoRaster: left.g = right.g, left.b = right.b (in place on left).
pub fn make_stereo_raster(left: *mut u8, right: *const u8, wrap: usize,
                          lx: i32, ly: i32, bpp: usize) {
    for i in 0..ly as usize {
        let pl = unsafe { left.add(i * wrap) };
        let pr = unsafe { right.add(i * wrap) };
        for j in 0..lx as usize {
            unsafe {
                *pl.add(j * bpp + G) = *pr.add(j * bpp + G);
                *pl.add(j * bpp + B) = *pr.add(j * bpp + B);
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn flare_shrink(
    src: *const u8, dst: *mut u8, ws: i32, wd: i32,
    lx: i32, ly: i32, pixel_size: i32, shrink: i32,
) {
    shrink(src, dst, ws.max(0) as usize, wd.max(0) as usize, lx, ly,
           pixel_size as usize, shrink)
}

#[no_mangle]
pub extern "C" fn flare_make_stereo_raster(
    left: *mut u8, right: *const u8, wrap: i32, lx: i32, ly: i32, bpp: i32,
) {
    make_stereo_raster(left, right, wrap.max(0) as usize, lx, ly, bpp as usize)
}

/// copy: if pixel sizes match, memcpy; else convert. This Rust version handles
/// the same-pixel-size fast path (memcpy). Cross-type conversion delegates
/// to the existing convert kernels (not duplicated here).
pub fn copy_same_size(src: *const u8, dst: *mut u8, total_bytes: usize) {
    unsafe { std::ptr::copy_nonoverlapping(src, dst, total_bytes); }
}

#[no_mangle]
pub extern "C" fn flare_copy_same_size(src: *const u8, dst: *mut u8, n: usize) {
    copy_same_size(src, dst, n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_opaque_all_opaque() {
        let src = vec![255u8; 4];
        assert_eq!(is_opaque_u8(src.as_ptr(), 4, 1, 1), 1);
    }
    #[test]
    fn is_opaque_not() {
        let src = vec![0u8, 0, 0, 100];
        assert_eq!(is_opaque_u8(src.as_ptr(), 4, 1, 1), 0);
    }
    #[test]
    fn swap_rb() {
        let mut buf = vec![10u8, 20, 30, 40]; // R=10? wait channel order
        // In BGRM: R offset=2, B offset=0. So buf[0]=B=10? Let's just check swap happens.
        let before_r = buf[R];
        let before_b = buf[B];
        swap_rb_channels(buf.as_mut_ptr(), 4, 1, 1);
        assert_eq!(buf[R], before_b);
        assert_eq!(buf[B], before_r);
    }
    #[test]
    fn gamma_correct_identity() {
        let mut buf = vec![100u8, 100, 100, 255];
        gamma_correct_u8(buf.as_mut_ptr(), 4, 1, 1, 1.0);
        assert_eq!(buf, vec![100, 100, 100, 255]);
    }
}