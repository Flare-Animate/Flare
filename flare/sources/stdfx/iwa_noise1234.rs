//! Port of stdfx/iwa_noise1234.cpp: `Noise1234`, Stefan Gustavson's public domain
//! Perlin noise (1D..4D, plain and periodic), in `f32` exactly as the C++ computes it.
//!
//! Every arithmetic step keeps the C++ type: the lerps / fades are `f32`; the 2D
//! gradient adds an `f32` and a `double` product (`2.0 * v`) before narrowing, as the
//! original does; `FASTFLOOR` keeps its quirk (`x == 0` and negative whole numbers
//! round one lower). No libm is involved, so results are bit-identical to the C++
//! (recorded goldens below, built with `-ffp-contract=off`).
//!
//! Deviations (only where the C++ was undefined behaviour): float -> int casts
//! saturate (`as i32`) instead of being UB for out-of-range / NaN input; `ix + 1`
//! wraps instead of overflowing; a period of 0 in `pnoise` (division by zero, SIGFPE
//! in the C++) returns 0.0.
//!
//! The header `iwa_noise1234.h` keeps the `Noise1234` class with its public static
//! signatures as inline forwarders to the `flare_noise1234_*` exports below.
//! `igs_perlin_noise.rs` still carries its own private copy of the 3D kernel.

// Permutation table (512 entries: the 256-entry jumble twice), generated from the C++.
const PERM1234: [u8; 512] = [
    151, 160, 137, 91, 90, 15, 131, 13, 201, 95, 96, 53, 194, 233, 7, 225,
    140, 36, 103, 30, 69, 142, 8, 99, 37, 240, 21, 10, 23, 190, 6, 148,
    247, 120, 234, 75, 0, 26, 197, 62, 94, 252, 219, 203, 117, 35, 11, 32,
    57, 177, 33, 88, 237, 149, 56, 87, 174, 20, 125, 136, 171, 168, 68, 175,
    74, 165, 71, 134, 139, 48, 27, 166, 77, 146, 158, 231, 83, 111, 229, 122,
    60, 211, 133, 230, 220, 105, 92, 41, 55, 46, 245, 40, 244, 102, 143, 54,
    65, 25, 63, 161, 1, 216, 80, 73, 209, 76, 132, 187, 208, 89, 18, 169,
    200, 196, 135, 130, 116, 188, 159, 86, 164, 100, 109, 198, 173, 186, 3, 64,
    52, 217, 226, 250, 124, 123, 5, 202, 38, 147, 118, 126, 255, 82, 85, 212,
    207, 206, 59, 227, 47, 16, 58, 17, 182, 189, 28, 42, 223, 183, 170, 213,
    119, 248, 152, 2, 44, 154, 163, 70, 221, 153, 101, 155, 167, 43, 172, 9,
    129, 22, 39, 253, 19, 98, 108, 110, 79, 113, 224, 232, 178, 185, 112, 104,
    218, 246, 97, 228, 251, 34, 242, 193, 238, 210, 144, 12, 191, 179, 162, 241,
    81, 51, 145, 235, 249, 14, 239, 107, 49, 192, 214, 31, 181, 199, 106, 157,
    184, 84, 204, 176, 115, 121, 50, 45, 127, 4, 150, 254, 138, 236, 205, 93,
    222, 114, 67, 29, 24, 72, 243, 141, 128, 195, 78, 66, 215, 61, 156, 180,
    151, 160, 137, 91, 90, 15, 131, 13, 201, 95, 96, 53, 194, 233, 7, 225,
    140, 36, 103, 30, 69, 142, 8, 99, 37, 240, 21, 10, 23, 190, 6, 148,
    247, 120, 234, 75, 0, 26, 197, 62, 94, 252, 219, 203, 117, 35, 11, 32,
    57, 177, 33, 88, 237, 149, 56, 87, 174, 20, 125, 136, 171, 168, 68, 175,
    74, 165, 71, 134, 139, 48, 27, 166, 77, 146, 158, 231, 83, 111, 229, 122,
    60, 211, 133, 230, 220, 105, 92, 41, 55, 46, 245, 40, 244, 102, 143, 54,
    65, 25, 63, 161, 1, 216, 80, 73, 209, 76, 132, 187, 208, 89, 18, 169,
    200, 196, 135, 130, 116, 188, 159, 86, 164, 100, 109, 198, 173, 186, 3, 64,
    52, 217, 226, 250, 124, 123, 5, 202, 38, 147, 118, 126, 255, 82, 85, 212,
    207, 206, 59, 227, 47, 16, 58, 17, 182, 189, 28, 42, 223, 183, 170, 213,
    119, 248, 152, 2, 44, 154, 163, 70, 221, 153, 101, 155, 167, 43, 172, 9,
    129, 22, 39, 253, 19, 98, 108, 110, 79, 113, 224, 232, 178, 185, 112, 104,
    218, 246, 97, 228, 251, 34, 242, 193, 238, 210, 144, 12, 191, 179, 162, 241,
    81, 51, 145, 235, 249, 14, 239, 107, 49, 192, 214, 31, 181, 199, 106, 157,
    184, 84, 204, 176, 115, 121, 50, 45, 127, 4, 150, 254, 138, 236, 205, 93,
    222, 114, 67, 29, 24, 72, 243, 141, 128, 195, 78, 66, 215, 61, 156, 180,
];

