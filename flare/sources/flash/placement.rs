//! Port of `flash/placement.cpp`: `Flash::stagePlacement`, which decomposes an XFL affine
//! (a b c d tx ty, downward Y) into Flare stage controls (position / centre in inches,
//! rotation in degrees, scale, shear). The structs and the `Error` type stay in
//! `placement.h` / `geometry.h`; the header forwards here and throws the original texts.
//!
//! Arithmetic order follows the C++ (QR form, then the rotated-basis fallback for rank-one
//! matrices); `hypot`, `atan2`, `sin`, `cos` are the platform libm as before.

use std::f64::consts::PI;

pub const PLACEMENT_OK: i32 = 0;
/// "Invalid XFL transformation matrix."
pub const PLACEMENT_BAD_MATRIX: i32 = 1;
/// "Invalid native placement scale or pivot."
pub const PLACEMENT_BAD_SCALE_PIVOT: i32 = 2;
/// "Native placement exceeds finite coordinate limits."
pub const PLACEMENT_OVERFLOW: i32 = 3;

/// `m` = a b c d tx ty. `out` = position.xy, center.xy, angle, scaleX, scaleY, shearX, shearY.
fn placement(m: &[f64; 6], dpi: f64, px: f64, py: f64) -> Result<[f64; 9], i32> {
    let [ma, mb, mc, md, tx, ty] = *m;
    if m.iter().any(|v| !v.is_finite() || v.abs() > 1000000000.0) {
        return Err(PLACEMENT_BAD_MATRIX);
    }
    if !dpi.is_finite() || dpi <= 0.0 || !px.is_finite() || !py.is_finite() {
        return Err(PLACEMENT_BAD_SCALE_PIVOT);
    }
    let center = (px / dpi, -py / dpi);
    // Affine::apply
    let pp = (ma * px + mc * py + tx, mb * px + md * py + ty);
    let position = (pp.0 / dpi, -pp.1 / dpi);
    let a = ma;
    let b = -mb;
    let c = -mc;
    let d = md;
    let first_length = a.hypot(b);
    let second_length = c.hypot(d);
    let mut radians = 0.0;
    let (mut scale_x, mut scale_y, mut shear_x, mut shear_y) = (1.0, 1.0, 0.0, 0.0);
    if first_length == 0.0 {
        scale_x = 0.0;
        scale_y = second_length;
        if second_length != 0.0 {
            radians = (-c).atan2(d);
        }
    } else {
        radians = b.atan2(a);
        let cosine = a / first_length;
        let sine = b / first_length;
        let upper = cosine * c + sine * d;
        let lower = -sine * c + cosine * d;
        scale_x = first_length;
        scale_y = lower;
        shear_x = if lower == 0.0 { 0.0 } else { upper / lower };
        if second_length != 0.0 && (lower == 0.0 || !shear_x.is_finite() || shear_x.abs() > 1000000.0) {
            radians += PI / 4.0;
            let cs = radians.cos();
            let sn = radians.sin();
            scale_x = cs * a + sn * b;
            scale_y = -sn * c + cs * d;
            shear_x = (cs * c + sn * d) / scale_y;
            shear_y = (-sn * a + cs * b) / scale_x;
        }
    }
    let angle = radians * 180.0 / PI;
    let out = [position.0, position.1, center.0, center.1, angle, scale_x, scale_y, shear_x, shear_y];
    if out.iter().any(|v| !v.is_finite()) {
        return Err(PLACEMENT_OVERFLOW);
    }
    Ok(out)
}

/// `Flash::stagePlacement`. Returns 0 or a `PLACEMENT_*` status (nothing written).
///
/// # Safety
/// `matrix` valid for 6 doubles, `out` for 9.
#[no_mangle]
pub unsafe extern "C" fn flare_flash_stage_placement(
    matrix: *const f64,
    dpi: f64,
    pivot_x: f64,
    pivot_y: f64,
    out: *mut f64,
) -> i32 {
    std::panic::catch_unwind(|| {
        if matrix.is_null() || out.is_null() {
            return PLACEMENT_BAD_MATRIX;
        }
        let m = *(matrix as *const [f64; 6]);
        match placement(&m, dpi, pivot_x, pivot_y) {
            Ok(v) => {
                std::ptr::copy_nonoverlapping(v.as_ptr(), out, 9);
                PLACEMENT_OK
            }
            Err(e) => e,
        }
    })
    .unwrap_or(PLACEMENT_OVERFLOW)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_and_rotation() {
        let r = placement(&[1.0, 0.0, 0.0, 1.0, 20.0, 40.0], 100.0, 0.0, 0.0).unwrap();
        assert_eq!(r, [0.2, -0.4, 0.0, -0.0, 0.0, 1.0, 1.0, 0.0, 0.0]);
        let q = PI / 2.0;
        let r = placement(&[q.cos(), q.sin(), -q.sin(), q.cos(), 0.0, 0.0], 1.0, 0.0, 0.0).unwrap();
        assert!((r[4] + 90.0).abs() < 1e-9);
    }

    #[test]
    fn errors() {
        assert_eq!(placement(&[f64::NAN, 0.0, 0.0, 1.0, 0.0, 0.0], 1.0, 0.0, 0.0), Err(PLACEMENT_BAD_MATRIX));
        assert_eq!(placement(&[1.0, 0.0, 0.0, 1.0, 0.0, 0.0], 0.0, 0.0, 0.0), Err(PLACEMENT_BAD_SCALE_PIVOT));
        assert_eq!(placement(&[1e9, 0.0, 0.0, 1.0, 1e9, 0.0], 1e-300, 0.0, 0.0), Err(PLACEMENT_OVERFLOW));
    }
}
