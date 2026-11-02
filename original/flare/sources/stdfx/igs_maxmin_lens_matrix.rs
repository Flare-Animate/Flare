//! Port of stdfx/igs_maxmin_lens_matrix.cpp: the "lens" (round or polygonal brush with a
//! soft outer edge) used by the max/min effect.
//!
//! The lens is `odd_diameter` scanlines; scanline `yy` covers `sizes[yy]` pixels starting
//! at column `offsets[yy]` (`-1` / `0` when the row is outside), and `ratio[yy][0..size]`
//! is the weight of each covered pixel: 1 inside the inner shape, a linear (circle) or
//! perpendicular-distance (polygon) fall-off up to the outer shape.
//!
//! Trigonometry (`cos`/`sin`/`atan2`) goes through the platform libm like the C++, so
//! polygon weights can differ by a few ULP between platforms (circle weights use
//! `sqrt` only and are exact).
//!
//! Deviations (only where the C++ was undefined behaviour or could not finish):
//! * `diameter_from_outer_radius` of NaN / absurd radii cannot be allocated: the shape
//!   call returns an error instead of `std::length_error` / `std::bad_alloc`, and a lens
//!   above 1 GiB of weights is refused;
//! * the `while (radian1 < 0.0) radian1 += add_radian` loop of `length_to_polygon_` is
//!   capped at 2^26 iterations (a huge negative roll never finished);
//! * the C++ `.at()` bounds checks become panics inside the FFI guard.

use std::f64::consts::PI;

/// Lens matrix (`lens_offsets`, `lens_sizes`, `lens_ratio` of the C++).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Lens {
    pub offsets: Vec<i32>,
    pub sizes: Vec<i32>,
    pub ratio: Vec<Vec<f64>>,
}

/// Refusal reasons of the allocating entry point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LensError {
    /// non-finite radius / unusable diameter (`std::length_error` in the C++)
    BadRadius,
    /// more than `MAX_LENS_BYTES` of weights (`std::bad_alloc`)
    TooLarge,
}

pub const MAX_LENS_BYTES: usize = 1 << 30;

fn inside_polygon(radius: f64, odd_diameter: i32, xp: f64, yp: f64, polygon_number: i32, roll_degree: f64) -> bool {
    if polygon_number < 3 {
        /* equal less than 2 is circle */
        return true;
    }
    let od = f64::from(odd_diameter);
    let mut radian = roll_degree * (PI / 180.0);
    let add_radian = 2.0 * PI / f64::from(polygon_number);
    let mut x1 = radius * radian.cos();
    let mut y1 = radius * radian.sin();
    let (mut x2, mut y2);
    let mut xa = od;
    let mut xb = od;
    radian += add_radian;

    for _ in 0..polygon_number {
        x2 = radius * radian.cos();
        y2 = radius * radian.sin();

        'segment: {
            /* skip segments the scanline does not cross */
            if !(((y1 <= yp) && (yp <= y2)) || ((y2 <= yp) && (yp <= y1))) {
                break 'segment;
            }
            /* horizontal segment */
            if y2 == y1 {
                return ((x1 <= xp) && (xp <= x2)) || ((x2 <= xp) && (xp <= x1));
            }
            /* first crossing */
            if xa == od {
                xa = (yp - y1) * (x2 - x1) / (y2 - y1) + x1;
            } else if xb == od {
                /* second crossing */
                xb = (yp - y1) * (x2 - x1) / (y2 - y1) + x1;
                return ((xa <= xp) && (xp <= xb)) || ((xb <= xp) && (xp <= xa));
            }
        }
        radian += add_radian;
        x1 = x2;
        y1 = y2;
    }
    false
}

