//! Port of `flarelib/sandor_fxs/SDirection.cpp`: the stroke-direction / radius map used
//! by the calligraphic and pattern-map effects.
//!
//! `CSDirection` keeps its byte map (`m_dir`) in C++ and its public interface
//! (`doDir`, `doRadius`, `getResult`); the three heavy steps forward here as in-place
//! passes over that buffer:
//!
//! - [`prepare`] = the constructor's `setDir01` + optional `setContourBorder`;
//! - [`do_dir`] = `doDir` (`makeDir` with the 4-direction filter, then `equalizeDir`);
//! - [`do_radius`] = `doRadius` (angle -> radius lookup, then `blurRadius`).
//!
//! Faithfully preserved quirks of the C++ (found by reading it, pinned by the golden):
//! - **`equalizeDir` only ever writes `m_dir[0]`.** Its output pointer is never advanced
//!   in the loop, so the equalised angles are discarded except for the last pixel's value,
//!   which lands in element 0. `doDir` therefore returns the raw `makeDir` angles plus that
//!   one overwrite. This is almost certainly a bug upstream, but "fixing" it would change
//!   every rendered result, so it is reproduced; [`do_dir`] evaluates only the one pixel
//!   whose value survives.
//! - Direction sums are `short` and wrap (`sum[j] += (short)v * w`); `w == 0` is tested on
//!   the wrapped value.
//!
//! Deviations, all where the C++ was undefined or could not finish:
//! - The `(2*sens+1)^2` filter table is not materialised (the weights are computed on the
//!   fly), so `sens` only has to satisfy `sens <= MAX_SENS` (the C++ would exhaust memory);
//!   a negative `sens` (uninitialised filter in the C++) acts as 0.
//! - Box scans (`border`, `dBlur`) are clipped to the image, which is the same result
//!   without looping `dBlur^2` times for a huge radius.
//! - Float -> byte casts of NaN/out-of-range go through `i32` (x86 behaviour) rather than
//!   being undefined.

use std::os::raw::c_int;

pub const NBDIR: usize = 4;
/// Largest filter half-size accepted.
pub const MAX_SENS: i32 = 4096;

#[derive(Debug, PartialEq, Eq)]
pub enum DirError {
    Invalid,
    TooBig,
}

fn dims(lx: i32, ly: i32, len: usize) -> Result<usize, DirError> {
    let n = (lx as i64) * (ly as i64);
    if lx <= 0 || ly <= 0 || n > i32::MAX as i64 || (len as i64) < n {
        return Err(DirError::Invalid);
    }
    Ok(n as usize)
}

/// `I_ROUND` from `SDef.h` (round half away from zero; the macro's guard is constant).
fn i_round(x: f64) -> i32 {
    if x < 0.0 { (x - 0.5) as i32 } else { (x + 0.5) as i32 }
}

/// `UC_ROUND`: `(unsigned char)(x + 0.5F)`, via `int` as on x86.
fn uc_round(x: f64) -> u8 {
    ((x + 0.5) as i32) as u8
}

/// Inclusive clipped range of `c - d ..= c + d` within `0..len`.
fn clip(c: i32, d: i32, len: i32) -> (i32, i32) {
    let lo = (c as i64 - d as i64).max(0);
    let hi = (c as i64 + d as i64).min(len as i64 - 1);
    (lo as i32, hi as i32)
}

/// `setDir01` + `setContourBorder` (when `border > 0`), in place.
pub fn prepare(lx: i32, ly: i32, dir: &mut [u8], sens: i32, border: i32) -> Result<(), DirError> {
    let n = dims(lx, ly, dir.len())?;
    if sens > MAX_SENS {
        return Err(DirError::TooBig);
    }
    for v in dir[..n].iter_mut() {
        *v = u8::from(*v > 0);
    }
    if border > 0 {
        let mut i = 0usize;
        for y in 0..ly {
            for x in 0..lx {
                if dir[i] == 1 && !is_contour_border(lx, ly, dir, x, y, border) {
                    dir[i] = 2;
                }
                i += 1;
            }
        }
        for v in dir[..n].iter_mut() {
            if *v == 2 {
                *v = 0;
            }
        }
    }
    Ok(())
}

