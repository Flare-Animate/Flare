//! Port of `flarelib/sandor_fxs/PatternPosition.cpp`: where the pattern-map effect drops
//! its stamps. Two modes, chosen by the (header-resident) constructor template:
//!
//! - **random** (`density > 0.00001`): every selected pixel is kept with probability
//!   `nbPat / nbPixel`, by comparing `rand()` against `RAND_MAX * nbPat / nbPixel`;
//! - **distance-driven** (`density == 0`): scan for the first free selected pixel, stamp
//!   it, and mark a disc around it (radius picked at random out of 20 between `minD` and
//!   `maxD`) as used.
//!
//! `CPatternPosition` keeps its public `m_pos` and its template constructor in
//! `PatternPosition.h`; `makeRandomPositions` / `makeDDPositions` forward here.
//!
//! Notes:
//! - **The RNG stays with the caller.** The C++ called the C runtime's `rand()` and read
//!   `RAND_MAX`; both are arguments now (the header passes `::rand` and `RAND_MAX`), so the
//!   same sequence is reproduced on any platform and the Rust side is testable with a
//!   deterministic generator.
//! - Positions are returned as a Rust-allocated `[x0, y0, x1, y1, ...]` block that the
//!   caller frees with `flare_pattern_position_free`.
//! - `SMemAllocError` is mapped to a negative status; the header rethrows it. The C++ only
//!   threw it on `std::bad_alloc`. Deviations, all where the C++ never finished or was UB:
//!   a distance circle that cannot clear the pixel it just stamped (empty disc, e.g. NaN
//!   or a radius <= -2; detected dynamically as `NoProgress`) made the original stamp the
//!   same pixel until memory ran out, so that now returns an error immediately; radii whose disc would exceed `MAX_RADIUS_CELLS` per side
//!   (an unreasonable ~8k-pixel stamp) are refused instead of looping for hours;
//!   `lX * lY` overflowing `int` is refused.
//! - Private helpers that nothing called (`isInSet`, `getPosAroundThis`, which was only
//!   referenced from a commented-out line) are not ported; `isInSet` stays inline.

use std::os::raw::c_int;

const MAXNBDDC: usize = 20;
/// Largest `(int)r + 1` accepted for a distance disc.
pub const MAX_RADIUS_CELLS: i32 = 1024;

/// `SRECT` from `SDef.h`.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Rect {
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
}

#[derive(Debug, PartialEq, Eq)]
pub enum PosError {
    /// Bad or unusable arguments (null buffers, overflow, an unreasonable radius).
    Invalid,
    /// The stamp loop cannot make progress (the C++ ran until `bad_alloc`).
    NoProgress,
}

fn area(lx: i32, ly: i32) -> Result<usize, PosError> {
    if lx <= 0 || ly <= 0 {
        return Ok(0);
    }
    lx.checked_mul(ly).map(|v| v as usize).ok_or(PosError::Invalid)
}

/// `makeRandomPositions`. `rand_max` is the C runtime's `RAND_MAX`.
pub fn random_positions(
    nb_pat: i32,
    nb_pixel: i32,
    lx: i32,
    ly: i32,
    sel: &[u8],
    rand_max: i32,
    rng: &mut dyn FnMut() -> i32,
) -> Result<Vec<(i32, i32)>, PosError> {
    let n = area(lx, ly)?;
    if sel.len() < n {
        return Err(PosError::Invalid);
    }
    // int threshold = RAND_MAX * (double)nbPat / (double)nbPixel;
    let threshold = (rand_max as f64 * nb_pat as f64 / nb_pixel as f64) as i32;
    let mut pos = Vec::new();
    let mut i = 0usize;
    for y in 0..ly.max(0) {
        for x in 0..lx.max(0) {
            if sel[i] > 0 && rng() < threshold {
                pos.push((x, y));
            }
            i += 1;
        }
    }
    Ok(pos)
}

/// `prepareCircle`: lattice offsets inside radius `r`, y-major.
fn prepare_circle(r: f64) -> Result<Vec<(i32, i32)>, PosError> {
    let r2 = r * r;
    let rr = (r as i32).saturating_add(1);
    if rr > MAX_RADIUS_CELLS {
        return Err(PosError::Invalid);
    }
    let mut v = Vec::new();
    for y in -rr..=rr {
        for x in -rr..=rr {
            if ((x * x + y * y) as f64) <= r2 {
                v.push((x, y));
            }
        }
    }
    Ok(v)
}