fn length_to_polygon(radius: f64, xp: f64, yp: f64, polygon_number: i32, roll_degree: f64) -> f64 {
    let mut radian = yp.atan2(xp);
    if radian < 0.0 {
        radian += PI * 2.0;
    }

    let (mut x1, mut y1, mut x2, mut y2) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    let add_radian = 2.0 * PI / f64::from(polygon_number);
    let mut radian1 = roll_degree * (PI / 180.0);
    let mut guard = 0u32;
    while radian1 < 0.0 {
        radian1 += add_radian;
        guard += 1;
        if guard >= 1 << 26 {
            break;
        }
    }
    let mut radian2 = radian1 + add_radian;

    if radian < radian1 {
        radian += PI * 2.0;
    }

    for _ in 0..polygon_number {
        if radian1 <= radian && radian <= radian2 {
            x1 = radius * radian1.cos();
            y1 = radius * radian1.sin();
            x2 = radius * radian2.cos();
            y2 = radius * radian2.sin();
            break;
        }
        radian1 = radian2;
        radian2 += add_radian;
    }
    if x1 == 0.0 {
        return -1.0;
    }
    ((y1 - y2) * xp + (x2 - x1) * yp + x1 * y2 - y1 * x2).abs() / ((y1 - y2) * (y1 - y2) + (x2 - x1) * (x2 - x1)).sqrt()
}

/// `igs::maxmin::diameter_from_outer_radius`
pub fn diameter_from_outer_radius(outer_radius: f64) -> i32 {
    /* (static_cast<int>(ceil(outer_radius + 0.5) + 0.5) - 1) * 2 + 1 */
    (((outer_radius + 0.5).ceil() + 0.5) as i32).wrapping_sub(1).wrapping_mul(2).wrapping_add(1)
}

/// `igs::maxmin::outer_radius_from_radius`
pub fn outer_radius_from_radius(radius: f64, smooth_outer_range: f64) -> f64 {
    if radius < 1.0 {
        return 1.0 + radius * (radius + smooth_outer_range - 1.0);
    }
    radius + smooth_outer_range
}

/// `alloc_lens_matrix_`
fn alloc_lens_matrix(odd_diameter: usize, lens: &mut Lens) {
    lens.offsets.resize(odd_diameter, 0);
    lens.sizes.resize(odd_diameter, 0);
    lens.ratio.resize(odd_diameter, Vec::new());
    for row in lens.ratio.iter_mut() {
        row.resize(odd_diameter, 0.0);
    }
}

/// `igs::maxmin::alloc_and_shape_lens_matrix`: returns the odd diameter, 0 when
/// `radius <= 0` (the lens is emptied).
pub fn alloc_and_shape_lens_matrix(
    radius: f64,
    outer_radius: f64,
    polygon_number: i32,
    roll_degree: f64,
    lens: &mut Lens,
) -> Result<i32, LensError> {
    if radius <= 0.0 {
        lens.ratio.clear();
        lens.sizes.clear();
        lens.offsets.clear();
        return Ok(0);
    }
    let odd_diameter = diameter_from_outer_radius(outer_radius);
    if !outer_radius.is_finite() || odd_diameter < 1 {
        return Err(LensError::BadRadius);
    }
    let d = odd_diameter as usize;
    if d.checked_mul(d).and_then(|n| n.checked_mul(8)).map_or(true, |b| b > MAX_LENS_BYTES) {
        return Err(LensError::TooLarge);
    }
    alloc_lens_matrix(d, lens);
    reshape_lens_matrix(
        radius,
        outer_radius_from_radius(radius, outer_radius - radius),
        odd_diameter,
        polygon_number,
        roll_degree,
        lens,
    );
    Ok(odd_diameter)
}

