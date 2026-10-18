//! Port of stdfx/igs_attenuation_distribution.cpp: generate 2D attenuation lens matrices.
//!
//! Creates a conical (curve=1.0) or gamma-curved decay distribution for circular or
//! polygonal masks. Used by maxmin/fog/glow effects.

use std::f64::consts::PI;

pub const ERR_CURVE_ZERO: i32 = -1;
pub const ERR_DIAMETER_TOO_SMALL: i32 = -2;
pub const OK: i32 = 0;

/// Point-in-polygon test for regular N-gon inscribed in circle at `radius`,
/// rotated by `degree` (CCW). Scanline intersection count.
fn inside_polygon(
    radius: f64,
    odd_diameter: i32,
    xp: f64,
    yp: f64,
    polygon_num: i32,
    degree: f64,
) -> bool {
    let mut radian = degree * (PI / 180.0);
    let add_radian = 2.0 * PI / (polygon_num as f64);
    let mut x1 = 0.0;
    let mut y1 = 0.0;
    let mut xa = -(odd_diameter as f64);
    let mut xb = -(odd_diameter as f64);

    for ii in 0..=polygon_num {
        // Vertex at (x2, y2); flip for math convention (start right, CCW)
        let x2 = -radius * radian.cos();
        let y2 = -radius * radian.sin();

        if ii > 0 {
            // Check if scanline yp intersects segment (x1,y1)→(x2,y2)
            if !((y1 <= yp && yp <= y2) || (y2 <= yp && yp <= y1)) {
                x1 = x2;
                y1 = y2;
                radian += add_radian;
                continue;
            }

            // Horizontal segment
            if (y2 - y1).abs() < f64::EPSILON {
                return (x1 <= xp && xp <= x2) || (x2 <= xp && xp <= x1);
            }

            // Compute x intersection
            let x_intersect = (yp - y1) * (x2 - x1) / (y2 - y1) + x1;

            if xa == -(odd_diameter as f64) {
                xa = x_intersect;
            } else if xb == -(odd_diameter as f64) {
                xb = x_intersect;
                return (xa <= xp && xp <= xb) || (xb <= xp && xp <= xa);
            }
        }

        x1 = x2;
        y1 = y2;
        radian += add_radian;
    }
    false
}

/// Generate attenuation distribution: a 2D lens matrix with conical/gamma decay.
///
/// # Parameters
/// - `radius`: effect radius in pixels (must be > 1.0)
/// - `curve`: gamma exponent (0 < curve; 1.0 = linear, <1.0 = darker, >1.0 = brighter)
/// - `polygon_num`: 2 = circle, >=3 = regular polygon inscribed in circle
/// - `degree`: rotation angle (degrees, CCW)
///
/// # Returns
/// `(lens_matrix, lens_offsets, lens_sizes, odd_diameter, status_code)`
///
/// Status: 0=OK, -1=curve is zero, -2=diameter <=1
///
/// `lens_matrix[y][x]`: normalized kernel weights (sum = 1.0)
/// `lens_offsets[y]`: first non-zero x in row y (-1 if empty)
/// `lens_sizes[y]`: count of non-zero pixels in row y
/// `odd_diameter`: actual matrix size (odd, >= ceil(radius*2))
// Index form mirrors the original C++ loops; the golden encodes this iteration
// order, so rewriting them as iterators would obscure the comparison for no gain.
#[allow(clippy::needless_range_loop)]
pub fn attenuation_distribution(
    radius: f64,
    curve: f64,
    polygon_num: i32,
    degree: f64,
) -> (Vec<Vec<f64>>, Vec<i32>, Vec<i32>, i32, i32) {
    // Validate curve
    if curve.abs() < f64::EPSILON {
        return (vec![], vec![], vec![], 0, ERR_CURVE_ZERO);
    }

    // Compute odd diameter
    let mut odd_diameter = (radius * 2.0).ceil() as i32;
    if odd_diameter <= 1 {
        return (vec![], vec![], vec![], 0, ERR_DIAMETER_TOO_SMALL);
    }
    if odd_diameter % 2 == 0 {
        odd_diameter += 1;
    }

    let size = odd_diameter as usize;
    let mut lens_matrix = vec![vec![0.0; size]; size];
    let mut lens_offsets = vec![-1; size];
    let mut lens_sizes = vec![-1; size];

    // Generate matrix with gamma-curved decay
    let mut yp = 0.5 - (odd_diameter as f64) / 2.0;
    for yy in 0..size {
        let mut xp = 0.5 - (odd_diameter as f64) / 2.0;
        for xx in 0..size {
            let length = (xp * xp + yp * yp).sqrt();
            let inside = length <= radius
                && (polygon_num < 3
                    || inside_polygon(radius, odd_diameter, xp, yp, polygon_num, degree));

            if inside {
                // Linear decay 1.0 (center) → 0.0 (edge)
                let mut val = 1.0 - (length / radius);
                // Apply gamma curve
                val = val.powf(1.0 / curve);
                lens_matrix[yy][xx] = val;

                // Record scanline start
                if lens_offsets[yy] < 0 {
                    lens_offsets[yy] = xx as i32;
                }
            } else {
                lens_matrix[yy][xx] = 0.0;

                // Record scanline size when exiting
                if lens_offsets[yy] >= 0 && lens_sizes[yy] < 0 {
                    lens_sizes[yy] = xx as i32 - lens_offsets[yy];
                }
            }
            xp += 1.0;
        }

        // Handle full-width scanlines
        if lens_offsets[yy] >= 0 && lens_sizes[yy] < 0 {
            lens_sizes[yy] = size as i32 - lens_offsets[yy];
        }
        yp += 1.0;
    }

    // Normalize so sum = 1.0
    let total: f64 = lens_matrix.iter().flat_map(|row| row.iter()).sum();
    if total > f64::EPSILON {
        for row in lens_matrix.iter_mut() {
            for val in row.iter_mut() {
                *val /= total;
            }
        }
    }

    (lens_matrix, lens_offsets, lens_sizes, odd_diameter, OK)
}