fn is_contour_border(lx: i32, ly: i32, dir: &[u8], xx: i32, yy: i32, border: i32) -> bool {
    let (y0, y1) = clip(yy, border, ly);
    let (x0, x1) = clip(xx, border, lx);
    for y in y0..=y1 {
        for x in x0..=x1 {
            if dir[(y * lx + x) as usize] == 0 {
                return true;
            }
        }
    }
    false
}

fn adjust_angle(sum: &[i16; 4], ima: usize, im45: usize, ip45: usize) -> f64 {
    let ma = sum[im45].max(sum[ip45]);
    if ma < 0 {
        return 0.0;
    }
    if (ma as f64) < sum[ima] as f64 / 10.0 {
        return 0.0;
    }
    if ((sum[im45] as i32 - sum[ip45] as i32).abs() as f64 / ma as f64) < 0.5 {
        return 0.0;
    }
    let d = 45.0 * ma as f64 / (sum[ima] as i32 + ma as i32) as f64;
    if ma == sum[im45] { -d } else { d }
}

fn get_angle(sum: &[i16; 4], ma: i16) -> f64 {
    let nb_max = sum.iter().filter(|&&s| s == ma).count();
    let diff = 50.0;
    if nb_max == 1 {
        let (angle, corr);
        if ma == sum[0] {
            let c = adjust_angle(sum, 0, 3, 1);
            angle = if c < 0.0 { 180.0 } else { 0.0 };
            corr = c;
        } else if ma == sum[1] {
            angle = 45.0;
            corr = adjust_angle(sum, 1, 0, 2);
        } else if ma == sum[2] {
            angle = 90.0;
            corr = adjust_angle(sum, 2, 1, 3);
        } else {
            angle = 135.0;
            corr = adjust_angle(sum, 3, 2, 0);
        }
        return angle + corr + diff;
    }
    if nb_max == 2 {
        if ma == sum[0] && ma == sum[1] {
            return 22.5 + diff;
        }
        if ma == sum[0] && ma == sum[3] {
            return 157.5 + diff;
        }
        if ma == sum[1] && ma == sum[2] {
            return 67.5 + diff;
        }
        if ma == sum[2] && ma == sum[3] {
            return 112.5 + diff;
        }
    }
    1.0
}

/// `getDir` with the filter weights computed on the fly (see module docs).
fn get_dir(lx: i32, ly: i32, sel: &[u8], xx: i32, yy: i32, sens: i32) -> u8 {
    let size = 2 * sens + 1;
    let middle = size / 2;
    let max_val = size - 1;
    let mut sum = [0i16; 4];
    let mut w: i16 = 0;
    let (fy0, fy1) = clip(yy, middle, ly); // image rows covered
    let (fx0, fx1) = clip(xx, middle, lx);
    for y in fy0..=fy1 {
        let fy = y - yy + middle;
        for x in fx0..=fx1 {
            let fx = x - xx + middle;
            let v = sel[(y * lx + x) as usize] as i32;
            let wt = [
                if fy == middle { max_val } else { -1 },
                if fx == fy { max_val } else { -1 },
                if fx == middle { max_val } else { -1 },
                if fx + fy == size - 1 { max_val } else { -1 },
            ];
            for j in 0..NBDIR {
                sum[j] = sum[j].wrapping_add(v.wrapping_mul(wt[j]) as i16);
            }
            w = w.wrapping_add(v as i16);
        }
    }
    if w == 0 {
        return 0;
    }
    let ma = *sum.iter().max().unwrap();
    uc_round(get_angle(&sum, ma))
}

fn equalize_gte50(lx: i32, ly: i32, sel: &[u8], xx: i32, yy: i32, d: i32) -> u8 {
    let mut sum = 0i32;
    let mut w = 0i32;
    let aa = sel[(yy * lx + xx) as usize] as i32 - 50;
    let (y0, y1) = clip(yy, d, ly);
    let (x0, x1) = clip(xx, d, lx);
    for y in y0..=y1 {
        for x in x0..=x1 {
            let mut a = sel[(y * lx + x) as usize] as i32;
            if a >= 50 {
                a -= 50;
                if aa < 90 && a > 135 {
                    a = -(180 - a);
                } else if aa > 90 && a < 45 {
                    a += 180;
                }
                sum += a;
                w += 1;
            }
        }
    }
    finish_equalize(sum, w, sel[(yy * lx + xx) as usize])
}

