//! Port of common/tparam/tcubicbezier.cpp: x -> parameter / y lookups on a cubic
//! Bezier segment described by two points and their tangent ("speed") vectors.
//! Points are passed as scalar (x, y) pairs over the C ABI.

use crate::tmathutil::{cubic_root, EPSILON};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointD {
    pub x: f64,
    pub y: f64,
}

fn pd(x: f64, y: f64) -> PointD {
    PointD { x, y }
}

fn tcrop(v: f64, lo: f64, hi: f64) -> f64 {
    if v < lo {
        lo
    } else if v > hi {
        hi
    } else {
        v
    }
}

pub fn inv_cubic_bezier_x(x: f64, a: PointD, a_speed: PointD, b_speed: PointD, b: PointD) -> f64 {
    let mut a_speed_x = a_speed.x;
    let mut b_speed_x = b_speed.x;
    if a_speed_x == 0.0 {
        a_speed_x = EPSILON;
    }
    if b_speed_x == 0.0 {
        b_speed_x = -EPSILON;
    }

    // a*u^3+b*u^2+c*u+d
    let x0 = a.x;
    let x1 = x0 + a_speed_x;
    let x3 = b.x;
    let x2 = x3 + b_speed_x;

    let a_x = x3 + 3.0 * (x1 - x2) - x0;
    let b_x = 3.0 * (x2 - 2.0 * x1 + x0);
    let c_x = 3.0 * (x1 - x0);
    let d_x = x0 - x;

    cubic_root(a_x, b_x, c_x, d_x)
}

pub fn get_cubic_bezier_y(x: f64, a: PointD, a_speed: PointD, b_speed: PointD, b: PointD) -> f64 {
    let y0 = a.y;
    let y1 = y0 + a_speed.y;
    let y3 = b.y;
    let y2 = y3 + b_speed.y;

    let a_y = y3 + 3.0 * (y1 - y2) - y0;
    let b_y = 3.0 * (y2 - 2.0 * y1 + y0);
    let c_y = 3.0 * (y1 - y0);
    let d_y = y0;

    let mut u = inv_cubic_bezier_x(x, a, a_speed, b_speed, b);
    u = tcrop(u, 0.0, 1.0);
    a_y * u * u * u + b_y * u * u + c_y * u + d_y
}

pub fn get_min_max_cubic_bezier_y(a: PointD, a_speed: PointD, b_speed: PointD, b: PointD) -> (PointD, PointD) {
    let y0 = a.y;
    let y1 = y0 + a_speed.y;
    let y3 = b.y;
    let y2 = y3 + b_speed.y;

    let a_y = y3 + 3.0 * (y1 - y2) - y0;
    let b_y = 3.0 * (y2 - 2.0 * y1 + y0);
    let c_y = 3.0 * (y1 - y0);
    let d_y = y0;

    let x0 = a.x;
    let x1 = x0 + a_speed.x;
    let x3 = b.x;
    let x2 = x3 + b_speed.x;

    let a_x = x3 + 3.0 * (x1 - x2) - x0;
    let b_x = 3.0 * (x2 - 2.0 * x1 + x0);
    let c_x = 3.0 * (x1 - x0);
    let d_x = x0;

    let aa_y = 3.0 * (y1 - y2) - y0 + y3;
    let bb_y = 2.0 * (y0 + y2 - 2.0 * y1);
    let cc_y = y1 - y0;

    if aa_y == 0.0 {
        if a.y < b.y {
            (pd(a.x, a.y), pd(b.x, b.y))
        } else {
            (pd(b.x, b.y), pd(a.x, a.y))
        }
    } else {
        let discr = bb_y * bb_y - 4.0 * aa_y * cc_y;
        if discr < 0.0 {
            if a.y < b.y {
                (pd(a.x, a.y), pd(b.x, b.y))
            } else {
                (pd(b.x, b.y), pd(a.x, a.y))
            }
        } else {
            let sqrt_discr = discr.sqrt();
            let inv_2a_y = 1.0 / (2.0 * aa_y);
            let mut u0 = (-bb_y + sqrt_discr) * inv_2a_y;
            let mut u1 = (-bb_y - sqrt_discr) * inv_2a_y;
            if u0 > 1.0 {
                u0 = 1.0;
            }
            if u0 < 0.0 {
                u0 = 0.0;
            }
            if u1 > 1.0 {
                u1 = 1.0;
            }
            if u1 < 0.0 {
                u1 = 0.0;
            }
            let y_0 = a_y * u0 * u0 * u0 + b_y * u0 * u0 + c_y * u0 + d_y;
            let y_1 = a_y * u1 * u1 * u1 + b_y * u1 * u1 + c_y * u1 + d_y;
            let x_0 = a_x * u0 * u0 * u0 + b_x * u0 * u0 + c_x * u0 + d_x;
            let x_1 = a_x * u1 * u1 * u1 + b_x * u1 * u1 + c_x * u1 + d_x;
            if y_0 < y_1 {
                (pd(x_0, y_0), pd(x_1, y_1))
            } else {
                (pd(x_1, y_1), pd(x_0, y_0))
            }
        }
    }
}

//=============================================================================
// C ABI: TPointD arguments are passed as (x, y) scalars.