/// `sel0255To01`: binarise in place and return the bounding box of the set pixels.
fn sel_0255_to_01(lx: i32, ly: i32, sel: &mut [u8]) -> Rect {
    let mut bb = Rect { x0: lx, y0: ly, x1: -1, y1: -1 };
    let mut i = 0usize;
    for y in 0..ly {
        for x in 0..lx {
            if sel[i] >= 1 {
                sel[i] = 1;
                bb.x0 = bb.x0.min(x);
                bb.x1 = bb.x1.max(x);
                bb.y0 = bb.y0.min(y);
                bb.y1 = bb.y1.max(y);
            }
            i += 1;
        }
    }
    bb
}

/// `findEmptyPos`, including its quirk: the first scan restarts at `x_pos` on *every* row
/// up to and including `y_pos`, and only the second scan covers the whole box.
fn find_empty_pos(lx: i32, sel: &[u8], x_pos: &mut i32, y_pos: &mut i32, bb: &Rect) -> bool {
    for y in 0..=*y_pos {
        for x in *x_pos..=bb.x1 {
            if sel[(y * lx + x) as usize] == 1 {
                *x_pos = x;
                *y_pos = y;
                return true;
            }
        }
    }
    for y in *y_pos..=bb.y1 {
        for x in bb.x0..=bb.x1 {
            if sel[(y * lx + x) as usize] == 1 {
                *x_pos = x;
                *y_pos = y;
                return true;
            }
        }
    }
    false
}

/// `eraseCurrentArea`: 1 -> 2 under the disc.
fn erase_current_area(lx: i32, ly: i32, sel: &mut [u8], ddc: &[(i32, i32)], xx: i32, yy: i32) {
    for &(dx, dy) in ddc {
        let x = xx.wrapping_add(dx);
        let y = yy.wrapping_add(dy);
        if x >= 0 && y >= 0 && x < lx && y < ly {
            let p = &mut sel[(y * lx + x) as usize];
            if *p == 1 {
                *p = 2;
            }
        }
    }
}

/// `makeDDPositions`. An empty result is returned for the C++'s early-outs (no pixels,
/// `minD > maxD`, empty selection).
pub fn dd_positions(
    lx: i32,
    ly: i32,
    sel: &[u8],
    min_d: f64,
    max_d: f64,
    rng: &mut dyn FnMut() -> i32,
) -> Result<Vec<(i32, i32)>, PosError> {
    if lx <= 0 || ly <= 0 {
        return Ok(Vec::new());
    }
    if min_d > max_d {
        return Ok(Vec::new());
    }
    let n = area(lx, ly)?;
    if sel.len() < n {
        return Err(PosError::Invalid);
    }
    let nb_ddc = if (max_d - min_d).abs() < 0.001 { 1 } else { MAXNBDDC };
    let mut ddc: Vec<Vec<(i32, i32)>> = Vec::with_capacity(nb_ddc);
    if nb_ddc == 1 {
        ddc.push(prepare_circle(min_d)?);
    } else {
        let d_dist = (max_d - min_d) / (nb_ddc - 1) as f64;
        let mut dist = min_d;
        for _ in 0..nb_ddc {
            ddc.push(prepare_circle(dist)?);
            dist += d_dist;
        }
    }
    let mut l_sel = sel[..n].to_vec();
    let bb = sel_0255_to_01(lx, ly, &mut l_sel);
    if bb.x0 > bb.x1 || bb.y0 > bb.y1 {
        return Ok(Vec::new());
    }
    let mut pos = Vec::new();
    let (mut x, mut y) = (0, 0);
    while find_empty_pos(lx, &l_sel, &mut x, &mut y, &bb) {
        pos.push((x, y));
        let iddc = if nb_ddc == 1 { 0 } else { (rng() as i64).rem_euclid(nb_ddc as i64) as usize };
        erase_current_area(lx, ly, &mut l_sel, &ddc[iddc], x, y);
        if l_sel[(y * lx + x) as usize] == 1 {
            return Err(PosError::NoProgress);
        }
    }
    Ok(pos)
}

// ---------------------------------------------------------------------------------------
// C ABI
// ---------------------------------------------------------------------------------------

type RandFn = extern "C" fn() -> c_int;

const ERR_ARGS: c_int = -1;
const ERR_FAILED: c_int = -2;

unsafe fn publish(res: Result<Vec<(i32, i32)>, PosError>, out: *mut *mut i32) -> c_int {
    match res {
        Ok(v) if v.is_empty() => {
            *out = std::ptr::null_mut();
            0
        }
        Ok(v) => {
            let n = v.len();
            if n > (c_int::MAX / 2) as usize {
                return ERR_FAILED;
            }
            let flat: Vec<i32> = v.iter().flat_map(|&(x, y)| [x, y]).collect();
            *out = Box::into_raw(flat.into_boxed_slice()) as *mut i32;
            n as c_int
        }
        Err(PosError::Invalid) => ERR_ARGS,
        Err(PosError::NoProgress) => ERR_FAILED,
    }
}

