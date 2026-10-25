//! Port of the noise evaluation of `stdfx/perlinnoise.cpp`: `PerlinNoise::LinearNoise`,
//! `Turbolence` and `Marble` (with and without the min/max remap).
//!
//! This is a partial port: `perlinnoise.cpp` stays for the static tuning values
//! (`Size`, `TimeSize`, `Offset`, `Pixel_size`), the table fill (`TRandom`) and `doClouds`
//! (rasters, `TSpectrumParam`). The class in `perlinnoise.h` now forwards its four public
//! evaluators to the entry points below, handing over the table pointer and its dimensions.
//!
//! Evaluation order and operand types follow the C++ exactly (table is `float`, math in
//! `double`, no fused multiply-add), so results are bit-identical on the same libm; there is
//! no libm call in here at all. Deviation, only where the C++ read out of bounds: a table
//! index outside the buffer (negative `x % Size` for a negative lattice coordinate) yields
//! `0.0` for that corner instead of reading adjacent heap memory.

/// Lattice dimensions plus the table, validated once per call.
struct Table<'a> {
    noise: &'a [f32],
    size: i32,
    tsize: i32,
}

impl Table<'_> {
    #[inline]
    fn at(&self, it: i32, iy: i32, ix: i32) -> f64 {
        let idx = (it as i64) + (self.tsize as i64) * (iy as i64) + (self.tsize as i64) * (self.size as i64) * (ix as i64);
        if idx < 0 || idx as u64 >= self.noise.len() as u64 {
            0.0
        } else {
            self.noise[idx as usize] as f64
        }
    }

    /// `PerlinNoise::LinearNoise`.
    fn linear(&self, x: f64, y: f64, t: f64) -> f64 {
        let (ix, iy, it) = (x as i32, y as i32, t as i32);
        let dx = x - ix as f64;
        let dy = y - iy as f64;
        let dt = t - it as f64;
        let ix = ix % self.size;
        let iy = iy % self.size;
        let it = it % self.tsize;
        let ix1 = (ix + 1) % self.size;
        let iy1 = (iy + 1) % self.size;
        let it1 = (it + 1) % self.tsize;
        let val1 = self.at(it, iy, ix) + dx * (self.at(it, iy, ix1) - self.at(it, iy, ix));
        let val2 = self.at(it, iy1, ix) + dx * (self.at(it, iy1, ix1) - self.at(it, iy1, ix));
        let val3 = self.at(it1, iy, ix) + dx * (self.at(it1, iy, ix1) - self.at(it1, iy, ix));
        let val4 = self.at(it1, iy1, ix) + dx * (self.at(it1, iy1, ix1) - self.at(it1, iy1, ix));
        let val5 = val1 + dy * (val2 - val1);
        let val6 = val3 + dy * (val4 - val3);
        val5 + dt * (val6 - val5)
    }

    /// Shared octave loop of Turbolence / Marble; returns `(t, tscale)`.
    fn octaves(&self, mut u: f64, mut v: f64, mut k: f64, grain: f64, offset: i32) -> (f64, f64) {
        u += offset as f64;
        v += offset as f64;
        let pixel_size = 0.05;
        let (mut t, mut scale, mut tscale) = (0.0f64, 1.0f64, 0.0f64);
        u /= grain;
        v /= grain;
        k /= 10.0;
        while scale > pixel_size {
            tscale += scale;
            t += self.linear(u / scale, v / scale, k / scale) * scale;
            scale /= 2.0;
        }
        (t, tscale)
    }
}

fn remap(t: f64, min: f64, max: f64) -> f64 {
    if t < min {
        0.0
    } else if t > max {
        1.0
    } else {
        (t - min) / (max - min)
    }
}

/// Which evaluator `flare_perlin_eval` runs.
pub const PERLIN_TURBOLENCE: i32 = 0;
pub const PERLIN_MARBLE: i32 = 1;

/// Pure form of the four public evaluators. `ranged != 0` selects the min/max overloads.
fn eval(t: &Table, kind: i32, offset: i32, u: f64, v: f64, k: f64, grain: f64, ranged: bool, min: f64, max: f64) -> f64 {
    let (sum, tscale) = t.octaves(u, v, k, grain, offset);
    let r = if kind == PERLIN_MARBLE {
        let m = 10.0 * sum;
        m - (m as i32) as f64
    } else {
        sum / tscale
    };
    if ranged {
        remap(r, min, max)
    } else {
        r
    }
}

fn table<'a>(noise: *const f32, len: usize, size: i32, tsize: i32) -> Option<Table<'a>> {
    if noise.is_null() || size <= 0 || tsize <= 0 || len == 0 || len > (1usize << 28) {
        return None;
    }
    // SAFETY: non-null, length supplied (and bounded) by the caller.
    Some(Table { noise: unsafe { std::slice::from_raw_parts(noise, len) }, size, tsize })
}

/// `PerlinNoise::LinearNoise`. Returns NaN for a bad argument.
///
/// # Safety
/// `noise` must be valid for `len` floats.
#[no_mangle]
pub unsafe extern "C" fn flare_perlin_linear_noise(
    noise: *const f32, len: usize, size: i32, tsize: i32, x: f64, y: f64, t: f64,
) -> f64 {
    std::panic::catch_unwind(|| table(noise, len, size, tsize).map_or(f64::NAN, |tb| tb.linear(x, y, t)))
        .unwrap_or(f64::NAN)
}