// C FFI exports for legacy C++ code.
//
// TRANSITIONAL FFI — remove once `stdfx/igs_fog.cpp` is ported to Rust.
// `igs_fog.cpp` is the only C++ caller: it reaches the lens matrix through the
// `attenuation_distribution_` shim in `igs_attenuation_distribution.h`, which calls
// these exports. When the fog kernel moves to Rust it can call
// `attenuation_distribution()` directly, so delete together:
//   - `flare_igs_attenuation_distribution` and `flare_igs_attenuation_free` (below),
//   - the inline wrapper in `igs_attenuation_distribution.h`,
//   - that header's `#include` in `igs_fog.cpp` and its CMake HEADERS entry,
//   - `tests/native/igs_attenuation_wrapper_probe.cpp` (it probes the wrapper).
// KEEP `stdfx/igs_attenuation_cases.txt`: the generator recovers the original C++ from
// git history, so the golden keeps pinning the Rust port after the C++ caller is gone.
// Do not add new callers.
use std::os::raw::{c_double, c_int};

/// An out-parameter was NULL; nothing was written.
pub const ERR_BAD_OUTPUT: i32 = -3;
/// An allocation failed; nothing is published and all out-parameters are NULL.
pub const ERR_ALLOC: i32 = -4;

/// C-compatible wrapper: computes the lens matrix and publishes three arrays plus
/// the odd diameter. All-or-nothing: either every out-parameter is set, or none is
/// and a non-zero status is returned. Release with `flare_igs_attenuation_free`.
///
/// # Safety
/// Each out-parameter must be a valid, writable pointer. A NULL one is rejected with
/// `ERR_BAD_OUTPUT` before anything is written. On success the three arrays are owned
/// by the caller and must be released with [`flare_igs_attenuation_free`].
#[no_mangle]
#[allow(clippy::needless_range_loop)] // flat row-major copy mirrors the C++ indexing
pub unsafe extern "C" fn flare_igs_attenuation_distribution(
    radius: c_double,
    curve: c_double,
    polygon_num: c_int,
    degree: c_double,
    out_matrix: *mut *mut c_double,
    out_offsets: *mut *mut c_int,
    out_sizes: *mut *mut c_int,
    out_odd_diameter: *mut c_int,
) -> c_int {
    if out_matrix.is_null()
        || out_offsets.is_null()
        || out_sizes.is_null()
        || out_odd_diameter.is_null()
    {
        return ERR_BAD_OUTPUT;
    }
    unsafe {
        *out_matrix = std::ptr::null_mut();
        *out_offsets = std::ptr::null_mut();
        *out_sizes = std::ptr::null_mut();
        *out_odd_diameter = 0;
    }

    // A panic must not unwind out of `extern "C"` (it aborts the process), so the
    // body goes through the crate guard like every other FFI entry point.
    crate::guard(ERR_ALLOC, || unsafe {
        let (mat, offs, sizes, diam, status) =
            attenuation_distribution(radius, curve, polygon_num, degree);
        if status != OK {
            return status;
        }

        let n = diam as usize;
        let mat_bytes = n
            .checked_mul(n)
            .and_then(|v| v.checked_mul(std::mem::size_of::<c_double>()));
        let int_bytes = n.checked_mul(std::mem::size_of::<c_int>());
        let (mat_bytes, int_bytes) = match (mat_bytes, int_bytes) {
            (Some(m), Some(i)) => (m, i),
            _ => return ERR_ALLOC,
        };

        let flat_mat = libc::malloc(mat_bytes) as *mut c_double;
        let offs_ptr = libc::malloc(int_bytes) as *mut c_int;
        let sizes_ptr = libc::malloc(int_bytes) as *mut c_int;
        if flat_mat.is_null() || offs_ptr.is_null() || sizes_ptr.is_null() {
            // free(NULL) is a no-op, so this covers partial allocation too.
            libc::free(flat_mat as *mut libc::c_void);
            libc::free(offs_ptr as *mut libc::c_void);
            libc::free(sizes_ptr as *mut libc::c_void);
            return ERR_ALLOC;
        }

        for y in 0..n {
            for x in 0..n {
                *flat_mat.add(y * n + x) = mat[y][x];
            }
        }
        for (i, &v) in offs.iter().enumerate() {
            *offs_ptr.add(i) = v;
        }
        for (i, &v) in sizes.iter().enumerate() {
            *sizes_ptr.add(i) = v;
        }

        // Everything succeeded; now publish.
        *out_matrix = flat_mat;
        *out_offsets = offs_ptr;
        *out_sizes = sizes_ptr;
        *out_odd_diameter = diam;
        OK
    })
}