/// `CPatternPosition::makeRandomPositions`. Returns the number of positions (>= 0) and
/// stores a block of `2 * n` ints in `*out` (null when n == 0; free with
/// `flare_pattern_position_free`), or a negative status: -1 bad arguments, -2 failure.
#[no_mangle]
pub unsafe extern "C" fn flare_pattern_position_random(
    nb_pat: c_int,
    nb_pixel: c_int,
    lx: c_int,
    ly: c_int,
    sel: *const u8,
    randfn: Option<RandFn>,
    rand_max: c_int,
    out: *mut *mut i32,
) -> c_int {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if out.is_null() {
            return ERR_ARGS;
        }
        *out = std::ptr::null_mut();
        let Some(rf) = randfn else { return ERR_ARGS };
        let n = match area(lx, ly) {
            Ok(n) => n,
            Err(_) => return ERR_ARGS,
        };
        if n > 0 && sel.is_null() {
            return ERR_ARGS;
        }
        let sel_s: &[u8] = if n == 0 { &[] } else { std::slice::from_raw_parts(sel, n) };
        publish(random_positions(nb_pat, nb_pixel, lx, ly, sel_s, rand_max, &mut || rf()), out)
    }))
    .unwrap_or(ERR_FAILED)
}

/// `CPatternPosition::makeDDPositions` (same return convention). A null `sel` or empty
/// image yields 0 positions, as in the C++.
#[no_mangle]
pub unsafe extern "C" fn flare_pattern_position_dd(
    lx: c_int,
    ly: c_int,
    sel: *const u8,
    min_d: f64,
    max_d: f64,
    randfn: Option<RandFn>,
    out: *mut *mut i32,
) -> c_int {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if out.is_null() {
            return ERR_ARGS;
        }
        *out = std::ptr::null_mut();
        if lx <= 0 || ly <= 0 || sel.is_null() {
            return 0;
        }
        let Some(rf) = randfn else { return ERR_ARGS };
        let n = match area(lx, ly) {
            Ok(n) => n,
            Err(_) => return ERR_ARGS,
        };
        let sel_s = std::slice::from_raw_parts(sel, n);
        publish(dd_positions(lx, ly, sel_s, min_d, max_d, &mut || rf()), out)
    }))
    .unwrap_or(ERR_FAILED)
}

