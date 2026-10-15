//! Port of stdfx/igs_perlin_noise.cpp: fill a whole 8/16-bit or float image with
//! 3D Perlin noise (octave sum, affine-mapped pixel coordinates).
//!
//! `iwa_noise1234.cpp` stays C++ (its `Noise1234::noise` is called per pixel from
//! `iwa_fractalnoisefx`/`iwa_pnperspectivefx`, so an FFI hop per sample is not worth it).
//! This file therefore carries a private, bit-identical copy of the 3D case
//! (`Noise1234::noise(x, y, z)`, Stefan Gustavson, public domain), which the
//! whole-buffer call uses without leaving Rust. Channel order is Toonz6 (b,g,r,a).

const PERM: [u8; 512] = {
    const BASE: [u8; 256] = [
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
    let mut p = [0u8; 512];
    let mut i = 0;
    while i < 512 {
        p[i] = BASE[i & 255];
        i += 1;
    }
    p
};

const ALP: usize = 3;

/// status codes: 0 ok, 1 "Bad bits,Not uchar/ushort" (std::domain_error), 2 invalid argument
pub const OK: i32 = 0;
pub const BAD_BITS: i32 = 1;
pub const INVALID_ARG: i32 = 2;

#[inline]
fn fade(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}
#[inline]
fn lerp(t: f32, a: f32, b: f32) -> f32 {
    a + t * (b - a)
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
fn grad3(hash: u8, x: f32, y: f32, z: f32) -> f32 {
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
#[inline]
fn p(i: i32) -> i32 {
    PERM[i as usize] as i32
}

/// Noise1234::noise(float x, float y, float z)
pub fn noise3(x: f32, y: f32, z: f32) -> f32 {
    let mut ix0 = fastfloor(x);
    let mut iy0 = fastfloor(y);
    let mut iz0 = fastfloor(z);
    let fx0 = x - ix0 as f32;
    let fy0 = y - iy0 as f32;
    let fz0 = z - iz0 as f32;
    let fx1 = fx0 - 1.0;
    let fy1 = fy0 - 1.0;
    let fz1 = fz0 - 1.0;
    let ix1 = ix0.wrapping_add(1) & 0xff;
    let iy1 = iy0.wrapping_add(1) & 0xff;
    let iz1 = iz0.wrapping_add(1) & 0xff;
    ix0 &= 0xff;
    iy0 &= 0xff;
    iz0 &= 0xff;

    let r = fade(fz0);
    let t = fade(fy0);
    let s = fade(fx0);

    let g = |a: i32, b: i32, c: i32, fx: f32, fy: f32, fz: f32| grad3(PERM[(a + p(b + p(c))) as usize], fx, fy, fz);

    let nxy0 = g(ix0, iy0, iz0, fx0, fy0, fz0);
    let nxy1 = g(ix0, iy0, iz1, fx0, fy0, fz1);
    let nx0 = lerp(r, nxy0, nxy1);
    let nxy0 = g(ix0, iy1, iz0, fx0, fy1, fz0);
    let nxy1 = g(ix0, iy1, iz1, fx0, fy1, fz1);
    let nx1 = lerp(r, nxy0, nxy1);
    let n0 = lerp(t, nx0, nx1);

    let nxy0 = g(ix1, iy0, iz0, fx1, fy0, fz0);
    let nxy1 = g(ix1, iy0, iz1, fx1, fy0, fz1);
    let nx0 = lerp(r, nxy0, nxy1);
    let nxy0 = g(ix1, iy1, iz0, fx1, fy1, fz0);
    let nxy1 = g(ix1, iy1, iz1, fx1, fy1, fz1);
    let nx1 = lerp(r, nxy0, nxy1);
    let n1 = lerp(t, nx0, nx1);

    0.936f32 * lerp(s, n0, n1)
}

fn perlin_noise_3d(x: f64, y: f64, z: f64, octaves_start: i32, octaves_end: i32, persistence: f64) -> f64 {
    let mut total = 0.0;
    let mut ii = octaves_start;
    while ii <= octaves_end {
        let frequency = 2.0f64.powf(ii as f64);
        let amplitude = persistence.powf(ii as f64);
        total += noise3((x * frequency) as f32, (y * frequency) as f32, (z * frequency) as f32) as f64 * amplitude;
        if ii == i32::MAX {
            break;
        }
        ii += 1;
    }
    total
}

fn perlin_noise_minmax(octaves_start: i32, octaves_end: i32, persistence: f64) -> f64 {
    let mut total = 0.0;
    let mut ii = octaves_start;
    while ii <= octaves_end {
        total += persistence.powf(ii as f64);
        if ii == i32::MAX {
            break;
        }
        ii += 1;
    }
    total
}

#[derive(Clone, Copy)]
struct Params {
    a: [f64; 6], // a11 a12 a13 a21 a22 a23
    zz: f64,
    octaves_start: i32,
    octaves_end: i32,
    persistence: f64,
}

trait Sample: Copy {
    /// (value for colour channels, value for an opaque alpha)
    fn encode(noise: f64, maxi: f64) -> (Self, Self);
}
macro_rules! int_sample {
    ($t:ty) => {
        impl Sample for $t {
            fn encode(noise: f64, maxi: f64) -> ($t, $t) {
                let max_div = <$t>::MAX as i32;
                let max_div_2 = max_div / 2;
                let max_mul = max_div_2 as f64 + 0.499999;
                let max_off = max_div_2 as f64 + 1.5;
                ((noise / maxi * max_mul + max_off) as $t, max_div as $t)
            }
        }
    };
}
int_sample!(u8);
int_sample!(u16);
impl Sample for f32 {
    fn encode(noise: f64, maxi: f64) -> (f32, f32) {
        ((noise / maxi * 0.5f32 as f64 + 0.5f32 as f64) as f32, 1.0)
    }
}

unsafe fn change_t<T: Sample>(image: *mut T, height: i32, width: i32, wrap: i32, channels: i32, alpha_rendering_sw: bool, q: Params) {
    let maxi = perlin_noise_minmax(q.octaves_start, q.octaves_end, q.persistence);
    let [a11, a12, a13, a21, a22, a23] = q.a;
    let mut scanline = image;
    for yy in 0..height {
        let mut crnt = scanline;
        for xx in 0..width {
            let (xf, yf) = (xx as f64, yy as f64);
            let n = perlin_noise_3d(xf * a11 + yf * a12 + a13, xf * a21 + yf * a22 + a23, q.zz, q.octaves_start, q.octaves_end, q.persistence);
            let (val, opaque) = T::encode(n, maxi);
            for zz in 0..channels as usize {
                *crnt.add(zz) = if !alpha_rendering_sw && ALP == zz { opaque } else { val };
            }
            crnt = crnt.offset(channels as isize);
        }
        scanline = scanline.offset(channels as isize * wrap as isize);
    }
}

/// igs::perlin_noise::change
///
/// # Safety
/// `image_array` must cover `(height-1)*wrap + width` pixels of `channels` samples of
/// `bits` bits (aligned for 16-bit / float samples).
#[allow(clippy::too_many_arguments)]
pub unsafe fn change(
    image_array: *mut u8, height: i32, width: i32, wrap: i32, channels: i32, bits: i32, alpha_rendering_sw: bool,
    a11: f64, a12: f64, a13: f64, a21: f64, a22: f64, a23: f64, zz: f64, octaves_start: i32, octaves_end: i32, persistence: f64,
) -> i32 {
    if image_array.is_null() || height < 0 || width < 0 || wrap < 0 || channels <= 0 {
        return INVALID_ARG;
    }
    let q = Params { a: [a11, a12, a13, a21, a22, a23], zz, octaves_start, octaves_end, persistence };
    match bits {
        8 => change_t(image_array, height, width, wrap, channels, alpha_rendering_sw, q),
        16 => {
            if image_array as usize % 2 != 0 {
                return INVALID_ARG;
            }
            change_t(image_array as *mut u16, height, width, wrap, channels, alpha_rendering_sw, q)
        }
        24 => {
            if image_array as usize % 4 != 0 {
                return INVALID_ARG;
            }
            change_t(image_array as *mut f32, height, width, wrap, channels, alpha_rendering_sw, q)
        }
        _ => return BAD_BITS,
    }
    OK
}

/// Returns 0, 1 (header throws std::domain_error) or 2 (std::invalid_argument).
///
/// # Safety
/// See [`change`].
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn flare_igs_perlin_noise_change(
    image_array: *mut u8, height: i32, width: i32, wrap: i32, channels: i32, bits: i32, alpha_rendering_sw: bool,
    a11: f64, a12: f64, a13: f64, a21: f64, a22: f64, a23: f64, zz: f64, octaves_start: i32, octaves_end: i32, persistence: f64,
) -> i32 {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        change(image_array, height, width, wrap, channels, bits, alpha_rendering_sw, a11, a12, a13, a21, a22, a23, zz, octaves_start, octaves_end, persistence)
    }))
    .unwrap_or(INVALID_ARG)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run8(w: i32, h: i32, ch: i32, alpha: bool) -> Vec<u8> {
        let mut v = vec![0u8; (w * h * ch) as usize];
        let rc = unsafe { change(v.as_mut_ptr(), h, w, w, ch, 8, alpha, 0.1, 0.02, 0.3, -0.03, 0.12, 5.5, 0.7, 0, 4, 1.0 / 1.7320508) };
        assert_eq!(rc, OK);
        v
    }

    #[test]
    fn perm_table_spot_checks() {
        assert_eq!((PERM[0], PERM[1], PERM[255], PERM[256], PERM[511]), (151, 160, 180, 151, 180));
    }

    #[test]
    fn noise3_properties() {
        // integer lattice points have zero noise (all residual vectors are zero or cancel)
        assert_eq!(noise3(3.0, 4.0, 5.0), 0.0);
        let mut seen = false;
        for i in 0..200 {
            let n = noise3(i as f32 * 0.37, i as f32 * 0.11 + 0.3, 1.7);
            assert!(n.abs() < 1.5);
            seen |= n != 0.0;
        }
        assert!(seen);
    }

    #[test]
    fn rgba8_channels_equal_and_alpha_opaque_when_not_rendered() {
        let v = run8(9, 7, 4, false);
        for px in v.chunks(4) {
            assert_eq!(px[0], px[1]);
            assert_eq!(px[1], px[2]);
            assert_eq!(px[3], 255);
        }
        assert!(v.chunks(4).any(|px| px[0] != v[0]));
        let w = run8(9, 7, 4, true);
        for (a, b) in v.chunks(4).zip(w.chunks(4)) {
            assert_eq!(a[..3], b[..3]);
            assert_eq!(b[3], b[0]);
        }
    }

    #[test]
    fn deterministic_and_wrap_skips_padding() {
        assert_eq!(run8(5, 5, 4, true), run8(5, 5, 4, true));
        // wrap larger than width: padding samples are untouched
        let (w, h, wrap, ch) = (3, 2, 5, 1);
        let mut v = vec![77u8; (wrap * h * ch) as usize];
        assert_eq!(unsafe { change(v.as_mut_ptr(), h, w, wrap, ch, 8, true, 0.2, 0.0, 0.0, 0.0, 0.2, 0.0, 0.5, 0, 2, 0.5) }, OK);
        assert_eq!(&v[3..5], &[77, 77]);
        assert_eq!(&v[8..10], &[77, 77]);
        assert_ne!(&v[0..3], &[77, 77, 77]);
    }

    #[test]
    fn sixteen_bit_and_float() {
        let mut v = vec![0u16; 4 * 4 * 4];
        assert_eq!(unsafe { change(v.as_mut_ptr() as *mut u8, 4, 4, 4, 4, 16, false, 0.3, 0.0, 0.1, 0.0, 0.3, 0.2, 0.0, 0, 3, 0.5) }, OK);
        assert!(v.chunks(4).all(|p| p[3] == 65535 && p[0] == p[1]));
        let mut f = vec![0f32; 4 * 4 * 4];
        assert_eq!(unsafe { change(f.as_mut_ptr() as *mut u8, 4, 4, 4, 4, 24, false, 0.3, 0.0, 0.1, 0.0, 0.3, 0.2, 0.0, 0, 3, 0.5) }, OK);
        assert!(f.chunks(4).all(|p| p[3] == 1.0 && p[0] >= 0.0 && p[0] <= 1.0));
    }

    #[test]
    fn errors_and_null() {
        let mut b = [0u8; 16];
        assert_eq!(unsafe { change(b.as_mut_ptr(), 2, 2, 2, 4, 32, true, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0, 1, 0.5) }, BAD_BITS);
        assert_eq!(unsafe { change(std::ptr::null_mut(), 2, 2, 2, 4, 8, true, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0, 1, 0.5) }, INVALID_ARG);
        assert_eq!(unsafe { change(b.as_mut_ptr(), -1, 2, 2, 4, 8, true, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0, 1, 0.5) }, INVALID_ARG);
        assert_eq!(unsafe { change(b.as_mut_ptr(), 1, 1, 1, 0, 8, true, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0, 1, 0.5) }, INVALID_ARG);
        // empty image is fine
        assert_eq!(unsafe { change(b.as_mut_ptr(), 0, 0, 0, 4, 8, true, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0, 1, 0.5) }, OK);
    }

    // Golden values generated by the original C++ (igs_perlin_noise.cpp + iwa_noise1234.cpp at 07930cf^, g++ -O0).
    #[test]
    fn golden_u8_16x16() {
        let v = run8_16();
        let px = |k: usize| v[k * 4];
        let exp = [(0, 142), (1, 141), (2, 150), (3, 156), (17, 128), (50, 113), (100, 134), (128, 130), (200, 143), (255, 109)];
        for (k, e) in exp {
            assert_eq!(px(k), e, "pixel {k}");
        }
        let mut h: u32 = 2166136261;
        for b in &v {
            h ^= *b as u32;
            h = h.wrapping_mul(16777619);
        }
        assert_eq!(h, 1152488355);
    }
    fn run8_16() -> Vec<u8> {
        let mut v = vec![0u8; 16 * 16 * 4];
        let rc = unsafe { change(v.as_mut_ptr(), 16, 16, 16, 4, 8, false, 0.1, 0.02, 0.3, -0.03, 0.12, 5.5, 0.7, 0, 4, 1.0 / 1.7320508) };
        assert_eq!(rc, OK);
        v
    }

    #[test]
    fn golden_u16_8x8() {
        let mut v = vec![0u16; 8 * 8 * 4];
        assert_eq!(unsafe { change(v.as_mut_ptr() as *mut u8, 8, 8, 8, 4, 16, false, 0.3, 0.0, 0.1, 0.0, 0.3, 0.2, 0.0, 0, 3, 0.5) }, OK);
        for (k, e) in [(0usize, 35030u16), (1, 39297), (2, 35900), (3, 36167), (17, 36128), (50, 31158)] {
            assert_eq!(v[k * 4], e, "pixel {k}");
        }
    }

    #[test]
    fn golden_f32_8x8() {
        let mut f = vec![0f32; 8 * 8 * 4];
        assert_eq!(unsafe { change(f.as_mut_ptr() as *mut u8, 8, 8, 8, 4, 24, false, 0.3, 0.1, 0.1, -0.1, 0.3, 0.2, 0.4, 0, 3, 0.5) }, OK);
        let exp = [0.676181436f32, 0.421930522, 0.414765418, 0.496234328, 0.423655272, 0.520360827, 0.505081594, 0.491554439, 0.526156783, 0.439454556, 0.431745827, 0.515352666, 0.555876613];
        for (i, e) in exp.iter().enumerate() {
            let k = i * 5;
            assert!((f[k * 4] - e).abs() < 1e-6, "pixel {k}: {} vs {e}", f[k * 4]);
            assert_eq!(f[k * 4 + 3], 1.0);
        }
    }
}