fn equalize_lt50(lx: i32, ly: i32, sel: &[u8], xx: i32, yy: i32, d: i32) -> u8 {
    let mut sum = 0i32;
    let mut w = 0i32;
    let (y0, y1) = clip(yy, d, ly);
    let (x0, x1) = clip(xx, d, lx);
    for y in y0..=y1 {
        for x in x0..=x1 {
            let a = sel[(y * lx + x) as usize] as i32;
            if a >= 50 {
                sum += a - 50;
                w += 1;
            }
        }
    }
    finish_equalize(sum, w, sel[(yy * lx + xx) as usize])
}

fn finish_equalize(sum: i32, w: i32, centre: u8) -> u8 {
    if w > 0 {
        let mut a = i_round(sum as f64 / w as f64);
        if a >= 180 {
            a -= 180;
        } else if a < 0 {
            a += 180;
        }
        return (a + 50) as u8;
    }
    centre
}

/// `doDir`, in place on the 0/1 map left by [`prepare`].
pub fn do_dir(lx: i32, ly: i32, dir: &mut [u8], sens: i32) -> Result<(), DirError> {
    let n = dims(lx, ly, dir.len())?;
    if sens > MAX_SENS {
        return Err(DirError::TooBig);
    }
    let sens = sens.max(0);
    let sel: Vec<u8> = dir[..n].to_vec();
    // makeDir
    for y in 0..ly {
        for x in 0..lx {
            let i = (y * lx + x) as usize;
            dir[i] = if sel[i] > 0 { get_dir(lx, ly, &sel, x, y, sens) } else { 0 };
        }
    }
    // equalizeDir(sel = copy of dir, d = 3): see module docs, only element 0 is written,
    // last pixel wins.
    let sel: Vec<u8> = dir[..n].to_vec();
    let (x, y) = (lx - 1, ly - 1);
    let last = sel[n - 1];
    dir[0] = if last > 0 {
        if last >= 50 { equalize_gte50(lx, ly, &sel, x, y, 3) } else { equalize_lt50(lx, ly, &sel, x, y, 3) }
    } else {
        0
    };
    Ok(())
}

fn get_radius(angle: f64, r: &[f64; 4]) -> u8 {
    let (q, p);
    if angle >= 0.0 && angle < 45.0 {
        q = angle / 45.0;
        p = r[0] * (1.0 - q) + r[1] * q;
    } else if angle >= 45.0 && angle < 90.0 {
        q = (angle - 45.0) / 45.0;
        p = r[1] * (1.0 - q) + r[2] * q;
    } else if angle >= 90.0 && angle < 135.0 {
        q = (angle - 90.0) / 45.0;
        p = r[2] * (1.0 - q) + r[3] * q;
    } else {
        q = (angle - 135.0) / 45.0;
        p = r[3] * (1.0 - q) + r[0] * q;
    }
    (1i32.wrapping_add(i_round(p * 254.0))) as u8
}

fn blur_radius_px(lx: i32, ly: i32, sel: &[u8], xx: i32, yy: i32, d_blur: i32) -> u8 {
    let (mut sum, mut w) = (0i64, 0i64);
    let (y0, y1) = clip(yy, d_blur, ly);
    let (x0, x1) = clip(xx, d_blur, lx);
    for y in y0..=y1 {
        for x in x0..=x1 {
            let v = sel[(y * lx + x) as usize];
            if v > 0 {
                sum += v as i64;
                w += 1;
            }
        }
    }
    if w > 0 {
        let d = (sum as f64 / w as f64).clamp(0.0, 255.0);
        return uc_round(d);
    }
    sel[(yy * lx + xx) as usize]
}

fn cut_0_1(x: f64) -> f64 {
    // D_CUT_0_1 lets NaN through
    if x < 0.0 { 0.0 } else if x > 1.0 { 1.0 } else { x }
}

