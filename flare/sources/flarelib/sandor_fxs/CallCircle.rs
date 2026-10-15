//! Port of the sample-list construction of flarelib/sandor_fxs/CallCircle.cpp:
//! every integer offset (x, y) with distance w = hypot <= r, sorted by w.
//!
//! The C++ used `qsort` (comparing only `w`), so the order of equal-distance
//! offsets was whatever the libc sort produced (glibc's qsort was a merge sort,
//! effectively stable; MSVC/macOS qsort are unstable and may order ties
//! differently). The order is now DEFINED: a stable sort over the row-major
//! generation order (y ascending, then x ascending). It matches glibc but may
//! differ from old MSVC/macOS tie order; only equal-distance offsets are
//! affected (draw uses max(), so output is order-insensitive in practice).
//! The class itself (drawing / template colour code) stays in `CallCircle.h`.

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sxydw {
    pub x: i32,
    pub y: i32,
    pub w: f64,
}

/// Number of cells the C++ allocated: dd*dd with dd = 4*((int)r+1)+1. None if it
/// would overflow `i32` (the C++ overflowed `int` there) or r is not usable.
pub fn capacity(r: f64) -> Option<usize> {
    if !r.is_finite() || r < 0.0 || r > 5000.0 {
        return None;
    }
    let rr = (r as i32 + 1) * 2;
    let dd = (rr * 2 + 1) as usize;
    Some(dd * dd)
}

pub fn build(r: f64) -> Vec<Sxydw> {
    let Some(cap) = capacity(r) else { return Vec::new() };
    let rr = (r as i32 + 1) * 2;
    let mut v = Vec::new();
    for y in -rr..=rr {
        for x in -rr..=rr {
            let d = ((x * x + y * y) as f64).sqrt();
            if d <= r && v.len() < cap {
                v.push(Sxydw { x, y, w: d });
            }
        }
    }
    // qsort compare: only `w`; NaN never occurs.
    v.sort_by(|a, b| a.w.partial_cmp(&b.w).unwrap_or(std::cmp::Ordering::Equal));
    v
}

/// Fills `out` (room for `cap` entries, normally `flare_callcircle_capacity(r)`)
/// and returns the entry count, or -1 on bad arguments.
#[no_mangle]
pub unsafe extern "C" fn flare_callcircle_build(r: f64, out: *mut Sxydw, cap: i32) -> i32 {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if out.is_null() || cap < 0 {
            return -1;
        }
        let v = build(r);
        if v.len() > cap as usize {
            return -1;
        }
        std::ptr::copy_nonoverlapping(v.as_ptr(), out, v.len());
        v.len() as i32
    }))
    .unwrap_or(-1)
}

/// Allocation size (in entries) the C++ constructor used, or 0 when `r` is unusable.
#[no_mangle]
pub extern "C" fn flare_callcircle_capacity(r: f64) -> i32 {
    match capacity(r) {
        Some(c) if c <= i32::MAX as usize => c as i32,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn radius_one() {
        let v = build(1.0);
        assert_eq!(v.len(), 5);
        assert_eq!(v[0], Sxydw { x: 0, y: 0, w: 0.0 });
        assert!(v[1..].iter().all(|s| s.w == 1.0));
    }
    #[test]
    fn radius_one_tie_order_is_row_major() {
        // stable sort of row-major (y, then x) generation order
        let xy: Vec<(i32, i32)> = build(1.0).iter().map(|s| (s.x, s.y)).collect();
        assert_eq!(xy, vec![(0, 0), (0, -1), (-1, 0), (1, 0), (0, 1)]);
    }
    #[test]
    fn sorted_and_inside() {
        let v = build(7.3);
        assert!(v.windows(2).all(|p| p[0].w <= p[1].w));
        assert!(v.iter().all(|s| s.w <= 7.3));
        // count of lattice points in the disc
        let n = (-9..=9).flat_map(|y: i32| (-9..=9).map(move |x: i32| (x, y))).filter(|(x, y)| ((x * x + y * y) as f64).sqrt() <= 7.3).count();
        assert_eq!(v.len(), n);
    }
    #[test]
    fn bad_radius() {
        assert!(build(f64::NAN).is_empty());
        assert!(build(-1.0).is_empty());
        assert_eq!(flare_callcircle_capacity(1e9), 0);
        unsafe { assert_eq!(flare_callcircle_build(1.0, std::ptr::null_mut(), 5), -1) }
    }
    #[test]
    fn ffi_fill() {
        let cap = flare_callcircle_capacity(2.0);
        let mut buf = vec![Sxydw { x: 0, y: 0, w: 0.0 }; cap as usize];
        let n = unsafe { flare_callcircle_build(2.0, buf.as_mut_ptr(), cap) };
        assert_eq!(n as usize, build(2.0).len());
    }
}