/// `PerlinNoise::Turbolence` (kind 0) / `Marble` (kind 1), plain or `ranged` (min/max).
/// Returns NaN for a bad argument.
///
/// # Safety
/// `noise` must be valid for `len` floats.
#[no_mangle]
pub unsafe extern "C" fn flare_perlin_eval(
    noise: *const f32, len: usize, size: i32, tsize: i32, offset: i32, kind: i32, u: f64, v: f64, k: f64,
    grain: f64, ranged: i32, min: f64, max: f64,
) -> f64 {
    std::panic::catch_unwind(|| {
        if kind != PERLIN_TURBOLENCE && kind != PERLIN_MARBLE {
            return f64::NAN;
        }
        table(noise, len, size, tsize)
            .map_or(f64::NAN, |tb| eval(&tb, kind, offset, u, v, k, grain, ranged != 0, min, max))
    })
    .unwrap_or(f64::NAN)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tab(size: i32, tsize: i32) -> Vec<f32> {
        // deterministic stand-in for TRandom(1).getFloat()
        let mut s: u32 = 12345;
        (0..(size * size * tsize))
            .map(|_| {
                s = s.wrapping_mul(1664525).wrapping_add(1013904223);
                (s >> 8) as f32 / 16777216.0
            })
            .collect()
    }

    #[test]
    fn bad_args_are_nan() {
        unsafe {
            assert!(flare_perlin_linear_noise(std::ptr::null(), 4, 2, 2, 0.0, 0.0, 0.0).is_nan());
            let v = tab(4, 2);
            assert!(flare_perlin_linear_noise(v.as_ptr(), v.len(), 0, 2, 0.0, 0.0, 0.0).is_nan());
            assert!(flare_perlin_eval(v.as_ptr(), v.len(), 4, 2, 0, 7, 0.0, 0.0, 0.0, 1.0, 0, 0.0, 1.0).is_nan());
        }
    }

    #[test]
    fn lattice_points_return_table_values() {
        let v = tab(6, 3);
        let tb = Table { noise: &v, size: 6, tsize: 3 };
        // integer lattice coordinates reproduce the table entry exactly
        for (x, y, t) in [(0, 0, 0), (1, 2, 1), (5, 5, 2), (3, 0, 1)] {
            let want = v[(t + 3 * y + 3 * 6 * x) as usize] as f64;
            assert_eq!(tb.linear(x as f64, y as f64, t as f64), want);
        }
    }

    #[test]
    fn range_and_marble() {
        let v = tab(8, 4);
        unsafe {
            let a = flare_perlin_eval(v.as_ptr(), v.len(), 8, 4, 1000000, PERLIN_TURBOLENCE, 3.5, 2.25, 4.0, 10.0, 0, 0.0, 1.0);
            assert!((0.0..=1.0).contains(&a));
            let r = flare_perlin_eval(v.as_ptr(), v.len(), 8, 4, 1000000, PERLIN_TURBOLENCE, 3.5, 2.25, 4.0, 10.0, 1, 0.0, 1.0);
            assert_eq!(a, r);
            let m = flare_perlin_eval(v.as_ptr(), v.len(), 8, 4, 1000000, PERLIN_MARBLE, 3.5, 2.25, 4.0, 10.0, 0, 0.0, 1.0);
            assert!((0.0..1.0).contains(&m));
        }
    }

    /// Replays `tests/native/gen_perlinnoise_cases.py`: the original evaluators (g++,
    /// `-ffp-contract=off`) over three table configurations, compared bit for bit.
    #[test]
    fn matches_recorded_cpp() {
        let golden = include_str!("perlinnoise_cases.txt");
        let (mut size, mut tsize, mut offset) = (0, 0, 0);
        let mut tabv: Vec<f32> = vec![];
        let (mut cfgs, mut cases) = (0, 0);
        for line in golden.lines() {
            let f: Vec<&str> = line.split(' ').collect();
            if f[0] == "cfg" {
                size = f[1].parse().unwrap();
                tsize = f[2].parse().unwrap();
                offset = f[3].parse().unwrap();
                let mut s: u32 = 12345;
                tabv = (0..(size * size * tsize) as usize)
                    .map(|_| {
                        s = s.wrapping_mul(1664525).wrapping_add(1013904223);
                        ((s >> 8) as f64 / 16777216.0) as f32
                    })
                    .collect();
                cfgs += 1;
                continue;
            }
            assert_eq!(f[0], "c");
            let h: Vec<f64> = f[1..].iter().map(|x| f64::from_bits(u64::from_str_radix(x, 16).unwrap())).collect();
            let (u, v, k, grain) = (h[0], h[1], h[2], h[3]);
            let p = tabv.as_ptr();
            let n = tabv.len();
            unsafe {
                let got = [
                    flare_perlin_linear_noise(p, n, size, tsize, u, v, k),
                    flare_perlin_eval(p, n, size, tsize, offset, 0, u, v, k, grain, 0, 0.0, 1.0),
                    flare_perlin_eval(p, n, size, tsize, offset, 0, u, v, k, grain, 1, 0.2, 0.8),
                    flare_perlin_eval(p, n, size, tsize, offset, 1, u, v, k, grain, 0, 0.0, 1.0),
                    flare_perlin_eval(p, n, size, tsize, offset, 1, u, v, k, grain, 1, 0.1, 0.9),
                ];
                for (g, w) in got.iter().zip(&h[4..]) {
                    assert_eq!(g.to_bits(), w.to_bits(), "{line}");
                }
            }
            cases += 1;
        }
        assert_eq!((cfgs, cases), (3, 120));
    }
}