/// `doRadius`, in place on the angle map left by [`do_dir`].
pub fn do_radius(lx: i32, ly: i32, dir: &mut [u8], rh: f64, rlr: f64, rv: f64, rrl: f64, d_blur: i32) -> Result<(), DirError> {
    let n = dims(lx, ly, dir.len())?;
    let r = [cut_0_1(rh), cut_0_1(rlr), cut_0_1(rv), cut_0_1(rrl)];
    for v in dir[..n].iter_mut() {
        *v = if *v < 50 { 0 } else { get_radius((*v as i32 - 50) as f64, &r) };
    }
    if d_blur > 0 {
        let sel: Vec<u8> = dir[..n].to_vec();
        let mut i = 0usize;
        for y in 0..ly {
            for x in 0..lx {
                if sel[i] > 0 {
                    dir[i] = blur_radius_px(lx, ly, &sel, x, y, d_blur);
                }
                i += 1;
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------
// C ABI. Status: 0 ok, -1 bad arguments, -2 request too big (the old SMemAllocError).
// ---------------------------------------------------------------------------------------

fn status(r: Result<(), DirError>) -> c_int {
    match r {
        Ok(()) => 0,
        Err(DirError::Invalid) => -1,
        Err(DirError::TooBig) => -2,
    }
}

unsafe fn buf<'a>(dir: *mut u8, lx: c_int, ly: c_int) -> Option<&'a mut [u8]> {
    if dir.is_null() || lx <= 0 || ly <= 0 {
        return None;
    }
    let n = (lx as i64).checked_mul(ly as i64)?;
    if n > i32::MAX as i64 {
        return None;
    }
    Some(std::slice::from_raw_parts_mut(dir, n as usize))
}

/// Constructor part of `CSDirection`: binarise `dir` (lx*ly bytes) in place, apply the
/// contour border when `border > 0`, validate `sens`.
#[no_mangle]
pub unsafe extern "C" fn flare_sdirection_prepare(lx: c_int, ly: c_int, dir: *mut u8, sens: c_int, border: c_int) -> c_int {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match buf(dir, lx, ly) {
        Some(d) => status(prepare(lx, ly, d, sens, border)),
        None => -1,
    }))
    .unwrap_or(-1)
}

/// `CSDirection::doDir` on the map left by `flare_sdirection_prepare`.
#[no_mangle]
pub unsafe extern "C" fn flare_sdirection_do_dir(lx: c_int, ly: c_int, dir: *mut u8, sens: c_int) -> c_int {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match buf(dir, lx, ly) {
        Some(d) => status(do_dir(lx, ly, d, sens)),
        None => -1,
    }))
    .unwrap_or(-1)
}