/// Frees a block returned by the functions above (`n` is the count they returned).
#[no_mangle]
pub unsafe extern "C" fn flare_pattern_position_free(p: *mut i32, n: c_int) {
    if p.is_null() || n <= 0 {
        return;
    }
    let _ = std::panic::catch_unwind(|| {
        drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(p, 2 * n as usize)));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Deterministic 31-bit generator; must match `ref_rand` in
    /// tests/native/patternposition_ref.cpp.
    pub struct RefRand(u64);
    impl RefRand {
        pub fn new(seed: u64) -> Self {
            RefRand(seed)
        }
        pub fn next(&mut self) -> i32 {
            self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (self.0 >> 33) as i32
        }
    }
    const RAND_MAX: i32 = 2147483647;

    fn i_round(x: f64) -> i32 {
        if x < 0.0 { (x - 0.5) as i32 } else { (x + 0.5) as i32 }
    }

    /// The header-resident template constructor of `CPatternPosition`.
    fn construct(lx: i32, ly: i32, sel: &[u8], nb_pixel: i32, dens: f64, min_d: f64, max_d: f64, seed: u64) -> Vec<(i32, i32)> {
        let mut r = RefRand::new(seed);
        if dens > 0.00001 {
            let mut nb_pat = i_round(nb_pixel as f64 * dens);
            nb_pat = nb_pat.min(nb_pixel);
            if nb_pat > 0 {
                return random_positions(nb_pat, nb_pixel, lx, ly, sel, RAND_MAX, &mut || r.next()).unwrap();
            }
            Vec::new()
        } else {
            dd_positions(lx, ly, sel, min_d, max_d, &mut || r.next()).unwrap()
        }
    }

    #[test]
    fn matches_the_cpp_reference_exactly() {
        let text = include_str!("PatternPosition_cases.txt").replace("\r\n", "\n");
        let mut n = 0;
        for block in text.split("\n\n").filter(|b| !b.trim().is_empty()) {
            let mut lines = block.lines();
            let h: Vec<&str> = lines.next().unwrap().split_whitespace().collect();
            assert_eq!(h[0], "C");
            let lx: i32 = h[1].parse().unwrap();
            let ly: i32 = h[2].parse().unwrap();
            let dens = f64::from_bits(u64::from_str_radix(h[3], 16).unwrap());
            let min_d = f64::from_bits(u64::from_str_radix(h[4], 16).unwrap());
            let max_d = f64::from_bits(u64::from_str_radix(h[5], 16).unwrap());
            let nb_pixel: i32 = h[6].parse().unwrap();
            let seed: u64 = h[7].parse().unwrap();
            let mask_hex = lines.next().unwrap().strip_prefix("M ").unwrap();
            let sel: Vec<u8> = (0..mask_hex.len() / 2).map(|i| u8::from_str_radix(&mask_hex[2 * i..2 * i + 2], 16).unwrap()).collect();
            assert_eq!(sel.len(), (lx * ly) as usize);
            let want = lines.next().unwrap();
            let got = construct(lx, ly, &sel, nb_pixel, dens, min_d, max_d, seed);
            let mut s = format!("O {}", got.len());
            for (x, y) in &got {
                s.push_str(&format!(" {x} {y}"));
            }
            assert_eq!(s, want, "case {n}: {}", block.lines().next().unwrap());
            n += 1;
        }
        assert!(n > 100, "golden looks empty: {n} cases");
    }

    #[test]
    fn early_outs() {
        let mut r = RefRand::new(1);
        let sel = [1u8; 4];
        assert!(dd_positions(0, 2, &sel, 1.0, 1.0, &mut || r.next()).unwrap().is_empty());
        assert!(dd_positions(2, 2, &sel, 3.0, 2.0, &mut || r.next()).unwrap().is_empty());
        assert!(dd_positions(2, 2, &[0u8; 4], 1.0, 1.0, &mut || r.next()).unwrap().is_empty());
    }

    #[test]
    fn degenerate_circles_error_instead_of_looping() {
        let mut r = RefRand::new(1);
        let sel = [1u8; 4];
        assert_eq!(dd_positions(2, 2, &sel, f64::NAN, f64::NAN, &mut || r.next()), Err(PosError::NoProgress));
        assert_eq!(dd_positions(2, 2, &sel, -3.0, -3.0, &mut || r.next()), Err(PosError::NoProgress));
        assert_eq!(dd_positions(2, 2, &sel, 1.0e9, 1.0e9, &mut || r.next()), Err(PosError::Invalid));
    }

    #[test]
    fn short_selection_is_refused() {
        let mut r = RefRand::new(1);
        assert_eq!(random_positions(1, 4, 2, 2, &[1, 1], RAND_MAX, &mut || r.next()), Err(PosError::Invalid));
    }

    extern "C" fn counter() -> c_int {
        use std::sync::atomic::{AtomicI32, Ordering};
        static N: AtomicI32 = AtomicI32::new(0);
        N.fetch_add(1000, Ordering::Relaxed) % 100000
    }

    #[test]
    fn ffi_roundtrip_and_null_checks() {
        unsafe {
            let sel = [1u8; 9];
            let mut out: *mut i32 = std::ptr::null_mut();
            let n = flare_pattern_position_dd(3, 3, sel.as_ptr(), 1.0, 1.0, Some(counter), &mut out);
            assert!(n > 0 && !out.is_null());
            let first = (*out.as_ref().unwrap(), *out.add(1).as_ref().unwrap());
            assert_eq!(first, (0, 0));
            flare_pattern_position_free(out, n);
            assert_eq!(flare_pattern_position_dd(3, 3, std::ptr::null(), 1.0, 1.0, Some(counter), &mut out), 0);
            assert!(out.is_null());
            assert_eq!(flare_pattern_position_dd(3, 3, sel.as_ptr(), 1.0, 1.0, Some(counter), std::ptr::null_mut()), ERR_ARGS);
            assert_eq!(flare_pattern_position_random(1, 9, 3, 3, sel.as_ptr(), None, RAND_MAX, &mut out), ERR_ARGS);
            assert_eq!(flare_pattern_position_random(1, 9, 3, 3, std::ptr::null(), Some(counter), RAND_MAX, &mut out), ERR_ARGS);
            assert_eq!(flare_pattern_position_random(1, 9, 3, 3, sel.as_ptr(), Some(counter), RAND_MAX, std::ptr::null_mut()), ERR_ARGS);
            assert_eq!(flare_pattern_position_random(1, 9, i32::MAX, 2, sel.as_ptr(), Some(counter), RAND_MAX, &mut out), ERR_ARGS);
            flare_pattern_position_free(std::ptr::null_mut(), 5);
        }
    }
}
