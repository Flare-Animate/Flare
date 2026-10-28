//! Port of stdfx/igs_maxmin_slrender.cpp: render one scanline of the max/min effect.
//!
//! `tracks[k]` is the (already margin-padded) scanline `k - margin` rows away from the
//! current one, `result` starts as the unmodified scanline and receives, per pixel, the
//! largest (max) or smallest (min) value reachable through the lens, each neighbour
//! pulled towards the centre value by its lens weight. With `alpha_ref` the lens radius
//! is `alpha_ref[x] * radius` per pixel and the lens is reshaped whenever it changes.
//!
//! The C++ walked raw `const double *` into the tracks; here the same positions are
//! column indices (`None` is the null pointer of a lens row that is outside). The
//! pointer for a row that is outside the lens is never advanced and never read, as
//! before. No deviations.

use crate::igs_maxmin_lens_matrix::{diameter_from_outer_radius, outer_radius_from_radius, reshape_lens_matrix, Lens};

/// The per-thread scratch of the C++ (`pixe_tracks_`, `alpha_ref_`, `result_`).
#[derive(Clone, Debug, Default)]
pub struct Scanline {
    pub tracks: Vec<Vec<f64>>,
    pub alpha_ref: Vec<f64>,
    pub result: Vec<f64>,
}

impl Scanline {
    /// `slrender::resize`
    pub fn resize(&mut self, odd_diameter: usize, width: usize, alpha_ref_sw: bool) {
        self.tracks.resize(odd_diameter, Vec::new());
        for t in self.tracks.iter_mut() {
            t.resize(width + odd_diameter - 1, 0.0);
        }
        if alpha_ref_sw {
            self.alpha_ref.resize(width, 0.0);
        }
        self.result.resize(width, 0.0);
    }

    /// `slrender::clear`
    pub fn clear(&mut self) {
        self.result.clear();
        self.alpha_ref.clear();
        self.tracks.clear();
    }

    /// `slrender::shift`: the last track becomes the first.
    pub fn shift(&mut self) {
        self.tracks.rotate_right(1);
    }
}

fn maxmin(src: f64, min_sw: bool, tracks: &[Vec<f64>], begin: &[Option<usize>], lens: &Lens) -> f64 {
    if min_sw {
        /* dark spreads: judge on the inverted values, invert the result back */
        let mut val = 1.0 - src;
        let rev_src = 1.0 - src;
        for yy in 0..begin.len() {
            let sz = lens.sizes[yy];
            if sz <= 0 {
                continue;
            }
            let start = begin[yy].expect("lens row with pixels has a start");
            let xs = &tracks[yy][start..start + sz as usize];
            let rs = &lens.ratio[yy][..sz as usize];
            for (x, r) in xs.iter().zip(rs) {
                let mut crnt = 1.0 - *x;
                if crnt <= rev_src {
                    continue;
                }
                crnt = rev_src + (crnt - rev_src) * *r;
                if val < crnt {
                    val = crnt;
                }
            }
        }
        return 1.0 - val;
    }
    /* Max */
    let mut val = src;
    for yy in 0..begin.len() {
        let sz = lens.sizes[yy];
        if sz <= 0 {
            continue;
        }
        let start = begin[yy].expect("lens row with pixels has a start");
        let xs = &tracks[yy][start..start + sz as usize];
        let rs = &lens.ratio[yy][..sz as usize];
        for (x, r) in xs.iter().zip(rs) {
            if *x <= src {
                continue;
            }
            let crnt = src + (*x - src) * *r;
            if val < crnt {
                val = crnt;
            }
        }
    }
    val
}

fn set_begin_ptr(tracks: &[Vec<f64>], lens: &Lens, offset: usize, begin: &mut [Option<usize>]) {
    for ii in 0..lens.offsets.len() {
        begin[ii] = if 0 <= lens.offsets[ii] {
            let col = offset + lens.offsets[ii] as usize;
            // `tracks.at(ii).at(col)`
            assert!(col < tracks[ii].len(), "std::out_of_range");
            Some(col)
        } else {
            None
        };
    }
}