/// `CSDirection::doRadius`.
#[no_mangle]
pub unsafe extern "C" fn flare_sdirection_do_radius(
    lx: c_int,
    ly: c_int,
    dir: *mut u8,
    r_h: f64,
    r_lr: f64,
    r_v: f64,
    r_rl: f64,
    d_blur: c_int,
) -> c_int {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match buf(dir, lx, ly) {
        Some(d) => status(do_radius(lx, ly, d, r_h, r_lr, r_v, r_rl, d_blur)),
        None => -1,
    }))
    .unwrap_or(-1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unhex(s: &str) -> Vec<u8> {
        (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap()).collect()
    }
    fn hex(b: &[u8]) -> String {
        b.iter().map(|v| format!("{v:02x}")).collect()
    }
    fn f(s: &str) -> f64 {
        f64::from_bits(u64::from_str_radix(s, 16).unwrap())
    }

    #[test]
    fn matches_the_cpp_reference_exactly() {
        let text = include_str!("SDirection_cases.txt").replace("\r\n", "\n");
        let mut n = 0;
        for block in text.split("\n\n").filter(|b| !b.trim().is_empty()) {
            let mut lines = block.lines();
            let h: Vec<&str> = lines.next().unwrap().split_whitespace().collect();
            // C lx ly sens border mode rH rLR rV rRL dBlur
            assert_eq!(h[0], "C");
            let lx: i32 = h[1].parse().unwrap();
            let ly: i32 = h[2].parse().unwrap();
            let sens: i32 = h[3].parse().unwrap();
            let border: i32 = h[4].parse().unwrap();
            let mode: i32 = h[5].parse().unwrap();
            let (rh, rlr, rv, rrl) = (f(h[6]), f(h[7]), f(h[8]), f(h[9]));
            let d_blur: i32 = h[10].parse().unwrap();
            let mut d = unhex(lines.next().unwrap().strip_prefix("M ").unwrap());
            let want = lines.next().unwrap().strip_prefix("O ").unwrap();
            prepare(lx, ly, &mut d, sens, border).unwrap();
            if mode & 1 != 0 {
                do_dir(lx, ly, &mut d, sens).unwrap();
            }
            if mode & 2 != 0 {
                do_radius(lx, ly, &mut d, rh, rlr, rv, rrl, d_blur).unwrap();
            }
            assert_eq!(hex(&d), want, "case {n}: {}", block.lines().next().unwrap());
            n += 1;
        }
        assert!(n > 100, "golden looks empty: {n} cases");
    }

    #[test]
    fn border_keeps_only_the_ring_next_to_background() {
        // 5x5 block inside a 7x7 map, border 1: the block's outer ring survives, its
        // 3x3 interior (no zero within one pixel) is cleared. Image edges count as
        // "not background", so a block touching the edge would lose its edge pixels.
        let mut d = vec![0u8; 49];
        for y in 1..6 {
            for x in 1..6 {
                d[y * 7 + x] = 200;
            }
        }
        prepare(7, 7, &mut d, 1, 1).unwrap();
        for y in 0..7 {
            for x in 0..7 {
                let in_block = (1..6).contains(&x) && (1..6).contains(&y);
                let ring = in_block && (x == 1 || y == 1 || x == 5 || y == 5);
                assert_eq!(d[y * 7 + x], u8::from(ring), "({x},{y})");
            }
        }
        let mut full = vec![9u8; 25];
        prepare(5, 5, &mut full, 1, 1).unwrap();
        assert!(full.iter().all(|&v| v == 0));
    }

    #[test]
    fn huge_box_radii_do_not_hang_or_overflow() {
        let mut d = vec![1u8; 16];
        prepare(4, 4, &mut d, 1, i32::MAX).unwrap();
        do_dir(4, 4, &mut d, 1).unwrap();
        do_radius(4, 4, &mut d, 0.5, 0.5, 0.5, 0.5, i32::MAX).unwrap();
    }

    #[test]
    fn size_and_argument_limits() {
        let mut d = vec![1u8; 4];
        assert_eq!(prepare(2, 2, &mut d, MAX_SENS + 1, 0), Err(DirError::TooBig));
        assert_eq!(prepare(0, 2, &mut d, 1, 0), Err(DirError::Invalid));
        assert_eq!(prepare(3, 3, &mut d, 1, 0), Err(DirError::Invalid)); // short buffer
        assert_eq!(do_dir(2, 2, &mut d, MAX_SENS + 1), Err(DirError::TooBig));
        assert!(do_dir(2, 2, &mut d, -5).is_ok());
    }

    #[test]
    fn ffi_null_checks() {
        unsafe {
            let mut d = [1u8; 4];
            assert_eq!(flare_sdirection_prepare(2, 2, std::ptr::null_mut(), 1, 0), -1);
            assert_eq!(flare_sdirection_prepare(-1, 2, d.as_mut_ptr(), 1, 0), -1);
            assert_eq!(flare_sdirection_prepare(i32::MAX, i32::MAX, d.as_mut_ptr(), 1, 0), -1);
            assert_eq!(flare_sdirection_prepare(2, 2, d.as_mut_ptr(), 1 << 20, 0), -2);
            assert_eq!(flare_sdirection_prepare(2, 2, d.as_mut_ptr(), 1, 0), 0);
            assert_eq!(flare_sdirection_do_dir(2, 2, std::ptr::null_mut(), 1), -1);
            assert_eq!(flare_sdirection_do_dir(2, 2, d.as_mut_ptr(), 1), 0);
            assert_eq!(flare_sdirection_do_radius(2, 2, std::ptr::null_mut(), 0.0, 0.0, 0.0, 0.0, 0), -1);
            assert_eq!(flare_sdirection_do_radius(2, 2, d.as_mut_ptr(), 0.0, 0.5, 1.0, 0.5, 1), 0);
        }
    }
}
