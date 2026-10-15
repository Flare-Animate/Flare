//! Port of flare/predict3d.cpp: predict the image position of hidden points of a
//! planar 3D object from the visible ones (homography fit by least squares via
//! `metnum`). One whole-array call; same operation order as the C++.

use crate::metnum;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

/// Fill `current[i]` for every `!visible[i]`. False on <4 visible points or a
/// singular system (same conditions as the C++).
pub fn predict(initial: &[Point], current: &mut [Point], visible: &[bool]) -> bool {
    let k = initial.len();
    if current.len() < k || visible.len() < k {
        return false;
    }
    let kvis = visible[..k].iter().filter(|v| **v).count();
    if kvis < 4 {
        return false;
    }
    let n = 3 * kvis + 1;
    let m = kvis + 9;
    let mut x = vec![0.0f64; m * n];
    let mut y = vec![0.0f64; n];
    y[n - 1] = 1.0;
    let mut ii = 0;
    for i in 0..k {
        if !visible[i] {
            continue;
        }
        let j = 3 * ii;
        let (ix, iy) = (initial[i].x, initial[i].y);
        for (r, v) in [(0, -ix), (1, -iy), (2, -1.0)] {
            x[r * n + j] = v;
            x[(r + 3) * n + j + 1] = v;
            x[(r + 6) * n + j + 2] = v;
        }
        let l = ii + 9;
        x[l * n + j] = current[i].x;
        x[l * n + j + 1] = current[i].y;
        x[l * n + j + 2] = 1.0;
        ii += 1;
    }
    x[9 * n + n - 1] = 1.0;
    let Ok(c) = metnum::approx(n as i32, m as i32, &x, &y) else { return false };
    for i in 0..k {
        if visible[i] {
            continue;
        }
        let (px, py) = (initial[i].x, initial[i].y);
        let uw1 = px * c[0] + py * c[1] + c[2];
        let vw1 = px * c[3] + py * c[4] + c[5];
        let w1 = px * c[6] + py * c[7] + c[8];
        current[i].x = uw1 / w1;
        current[i].y = vw1 / w1;
    }
    true
}

/// `Predict3D::Predict(k, initial, current, visible)`; `visible` is C++ `bool[]`
/// (one byte each, any non-zero = true).
#[no_mangle]
pub unsafe extern "C" fn flare_predict3d(k: i32, initial: *const Point, current: *mut Point, visible: *const u8) -> bool {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if k < 4 || initial.is_null() || current.is_null() || visible.is_null() {
            return false;
        }
        let k = k as usize;
        let init = std::slice::from_raw_parts(initial, k);
        let cur = std::slice::from_raw_parts_mut(current, k);
        let vis: Vec<bool> = std::slice::from_raw_parts(visible, k).iter().map(|b| *b != 0).collect();
        predict(init, cur, &vis)
    }))
    .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn p(x: f64, y: f64) -> Point {
        Point { x, y }
    }
    #[test]
    fn translation_predicted() {
        let init = [p(0., 0.), p(10., 0.), p(10., 10.), p(0., 10.), p(5., 5.), p(2., 7.)];
        let mut cur: Vec<Point> = init.iter().map(|q| p(q.x + 3., q.y - 2.)).collect();
        cur[4] = p(f64::NAN, f64::NAN);
        cur[5] = p(0., 0.);
        let vis = [true, true, true, true, false, false];
        assert!(predict(&init, &mut cur, &vis));
        assert!((cur[4].x - 8.).abs() < 1e-6 && (cur[4].y - 3.).abs() < 1e-6);
        assert!((cur[5].x - 5.).abs() < 1e-6 && (cur[5].y - 5.).abs() < 1e-6);
    }
    #[test]
    fn too_few_visible() {
        let init = [p(0., 0.); 5];
        let mut cur = init;
        assert!(!predict(&init, &mut cur, &[true, true, true, false, false]));
    }
    #[test]
    fn collinear_fails() {
        let init = [p(0., 0.), p(1., 1.), p(2., 2.), p(3., 3.), p(4., 4.)];
        let mut cur = init;
        assert!(!predict(&init, &mut cur, &[true, true, true, true, false]));
    }
    #[test]
    fn ffi_nulls() {
        unsafe {
            assert!(!flare_predict3d(5, std::ptr::null(), std::ptr::null_mut(), std::ptr::null()));
        }
    }
}