#[inline]
fn p(i: i32) -> i32 {
    i32::from(PERM1234[i as usize])
}

#[inline]
fn fastfloor(x: f32) -> i32 {
    if x > 0.0 {
        x as i32
    } else {
        (x as i32).wrapping_sub(1)
    }
}

#[inline]
fn fade(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

#[inline]
fn lerp(t: f32, a: f32, b: f32) -> f32 {
    a + t * (b - a)
}

fn grad1(hash: i32, x: f32) -> f32 {
    let h = hash & 15;
    let mut grad: f32 = (1.0 + f64::from(h & 7)) as f32;
    if h & 8 != 0 {
        grad = -grad;
    }
    grad * x
}

fn grad2(hash: i32, x: f32, y: f32) -> f32 {
    let h = hash & 7;
    let u = if h < 4 { x } else { y };
    let v = if h < 4 { y } else { x };
    let a = if h & 1 != 0 { -u } else { u };
    let b: f64 = if h & 2 != 0 { -2.0 * f64::from(v) } else { 2.0 * f64::from(v) };
    (f64::from(a) + b) as f32
}

fn grad3(hash: i32, x: f32, y: f32, z: f32) -> f32 {
    let h = hash & 15;
    let u = if h < 8 { x } else { y };
    let v = if h < 4 {
        y
    } else if h == 12 || h == 14 {
        x
    } else {
        z
    };
    (if h & 1 != 0 { -u } else { u }) + (if h & 2 != 0 { -v } else { v })
}

fn grad4(hash: i32, x: f32, y: f32, z: f32, t: f32) -> f32 {
    let h = hash & 31;
    let u = if h < 24 { x } else { y };
    let v = if h < 16 { y } else { z };
    let w = if h < 8 { z } else { t };
    (if h & 1 != 0 { -u } else { u }) + (if h & 2 != 0 { -v } else { v }) + (if h & 4 != 0 { -w } else { w })
}

fn core1(fx0: f32, ix0: i32, ix1: i32) -> f32 {
    let fx1 = fx0 - 1.0;
    let s = fade(fx0);
    let n0 = grad1(p(ix0), fx0);
    let n1 = grad1(p(ix1), fx1);
    0.188f32 * lerp(s, n0, n1)
}

fn core2(fx0: f32, fy0: f32, ix0: i32, iy0: i32, ix1: i32, iy1: i32) -> f32 {
    let fx1 = fx0 - 1.0;
    let fy1 = fy0 - 1.0;
    let t = fade(fy0);
    let s = fade(fx0);
    let mut nx0 = grad2(p(ix0 + p(iy0)), fx0, fy0);
    let mut nx1 = grad2(p(ix0 + p(iy1)), fx0, fy1);
    let n0 = lerp(t, nx0, nx1);
    nx0 = grad2(p(ix1 + p(iy0)), fx1, fy0);
    nx1 = grad2(p(ix1 + p(iy1)), fx1, fy1);
    let n1 = lerp(t, nx0, nx1);
    0.507f32 * lerp(s, n0, n1)
}

#[allow(clippy::too_many_arguments)]
fn core3(fx0: f32, fy0: f32, fz0: f32, ix0: i32, iy0: i32, iz0: i32, ix1: i32, iy1: i32, iz1: i32) -> f32 {
    let fx1 = fx0 - 1.0;
    let fy1 = fy0 - 1.0;
    let fz1 = fz0 - 1.0;
    let r = fade(fz0);
    let t = fade(fy0);
    let s = fade(fx0);
    let h = |ix: i32, iy: i32, iz: i32| p(ix + p(iy + p(iz)));

    let mut nxy0 = grad3(h(ix0, iy0, iz0), fx0, fy0, fz0);
    let mut nxy1 = grad3(h(ix0, iy0, iz1), fx0, fy0, fz1);
    let mut nx0 = lerp(r, nxy0, nxy1);
    nxy0 = grad3(h(ix0, iy1, iz0), fx0, fy1, fz0);
    nxy1 = grad3(h(ix0, iy1, iz1), fx0, fy1, fz1);
    let mut nx1 = lerp(r, nxy0, nxy1);
    let n0 = lerp(t, nx0, nx1);

    nxy0 = grad3(h(ix1, iy0, iz0), fx1, fy0, fz0);
    nxy1 = grad3(h(ix1, iy0, iz1), fx1, fy0, fz1);
    nx0 = lerp(r, nxy0, nxy1);
    nxy0 = grad3(h(ix1, iy1, iz0), fx1, fy1, fz0);
    nxy1 = grad3(h(ix1, iy1, iz1), fx1, fy1, fz1);
    nx1 = lerp(r, nxy0, nxy1);
    let n1 = lerp(t, nx0, nx1);

    0.936f32 * lerp(s, n0, n1)
}

#[allow(clippy::too_many_arguments)]
fn core4(fx0: f32, fy0: f32, fz0: f32, fw0: f32, ix0: i32, iy0: i32, iz0: i32, iw0: i32, ix1: i32, iy1: i32, iz1: i32, iw1: i32) -> f32 {
    let fx1 = fx0 - 1.0;
    let fy1 = fy0 - 1.0;
    let fz1 = fz0 - 1.0;
    let fw1 = fw0 - 1.0;
    let q = fade(fw0);
    let r = fade(fz0);
    let t = fade(fy0);
    let s = fade(fx0);
    let h = |ix: i32, iy: i32, iz: i32, iw: i32| p(ix + p(iy + p(iz + p(iw))));

    // x0 half
    let mut nxyz0 = grad4(h(ix0, iy0, iz0, iw0), fx0, fy0, fz0, fw0);
    let mut nxyz1 = grad4(h(ix0, iy0, iz0, iw1), fx0, fy0, fz0, fw1);
    let mut nxy0 = lerp(q, nxyz0, nxyz1);
    nxyz0 = grad4(h(ix0, iy0, iz1, iw0), fx0, fy0, fz1, fw0);
    nxyz1 = grad4(h(ix0, iy0, iz1, iw1), fx0, fy0, fz1, fw1);
    let mut nxy1 = lerp(q, nxyz0, nxyz1);
    let mut nx0 = lerp(r, nxy0, nxy1);

    nxyz0 = grad4(h(ix0, iy1, iz0, iw0), fx0, fy1, fz0, fw0);
    nxyz1 = grad4(h(ix0, iy1, iz0, iw1), fx0, fy1, fz0, fw1);
    nxy0 = lerp(q, nxyz0, nxyz1);
    nxyz0 = grad4(h(ix0, iy1, iz1, iw0), fx0, fy1, fz1, fw0);
    nxyz1 = grad4(h(ix0, iy1, iz1, iw1), fx0, fy1, fz1, fw1);
    nxy1 = lerp(q, nxyz0, nxyz1);
    let mut nx1 = lerp(r, nxy0, nxy1);

    let n0 = lerp(t, nx0, nx1);

    // x1 half
    nxyz0 = grad4(h(ix1, iy0, iz0, iw0), fx1, fy0, fz0, fw0);
    nxyz1 = grad4(h(ix1, iy0, iz0, iw1), fx1, fy0, fz0, fw1);
    nxy0 = lerp(q, nxyz0, nxyz1);
    nxyz0 = grad4(h(ix1, iy0, iz1, iw0), fx1, fy0, fz1, fw0);
    nxyz1 = grad4(h(ix1, iy0, iz1, iw1), fx1, fy0, fz1, fw1);
    nxy1 = lerp(q, nxyz0, nxyz1);
    nx0 = lerp(r, nxy0, nxy1);

    nxyz0 = grad4(h(ix1, iy1, iz0, iw0), fx1, fy1, fz0, fw0);
    nxyz1 = grad4(h(ix1, iy1, iz0, iw1), fx1, fy1, fz0, fw1);
    nxy0 = lerp(q, nxyz0, nxyz1);
    nxyz0 = grad4(h(ix1, iy1, iz1, iw0), fx1, fy1, fz1, fw0);
    nxyz1 = grad4(h(ix1, iy1, iz1, iw1), fx1, fy1, fz1, fw1);
    nxy1 = lerp(q, nxyz0, nxyz1);
    nx1 = lerp(r, nxy0, nxy1);

    let n1 = lerp(t, nx0, nx1);

    0.87f32 * lerp(s, n0, n1)
}

#[inline]
fn frac(x: f32, i: i32) -> f32 {
    x - i as f32
}

#[inline]
fn pw(i: i32, period: i32) -> i32 {
    i.wrapping_rem(period) & 0xff
}

pub fn noise1(x: f32) -> f32 {
    let ix0 = fastfloor(x);
    core1(frac(x, ix0), ix0 & 0xff, ix0.wrapping_add(1) & 0xff)
}

pub fn noise2(x: f32, y: f32) -> f32 {
    let (ix0, iy0) = (fastfloor(x), fastfloor(y));
    core2(frac(x, ix0), frac(y, iy0), ix0 & 0xff, iy0 & 0xff, ix0.wrapping_add(1) & 0xff, iy0.wrapping_add(1) & 0xff)
}

pub fn noise3(x: f32, y: f32, z: f32) -> f32 {
    let (ix0, iy0, iz0) = (fastfloor(x), fastfloor(y), fastfloor(z));
    core3(
        frac(x, ix0),
        frac(y, iy0),
        frac(z, iz0),
        ix0 & 0xff,
        iy0 & 0xff,
        iz0 & 0xff,
        ix0.wrapping_add(1) & 0xff,
        iy0.wrapping_add(1) & 0xff,
        iz0.wrapping_add(1) & 0xff,
    )
}

pub fn noise4(x: f32, y: f32, z: f32, w: f32) -> f32 {
    let (ix0, iy0, iz0, iw0) = (fastfloor(x), fastfloor(y), fastfloor(z), fastfloor(w));
    core4(
        frac(x, ix0),
        frac(y, iy0),
        frac(z, iz0),
        frac(w, iw0),
        ix0 & 0xff,
        iy0 & 0xff,
        iz0 & 0xff,
        iw0 & 0xff,
        ix0.wrapping_add(1) & 0xff,
        iy0.wrapping_add(1) & 0xff,
        iz0.wrapping_add(1) & 0xff,
        iw0.wrapping_add(1) & 0xff,
    )
}

pub fn pnoise1(x: f32, px: i32) -> f32 {
    if px == 0 {
        return 0.0;
    }
    let ix0 = fastfloor(x);
    core1(frac(x, ix0), pw(ix0, px), pw(ix0.wrapping_add(1), px))
}

pub fn pnoise2(x: f32, y: f32, px: i32, py: i32) -> f32 {
    if px == 0 || py == 0 {
        return 0.0;
    }
    let (ix0, iy0) = (fastfloor(x), fastfloor(y));
    core2(frac(x, ix0), frac(y, iy0), pw(ix0, px), pw(iy0, py), pw(ix0.wrapping_add(1), px), pw(iy0.wrapping_add(1), py))
}

pub fn pnoise3(x: f32, y: f32, z: f32, px: i32, py: i32, pz: i32) -> f32 {
    if px == 0 || py == 0 || pz == 0 {
        return 0.0;
    }
    let (ix0, iy0, iz0) = (fastfloor(x), fastfloor(y), fastfloor(z));
    core3(
        frac(x, ix0),
        frac(y, iy0),
        frac(z, iz0),
        pw(ix0, px),
        pw(iy0, py),
        pw(iz0, pz),
        pw(ix0.wrapping_add(1), px),
        pw(iy0.wrapping_add(1), py),
        pw(iz0.wrapping_add(1), pz),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn pnoise4(x: f32, y: f32, z: f32, w: f32, px: i32, py: i32, pz: i32, pw_: i32) -> f32 {
    if px == 0 || py == 0 || pz == 0 || pw_ == 0 {
        return 0.0;
    }
    let (ix0, iy0, iz0, iw0) = (fastfloor(x), fastfloor(y), fastfloor(z), fastfloor(w));
    core4(
        frac(x, ix0),
        frac(y, iy0),
        frac(z, iz0),
        frac(w, iw0),
        pw(ix0, px),
        pw(iy0, py),
        pw(iz0, pz),
        pw(iw0, pw_),
        pw(ix0.wrapping_add(1), px),
        pw(iy0.wrapping_add(1), py),
        pw(iz0.wrapping_add(1), pz),
        pw(iw0.wrapping_add(1), pw_),
    )
}

// ---- C ABI (scalar in, scalar out: nothing to null-check; panics are caught) ----

#[no_mangle]
pub extern "C" fn flare_noise1234_noise1(x: f32) -> f32 {
    crate::guard(0.0, || noise1(x))
}
#[no_mangle]
pub extern "C" fn flare_noise1234_noise2(x: f32, y: f32) -> f32 {
    crate::guard(0.0, || noise2(x, y))
}
#[no_mangle]
pub extern "C" fn flare_noise1234_noise3(x: f32, y: f32, z: f32) -> f32 {
    crate::guard(0.0, || noise3(x, y, z))
}
#[no_mangle]
pub extern "C" fn flare_noise1234_noise4(x: f32, y: f32, z: f32, w: f32) -> f32 {
    crate::guard(0.0, || noise4(x, y, z, w))
}
#[no_mangle]
pub extern "C" fn flare_noise1234_pnoise1(x: f32, px: i32) -> f32 {
    crate::guard(0.0, || pnoise1(x, px))
}
#[no_mangle]
pub extern "C" fn flare_noise1234_pnoise2(x: f32, y: f32, px: i32, py: i32) -> f32 {
    crate::guard(0.0, || pnoise2(x, y, px, py))
}
#[no_mangle]
pub extern "C" fn flare_noise1234_pnoise3(x: f32, y: f32, z: f32, px: i32, py: i32, pz: i32) -> f32 {
    crate::guard(0.0, || pnoise3(x, y, z, px, py, pz))
}
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub extern "C" fn flare_noise1234_pnoise4(x: f32, y: f32, z: f32, w: f32, px: i32, py: i32, pz: i32, pw: i32) -> f32 {
    crate::guard(0.0, || pnoise4(x, y, z, w, px, py, pz, pw))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_is_the_jumble_twice() {
        assert_eq!(PERM1234[..256], PERM1234[256..]);
        let mut seen = [false; 256];
        for &v in &PERM1234[..256] {
            seen[v as usize] = true;
        }
        assert!(seen.iter().all(|b| *b));
    }

    #[test]
    fn fastfloor_keeps_the_zero_and_negative_whole_quirk() {
        assert_eq!(fastfloor(0.0), -1);
        assert_eq!(fastfloor(1.5), 1);
        assert_eq!(fastfloor(-2.0), -3);
        assert_eq!(fastfloor(-2.5), -3);
    }

    #[test]
    fn lattice_points_are_zero_for_gradient_noise() {
        // at x = 3.0 the fractional part is 0, so every gradient dot product is 0 or from fx1
        assert_eq!(noise1(3.0), 0.0);
        assert_eq!(noise2(3.0, 5.0), 0.0);
    }

    #[test]
    fn periodic_noise_repeats_with_its_period() {
        for i in 0..20 {
            let x = i as f32 * 0.25 + 0.125; // dyadic: x + period is exact
            assert_eq!(pnoise1(x, 4), pnoise1(x + 4.0, 4));
            assert_eq!(pnoise2(x, 1.5, 3, 5), pnoise2(x + 3.0, 1.5 + 5.0, 3, 5));
        }
    }

    #[test]
    fn zero_period_does_not_trap() {
        assert_eq!(pnoise1(1.5, 0), 0.0);
        assert_eq!(pnoise4(1.0, 2.0, 3.0, 4.0, 1, 1, 0, 1), 0.0);
    }

    #[test]
    fn ffi_matches_rust() {
        assert_eq!(flare_noise1234_noise3(1.25, 2.5, 3.75), noise3(1.25, 2.5, 3.75));
        assert_eq!(flare_noise1234_pnoise4(0.5, 1.5, 2.5, 3.5, 2, 3, 4, 5), pnoise4(0.5, 1.5, 2.5, 3.5, 2, 3, 4, 5));
    }

    // Replays cases recorded from the ORIGINAL iwa_noise1234.cpp (tests/native/gen_noise_cases.py).
    // Line: `<kind> <f32 bits of the args in hex...> <ints...> <f32 result bits>`
    #[test]
    fn matches_recorded_cpp() {
        let mut n = 0;
        for line in GOLDEN.lines() {
            let t: Vec<&str> = line.split_whitespace().collect();
            let f = |s: &str| f32::from_bits(u32::from_str_radix(s, 16).unwrap());
            let i = |s: &str| s.parse::<i32>().unwrap();
            let (got, want) = match t[0] {
                "N1" => (noise1(f(t[1])), f(t[2])),
                "N2" => (noise2(f(t[1]), f(t[2])), f(t[3])),
                "N3" => (noise3(f(t[1]), f(t[2]), f(t[3])), f(t[4])),
                "N4" => (noise4(f(t[1]), f(t[2]), f(t[3]), f(t[4])), f(t[5])),
                "P1" => (pnoise1(f(t[1]), i(t[2])), f(t[3])),
                "P2" => (pnoise2(f(t[1]), f(t[2]), i(t[3]), i(t[4])), f(t[5])),
                "P3" => (pnoise3(f(t[1]), f(t[2]), f(t[3]), i(t[4]), i(t[5]), i(t[6])), f(t[7])),
                "P4" => (pnoise4(f(t[1]), f(t[2]), f(t[3]), f(t[4]), i(t[5]), i(t[6]), i(t[7]), i(t[8])), f(t[9])),
                k => panic!("bad kind {k}"),
            };
            assert_eq!(got.to_bits(), want.to_bits(), "{line}");
            n += 1;
        }
        assert!(n > 1000, "{n}");
    }

    const GOLDEN: &str = include_str!("iwa_noise1234_cases.txt");
}