#[no_mangle]
pub extern "C" fn flare_invCubicBezierX(
    x: f64, ax: f64, ay: f64, asx: f64, asy: f64, bsx: f64, bsy: f64, bx: f64, by: f64,
) -> f64 {
    inv_cubic_bezier_x(x, pd(ax, ay), pd(asx, asy), pd(bsx, bsy), pd(bx, by))
}

#[no_mangle]
pub extern "C" fn flare_getCubicBezierY(
    x: f64, ax: f64, ay: f64, asx: f64, asy: f64, bsx: f64, bsy: f64, bx: f64, by: f64,
) -> f64 {
    get_cubic_bezier_y(x, pd(ax, ay), pd(asx, asy), pd(bsx, bsy), pd(bx, by))
}

/// `out` = { min.x, min.y, max.x, max.y }
///
/// # Safety
/// `out` must point to 4 doubles.
#[no_mangle]
pub unsafe extern "C" fn flare_getMinMaxCubicBezierY(
    ax: f64, ay: f64, asx: f64, asy: f64, bsx: f64, bsy: f64, bx: f64, by: f64, out: *mut f64,
) {
    let (lo, hi) = get_min_max_cubic_bezier_y(pd(ax, ay), pd(asx, asy), pd(bsx, bsy), pd(bx, by));
    let o = std::slice::from_raw_parts_mut(out, 4);
    o[0] = lo.x;
    o[1] = lo.y;
    o[2] = hi.x;
    o[3] = hi.y;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    // Segment (0,0) -> (3,3), speeds (1,1)/(-1,-1): control points (0,0),(1,1),(2,2),(3,3), a straight line.
    fn line() -> (PointD, PointD, PointD, PointD) {
        (pd(0.0, 0.0), pd(1.0, 1.0), pd(-1.0, -1.0), pd(3.0, 3.0))
    }

    #[test]
    fn linear_segment_is_identity() {
        let (a, asp, bsp, b) = line();
        // cubicRoot returns the root in [0,1]; x(u) = 3u, so u = x/3
        assert!(near(inv_cubic_bezier_x(1.5, a, asp, bsp, b), 0.5));
        assert!(near(get_cubic_bezier_y(1.5, a, asp, bsp, b), 1.5));
        assert!(near(get_cubic_bezier_y(0.0, a, asp, bsp, b), 0.0));
        assert!(near(get_cubic_bezier_y(3.0, a, asp, bsp, b), 3.0));
    }

    #[test]
    fn zero_speed_x_is_replaced_by_epsilon() {
        // With aSpeed.x == 0 and bSpeed.x == 0 the code still builds a valid cubic.
        let u = inv_cubic_bezier_x(0.5, pd(0.0, 0.0), pd(0.0, 1.0), pd(0.0, -1.0), pd(1.0, 1.0));
        assert!((0.0..=1.0).contains(&u));
        // x(u) = 3u^2 - 2u^3 (+- eps terms): u = 0.5 at x = 0.5
        assert!((u - 0.5).abs() < 1e-5, "{}", u);
    }

    #[test]
    fn y_is_clamped_by_u() {
        let (a, asp, bsp, b) = line();
        // x far outside the segment: cubicRoot returns 1.0 -> y(b)
        assert!(near(get_cubic_bezier_y(100.0, a, asp, bsp, b), 3.0));
    }

    #[test]
    fn min_max_degenerate_and_ordering() {
        let (a, asp, bsp, b) = line();
        // aaY == 0 for a straight line: endpoints ordered by y
        assert_eq!(get_min_max_cubic_bezier_y(a, asp, bsp, b), (pd(0.0, 0.0), pd(3.0, 3.0)));
        assert_eq!(get_min_max_cubic_bezier_y(b, bsp, asp, a), (pd(0.0, 0.0), pd(3.0, 3.0)));
        // y control points 0, 3, -3, 0: aaY=18, bbY=-18, ccY=3, discr=108,
        // u in {0.2113, 0.7887}, y(u) = 18u^3 - 27u^2 + 9u = -+0.8660
        let (lo, hi) = get_min_max_cubic_bezier_y(pd(0.0, 0.0), pd(1.0, 3.0), pd(-1.0, -3.0), pd(3.0, 0.0));
        assert!(near(lo.y, -(3f64.sqrt()) / 2.0) && near(hi.y, 3f64.sqrt() / 2.0));
    }

    #[test]
    fn min_max_negative_discriminant_falls_back_to_endpoints() {
        // y control points 0,1,2,4: aaY=1, bbY=0, ccY=1 -> discr = -4 < 0
        let (lo, hi) = get_min_max_cubic_bezier_y(pd(0.0, 0.0), pd(1.0, 1.0), pd(-1.0, -2.0), pd(3.0, 4.0));
        assert_eq!((lo, hi), (pd(0.0, 0.0), pd(3.0, 4.0)));
    }

    #[test]
    fn ffi_matches() {
        assert!(near(flare_getCubicBezierY(1.5, 0.0, 0.0, 1.0, 1.0, -1.0, -1.0, 3.0, 3.0), 1.5));
        let mut out = [0.0; 4];
        unsafe { flare_getMinMaxCubicBezierY(0.0, 0.0, 1.0, 1.0, -1.0, -1.0, 3.0, 3.0, out.as_mut_ptr()) };
        assert_eq!(out, [0.0, 0.0, 3.0, 3.0]);
    }
}