/// `slrender::render`: render `tracks` into `result` (see the module comment).
#[allow(clippy::too_many_arguments)]
pub fn render(
    radius: f64,
    smooth_outer_range: f64,
    polygon_number: i32,
    roll_degree: f64,
    min_sw: bool,
    lens: &mut Lens,
    tracks: &[Vec<f64>],
    alpha_ref: &[f64],
    result: &mut [f64],
) {
    /* start position */
    let mut begin: Vec<Option<usize>> = vec![None; lens.offsets.len()];
    set_begin_ptr(tracks, lens, 0, &mut begin);

    /* the effect radius varies per pixel */
    if !alpha_ref.is_empty() {
        let mut before_radius = 0.0f64;
        for xx in 0..result.len() {
            /* radius of this pixel */
            let radius2 = alpha_ref[xx] * radius;

            if 0.0 < alpha_ref[xx] {
                /* reshape when the size differs from the previous pixel */
                if radius2 != before_radius {
                    reshape_lens_matrix(
                        radius2,
                        outer_radius_from_radius(radius2, smooth_outer_range),
                        diameter_from_outer_radius(radius + smooth_outer_range),
                        polygon_number,
                        roll_degree,
                        lens,
                    );
                    set_begin_ptr(tracks, lens, xx, &mut begin);
                }
                result[xx] = maxmin(result[xx], min_sw, tracks, &begin, lens);
            } /* alpha_ref == 0: unchanged */

            /* next position */
            for b in begin.iter_mut().flatten() {
                *b += 1;
            }
            if radius2 != before_radius {
                before_radius = radius2;
            }
        }
    } else {
        /* constant radius */
        for xx in 0..result.len() {
            result[xx] = maxmin(result[xx], min_sw, tracks, &begin, lens);
            for b in begin.iter_mut().flatten() {
                *b += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::igs_maxmin_lens_matrix::alloc_and_shape_lens_matrix;

    fn setup(radius: f64, smooth: f64) -> (Lens, usize) {
        let mut l = Lens::default();
        let d = alloc_and_shape_lens_matrix(radius, radius + smooth, 2, 0.0, &mut l).unwrap();
        (l, d as usize)
    }

    /// tracks for a single bright pixel at column `at` in the middle row
    fn impulse(d: usize, width: usize, at: usize, v: f64, bg: f64) -> Scanline {
        let mut s = Scanline::default();
        s.resize(d, width, false);
        for t in s.tracks.iter_mut() {
            for x in t.iter_mut() {
                *x = bg;
            }
        }
        s.tracks[d / 2][d / 2 + at] = v;
        for x in 0..width {
            s.result[x] = if x == at { v } else { bg };
        }
        s
    }

    #[test]
    fn resize_and_clear_and_shift() {
        let mut s = Scanline::default();
        s.resize(5, 8, true);
        assert_eq!(s.tracks.len(), 5);
        assert!(s.tracks.iter().all(|t| t.len() == 12));
        assert_eq!((s.alpha_ref.len(), s.result.len()), (8, 8));
        s.tracks[4][0] = 9.0;
        s.shift();
        assert_eq!(s.tracks[0][0], 9.0);
        s.clear();
        assert!(s.tracks.is_empty() && s.alpha_ref.is_empty() && s.result.is_empty());
        let mut s = Scanline::default();
        s.resize(3, 4, false);
        assert!(s.alpha_ref.is_empty());
    }

    #[test]
    fn max_spreads_a_bright_pixel_with_lens_weights() {
        let (mut lens, d) = setup(1.0, 1.0);
        let s = impulse(d, 9, 4, 1.0, 0.0);
        let mut result = s.result.clone();
        render(1.0, 1.0, 2, 0.0, false, &mut lens, &s.tracks, &[], &mut result);
        // the impulse itself and its neighbours at distance <= 1 (weight 1) are lit
        assert_eq!(result[4], 1.0);
        assert_eq!(result[3], 1.0);
        assert_eq!(result[5], 1.0);
        // distance 2 == outer radius: weight 0
        assert_eq!(result[2], 0.0);
        assert_eq!(result[6], 0.0);
    }

    #[test]
    fn min_spreads_a_dark_pixel() {
        let (mut lens, d) = setup(1.0, 1.0);
        let s = impulse(d, 9, 4, 0.0, 1.0);
        let mut result = s.result.clone();
        render(1.0, 1.0, 2, 0.0, true, &mut lens, &s.tracks, &[], &mut result);
        assert_eq!(result[4], 0.0);
        assert_eq!(result[3], 0.0);
        assert_eq!(result[5], 0.0);
        assert_eq!(result[2], 1.0);
    }

    #[test]
    fn values_never_decrease_for_max_and_never_increase_for_min() {
        let (mut lens, d) = setup(2.0, 1.0);
        let mut s = Scanline::default();
        s.resize(d, 12, false);
        let mut seed = 12345u32;
        for t in s.tracks.iter_mut() {
            for x in t.iter_mut() {
                seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
                *x = f64::from((seed >> 16) & 0xff) / 255.0;
            }
        }
        for x in 0..12 {
            s.result[x] = s.tracks[d / 2][d / 2 + x];
        }
        let mut mx = s.result.clone();
        render(2.0, 1.0, 2, 0.0, false, &mut lens, &s.tracks, &[], &mut mx);
        let mut mn = s.result.clone();
        render(2.0, 1.0, 2, 0.0, true, &mut lens, &s.tracks, &[], &mut mn);
        for x in 0..12 {
            assert!(mx[x] >= s.result[x]);
            assert!(mn[x] <= s.result[x] + 1e-12);
        }
    }

    #[test]
    fn alpha_ref_zero_leaves_pixels_and_scales_the_radius() {
        let (mut lens, d) = setup(2.0, 1.0);
        let mut s = impulse(d, 9, 4, 1.0, 0.0);
        s.alpha_ref = vec![1.0; 9];
        s.alpha_ref[3] = 0.0; // no effect at x = 3
        s.alpha_ref[5] = 0.25; // radius 0.5: outer 1 + 0.5*(0.5+1-1) = 1.25
        let mut result = s.result.clone();
        let ar = s.alpha_ref.clone();
        render(2.0, 1.0, 2, 0.0, false, &mut lens, &s.tracks, &ar, &mut result);
        assert_eq!(result[3], 0.0, "alpha_ref 0 -> unchanged");
        assert_eq!(result[4], 1.0);
        assert_eq!(result[6], 1.0, "full radius");
        // radius 0.25*2 = 0.5, outer 1.25: the neighbour at distance 1 gets a partial weight
        assert!(result[5] > 0.0 && result[5] < 1.0, "{}", result[5]);
    }

    #[test]
    fn empty_lens_leaves_the_result() {
        let mut lens = Lens::default();
        let tracks: Vec<Vec<f64>> = vec![];
        let mut result = vec![0.25, 0.5];
        render(0.0, 1.0, 2, 0.0, false, &mut lens, &tracks, &[], &mut result);
        assert_eq!(result, vec![0.25, 0.5]);
    }
}