/// Free arrays allocated by flare_igs_attenuation_distribution().
///
/// # Safety
/// Each pointer must come from `flare_igs_attenuation_distribution` and must not be
/// used afterwards. NULL is accepted (and ignored), so a failed call can be released
/// unconditionally.
#[no_mangle]
pub unsafe extern "C" fn flare_igs_attenuation_free(
    matrix: *mut c_double,
    offsets: *mut c_int,
    sizes: *mut c_int,
) {
    unsafe {
        if !matrix.is_null() {
            libc::free(matrix as *mut libc::c_void);
        }
        if !offsets.is_null() {
            libc::free(offsets as *mut libc::c_void);
        }
        if !sizes.is_null() {
            libc::free(sizes as *mut libc::c_void);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_zero_curve() {
        let (_, _, _, _, status) = attenuation_distribution(5.0, 0.0, 2, 0.0);
        assert_eq!(status, ERR_CURVE_ZERO);
    }

    #[test]
    fn rejects_tiny_radius() {
        let (_, _, _, _, status) = attenuation_distribution(0.4, 1.0, 2, 0.0);
        assert_eq!(status, ERR_DIAMETER_TOO_SMALL);
    }

    #[test]
    fn circle_radius_5_linear() {
        let (matrix, offsets, sizes, diam, status) =
            attenuation_distribution(5.0, 1.0, 2, 0.0);
        assert_eq!(status, OK);
        assert_eq!(diam, 11); // ceil(5*2)=10 → 11 (odd)
        assert_eq!(matrix.len(), 11);

        // Center pixel should be max
        let center = matrix[5][5];
        assert!(center > 0.0);

        // Check normalization
        let sum: f64 = matrix.iter().flat_map(|r| r.iter()).sum();
        assert!((sum - 1.0).abs() < 1e-9, "sum={}", sum);

        // Offsets/sizes should be symmetric-ish
        assert!(offsets[5] >= 0);
        assert!(sizes[5] > 0);
    }

    #[test]
    fn polygon_4_square() {
        let (matrix, _, _, diam, status) = attenuation_distribution(5.0, 1.0, 4, 0.0);
        assert_eq!(status, OK);
        assert_eq!(diam, 11);

        // Should still be normalized
        let sum: f64 = matrix.iter().flat_map(|r| r.iter()).sum();
        assert!((sum - 1.0).abs() < 1e-9);
    }

    #[test]
    fn gamma_curve_darker() {
        let (mat_lin, _, _, _, _) = attenuation_distribution(5.0, 1.0, 2, 0.0);
        let (mat_dark, _, _, _, _) = attenuation_distribution(5.0, 0.5, 2, 0.0);

        // curve < 1.0 → BRIGHTER (exponent 1/0.5=2 lifts values)
        // curve > 1.0 → darker
        // Check mid-radius: 0.5 curve should brighten mid-range
        let mid = 3;
        assert!(mat_dark[5][mid] > mat_lin[5][mid], "curve=0.5 should brighten vs linear");
    }

    /// Golden from the original C++ (`tests/native/gen_igs_attenuation_cases.py`).
    ///
    /// Structural fields (diameter, per-row offset/size, and where `lens_starts[y]`
    /// points) are compared exactly; matrix values within 1e-12 because
    /// `sqrt`/`pow`/`cos`/`sin` may differ in the last bits between the compiler that
    /// recorded the golden and the one running this test.
    #[test]
    fn matches_the_cpp_reference() {
        const GOLDEN: &str = include_str!("igs_attenuation_cases.txt");

        let mut cases = 0usize;
        for block in GOLDEN.split("\n\n").filter(|b| !b.trim().is_empty()) {
            let lines: Vec<&str> = block.lines().collect();
            let head: Vec<&str> = lines[0].split_whitespace().collect();

            if head[0] == "err" {
                let which: i32 = head[1].parse().unwrap();
                let (radius, curve) = if which == 0 { (5.0, 0.0) } else { (0.4, 1.0) };
                let (_, _, _, _, status) = attenuation_distribution(radius, curve, 2, 0.0);
                let want = if which == 0 {
                    ERR_CURVE_ZERO
                } else {
                    ERR_DIAMETER_TOO_SMALL
                };
                assert_eq!(status, want, "error case {which}");
                cases += 1;
                continue;
            }

            // case <radius> <curve> <polygon> <degree> diam <diam>
            let radius: f64 = head[1].parse().unwrap();
            let curve: f64 = head[2].parse().unwrap();
            let polygon: i32 = head[3].parse().unwrap();
            let degree: f64 = head[4].parse().unwrap();
            let want_diam: i32 = head[6].parse().unwrap();

            let (matrix, offsets, sizes, diam, status) =
                attenuation_distribution(radius, curve, polygon, degree);
            assert_eq!(status, OK, "case r={radius} c={curve} p={polygon} d={degree}");
            assert_eq!(diam, want_diam, "diameter r={radius} c={curve} p={polygon} d={degree}");
            assert_eq!(matrix.len(), want_diam as usize);

            let n = want_diam as usize;
            for y in 0..n {
                let row: Vec<&str> = lines[1 + y].split_whitespace().collect();
                let want_offset: i32 = row[3].parse().unwrap();
                let want_size: i32 = row[5].parse().unwrap();
                let want_delta: i64 = row[7].parse().unwrap();
                assert_eq!(offsets[y], want_offset, "offset r={radius} y={y}");
                assert_eq!(sizes[y], want_size, "size r={radius} y={y}");
                // lens_starts[y] must point at the first covered pixel, not the row start.
                let want = if want_offset >= 0 { want_offset as i64 } else { -1 };
                assert_eq!(want_delta, want, "golden start_delta r={radius} y={y}");
            }

            for y in 0..n {
                for x in 0..n {
                    let line = lines[1 + n + y * n + x];
                    let hex = line.split_whitespace().nth(1).unwrap();
                    let want = f64::from_bits(u64::from_str_radix(hex, 16).unwrap());
                    let got = matrix[y][x];
                    assert!(
                        (got - want).abs() <= 1e-12,
                        "matrix[{y}][{x}] r={radius} c={curve} p={polygon} d={degree}: got {got}, want {want}"
                    );
                }
            }
            cases += 1;
        }
        assert!(cases >= 62, "golden looked truncated: {cases} cases");
    }
}