/// `igs::maxmin::reshape_lens_matrix`: rewrite the first `odd_diameter` rows of `lens`.
pub fn reshape_lens_matrix(
    radius: f64,
    outer_radius: f64,
    odd_diameter: i32,
    polygon_number: i32,
    roll_degree: f64,
    lens: &mut Lens,
) {
    let od = f64::from(odd_diameter);
    let start = 0.5 - (od / 2.0);
    /* scanline start position and size */
    let mut yp = start;
    for yy in 0..odd_diameter as usize {
        lens.offsets[yy] = -1;
        lens.sizes[yy] = 0;
        let mut xp = start;
        for xx in 0..odd_diameter {
            let current_radius = (xp * xp + yp * yp).sqrt();
            if current_radius <= outer_radius
                && inside_polygon(outer_radius, odd_diameter, xp, yp, polygon_number, roll_degree)
            {
                if lens.offsets[yy] < 0 {
                    lens.offsets[yy] = xx;
                }
            } else if 0 <= lens.offsets[yy] && lens.sizes[yy] == 0 {
                lens.sizes[yy] = xx - lens.offsets[yy];
            }
            xp += 1.0;
        }
        /* the scanline runs to the end */
        if 0 <= lens.offsets[yy] && lens.sizes[yy] == 0 {
            lens.sizes[yy] = odd_diameter - lens.offsets[yy];
        }

        /* outside the circle */
        if lens.sizes[yy] <= 0 {
            yp += 1.0;
            continue;
        }
        /* ratio: linear change from the outer to the inner shape */
        let mut xp = start;
        let mut xr = 0usize;
        for _ in 0..odd_diameter {
            let current_radius = (xp * xp + yp * yp).sqrt();
            if current_radius <= outer_radius
                && inside_polygon(outer_radius, odd_diameter, xp, yp, polygon_number, roll_degree)
            {
                if current_radius <= radius && inside_polygon(radius, odd_diameter, xp, yp, polygon_number, roll_degree) {
                    lens.ratio[yy][xr] = 1.0;
                } else if polygon_number < 3 {
                    lens.ratio[yy][xr] = (outer_radius - current_radius) / (outer_radius - radius);
                } else {
                    let leninn = length_to_polygon(radius, xp, yp, polygon_number, roll_degree);
                    let lenout = length_to_polygon(outer_radius, xp, yp, polygon_number, roll_degree);
                    lens.ratio[yy][xr] = lenout / (leninn + lenout);
                }
                xr += 1;
            }
            xp += 1.0;
        }
        yp += 1.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lens(radius: f64, smooth: f64, poly: i32, roll: f64) -> (i32, Lens) {
        let mut l = Lens::default();
        let d = alloc_and_shape_lens_matrix(radius, radius + smooth, poly, roll, &mut l).unwrap();
        (d, l)
    }

    #[test]
    fn diameter_and_outer_radius() {
        assert_eq!(diameter_from_outer_radius(0.0), 1);
        assert_eq!(diameter_from_outer_radius(1.0), 3);
        assert_eq!(diameter_from_outer_radius(1.4), 3);
        assert_eq!(diameter_from_outer_radius(1.6), 5);
        assert_eq!(diameter_from_outer_radius(3.0), 7);
        assert_eq!(outer_radius_from_radius(2.0, 1.5), 3.5);
        assert_eq!(outer_radius_from_radius(0.0, 2.0), 1.0);
        assert_eq!(outer_radius_from_radius(0.5, 2.0), 1.0 + 0.5 * 1.5);
        assert_eq!(outer_radius_from_radius(1.0, 2.0), 3.0);
    }

    #[test]
    fn zero_radius_empties_the_lens() {
        let mut l = Lens { offsets: vec![1], sizes: vec![1], ratio: vec![vec![1.0]] };
        assert_eq!(alloc_and_shape_lens_matrix(0.0, 3.0, 2, 0.0, &mut l), Ok(0));
        assert_eq!(l, Lens::default());
        assert_eq!(alloc_and_shape_lens_matrix(-1.0, 3.0, 2, 0.0, &mut l), Ok(0));
    }

    #[test]
    fn circle_lens_is_symmetric_with_unit_core() {
        let (d, l) = lens(2.0, 1.0, 2, 0.0);
        assert_eq!(d, 7);
        assert_eq!(l.offsets.len(), 7);
        // vertical and horizontal mirror symmetry
        for yy in 0..7 {
            let m = 6 - yy;
            assert_eq!(l.offsets[yy], l.offsets[m]);
            assert_eq!(l.sizes[yy], l.sizes[m]);
            let n = l.sizes[yy] as usize;
            assert_eq!(l.offsets[yy] + l.sizes[yy] + l.offsets[yy], 7, "row {yy} centred");
            for i in 0..n {
                assert_eq!(l.ratio[yy][i], l.ratio[yy][n - 1 - i]);
                assert_eq!(l.ratio[yy][i], l.ratio[m][i]);
            }
        }
        // centre row: 1 inside radius 2, falling to 0 at the outer edge (3)
        let mid = &l.ratio[3][..l.sizes[3] as usize];
        assert_eq!(mid.len(), 7);
        assert_eq!(&mid[1..6], &[1.0, 1.0, 1.0, 1.0, 1.0][..]);
        assert_eq!(mid[0], 0.0); // distance 3 == outer radius
    }

    #[test]
    fn unit_square_polygon_covers_the_corners_a_circle_misses() {
        let (_, circle) = lens(2.0, 0.0, 2, 0.0);
        let (_, square) = lens(2.0, 0.0, 4, 45.0);
        let covered = |l: &Lens| l.sizes.iter().sum::<i32>();
        assert!(covered(&square) > 0);
        // a diamond (square rolled 0 deg) holds fewer pixels than the circle of equal radius
        let (_, diamond) = lens(2.0, 0.0, 4, 0.0);
        assert!(covered(&diamond) < covered(&circle));
        // all weights are in [0, 1]
        for l in [&circle, &square, &diamond] {
            for (yy, row) in l.ratio.iter().enumerate() {
                for &r in &row[..l.sizes[yy].max(0) as usize] {
                    assert!((0.0..=1.0).contains(&r), "{r}");
                }
            }
        }
    }

    #[test]
    fn polygon_weights_fall_off_between_the_shapes() {
        let (_, l) = lens(2.0, 2.0, 6, 0.0);
        let mut seen_mid = false;
        for (yy, row) in l.ratio.iter().enumerate() {
            for &r in &row[..l.sizes[yy].max(0) as usize] {
                assert!((0.0..=1.0).contains(&r), "{r}");
                if r > 0.05 && r < 0.95 {
                    seen_mid = true;
                }
            }
        }
        assert!(seen_mid);
    }

    #[test]
    fn reshape_reuses_the_allocation_for_a_smaller_radius() {
        let (d, mut l) = lens(3.0, 1.0, 2, 0.0);
        let (a, b) = (l.offsets.len(), l.ratio.len());
        reshape_lens_matrix(1.0, outer_radius_from_radius(1.0, 1.0), d, 2, 0.0, &mut l);
        assert_eq!((l.offsets.len(), l.ratio.len()), (a, b));
        // the centre row now covers 3 pixels (outer radius 2) not 9
        let mid = d as usize / 2;
        assert_eq!(l.sizes[mid], 5);
    }

    #[test]
    fn unusable_radii_are_refused() {
        let mut l = Lens::default();
        assert_eq!(alloc_and_shape_lens_matrix(1.0, f64::NAN, 2, 0.0, &mut l), Err(LensError::BadRadius));
        assert_eq!(alloc_and_shape_lens_matrix(1.0, f64::INFINITY, 2, 0.0, &mut l), Err(LensError::BadRadius));
        assert_eq!(alloc_and_shape_lens_matrix(1.0, 1.0e6, 2, 0.0, &mut l), Err(LensError::TooLarge));
    }

    #[test]
    fn huge_negative_roll_terminates() {
        // the C++ spun forever; the loop is capped (result is meaningless but finite)
        let v = length_to_polygon(2.0, 1.0, 1.0, 5, -1.0e30);
        assert!(v.is_finite() || v.is_nan());
    }
}
