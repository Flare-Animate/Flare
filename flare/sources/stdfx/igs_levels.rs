//! Port of stdfx/igs_levels.cpp: per-channel levels (in-range, gamma,
//! out-range) of an 8/16-bit or float image, optionally weighted per pixel by
//! a reference image and by alpha.
//!
//! * 8/16-bit images use an integer lookup table (`val_max + 1` entries).
//! * float images (`bits == 24`, the C++ `numeric_limits<float>::digits`) use a 65536-entry float table with linear interpolation and
//!   a straight extension above 1.0.
//!
//! Channel order of the image is the Toonz6 one: B,G,R,A. Parameters are
//! given in R,G,B,A order. Quirk kept from the C++: a grayscale image is
//! processed with the *B* parameters (it indexes `tables[0]`, the blue table).
//!
//! Deviation from the C++: converting an out-of-range double to the integer
//! sample type (only reachable with `clamp_sw == false`) was undefined
//! behaviour (wrapped on x86); here it saturates.

/// 0 = ok; 1 / 2 = std::domain_error texts; 3 = bad argument.
pub const OK: i32 = 0;
pub const BAD_CHANNELS: i32 = 1; // "Bad channels,Not rgba/rgb/grayscale"
pub const BAD_BITS: i32 = 2; // "Bad bits,Not uchar/ushort"
pub const BAD_ARG: i32 = 3;

const BLU: usize = 0;
const GRE: usize = 1;
const RED: usize = 2;
const ALP: usize = 3;
/// user (R,G,B,A) index -> position in the Toonz6 pixel
const POS: [usize; 4] = [RED, GRE, BLU, ALP];

const EPSILON: f64 = 1e-8; // TConsts::epsilon

/// `bits` value that selects float images: std::numeric_limits<float>::digits (the mantissa
/// width, 24), as `ino::bits()` passes it
pub const FLOAT_BITS: i32 = 24;

fn almost_equal(a: f64, b: f64) -> bool {
    (a - b).abs() < EPSILON
}

#[derive(Clone, Copy)]
pub struct Levels {
    /// R,G,B,A
    pub in_min: [f64; 4],
    pub in_max: [f64; 4],
    pub gamma: [f64; 4],
    pub out_min: [f64; 4],
    pub out_max: [f64; 4],
    pub clamp_sw: bool,
    pub alpha_sw: bool,
    pub add_blend_sw: bool,
}

/// levels_(): one normalized value through the levels curve
fn levels(mut val: f64, in_min: f64, in_max: f64, gamma: f64, out_min: f64, out_max: f64, clamp_sw: bool) -> f64 {
    /* 1 normalize the input range */
    val = if in_max == in_min { in_max } else { (val - in_min) / (in_max - in_min) };

    /* 2 clamp (to the output range) */
    if clamp_sw || !almost_equal(out_max, 1.0) {
        val = if val < 0.0 {
            0.0
        } else if 1.0 < val {
            1.0
        } else {
            val
        };
    } else {
        val = if val < 0.0 { 0.0 } else { val };
    }

    /* 3 gamma inside the normalized range */
    if 1.0 != gamma && 0.0 != gamma {
        if 0.0 < val && val < 1.0 {
            val = val.powf(1.0 / gamma);
        } else if val > 1.0 {
            /* extend the gamma line straight */
            val = 1.0 + (val - 1.0) / gamma;
        }
    }

    /* 4 normalized -> output range */
    val = out_min + val * (out_max - out_min);

    /* 5 clamp */
    if clamp_sw {
        if val < 0.0 {
            0.0
        } else if 1.0 < val {
            1.0
        } else {
            val
        }
    } else if val < 0.0 {
        0.0
    } else {
        val
    }
}

/// Image sample types the integer path handles.
pub(crate) trait IntPx: Copy + PartialEq {
    const MAX: u32;
    fn to_i32(self) -> i32;
    /// plain integer conversion (value must fit)
    fn from_i32(v: i32) -> Self;
    /// static_cast<IT>(double) with saturation (truncates toward zero)
    fn from_f64(v: f64) -> Self;
}
impl IntPx for u8 {
    const MAX: u32 = u8::MAX as u32;
    fn to_i32(self) -> i32 {
        i32::from(self)
    }
    fn from_i32(v: i32) -> u8 {
        v as u8
    }
    fn from_f64(v: f64) -> u8 {
        v as u8
    }
}
impl IntPx for u16 {
    const MAX: u32 = u16::MAX as u32;
    fn to_i32(self) -> i32 {
        i32::from(self)
    }
    fn from_i32(v: i32) -> u16 {
        v as u16
    }
    fn from_f64(v: f64) -> u16 {
        v as u16
    }
}

/// Reference sample types.
pub(crate) trait RefPx: Copy {
    /// numeric_limits<RT>::max() as passed to ref_value (float refs pass 1)
    const MAX: i32;
    fn f(self) -> f64;
}
impl RefPx for u8 {
    const MAX: i32 = u8::MAX as i32;
    fn f(self) -> f64 {
        f64::from(self)
    }
}
impl RefPx for u16 {
    const MAX: i32 = u16::MAX as i32;
    fn f(self) -> f64 {
        f64::from(self)
    }
}
impl RefPx for f32 {
    const MAX: i32 = 1;
    fn f(self) -> f64 {
        f64::from(self)
    }
}

/// igs::color::ref_value (igs_ifx_common.h) on one reference pixel of `cc` samples.
pub(crate) fn ref_value<R: RefPx>(r: &[R], cc: usize, ref_max: i32, mode: i32) -> f64 {
    let m = f64::from(ref_max);
    if ref_max == 0 {
        return 0.0;
    }
    let lum = |r: &[R]| 0.298912 * r[RED].f() / m + 0.586611 * r[GRE].f() / m + 0.114478 * r[BLU].f() / m;
    if cc == 4 {
        match mode {
            0 => return r[RED].f() / m,
            1 => return r[GRE].f() / m,
            2 => return r[BLU].f() / m,
            3 => return r[ALP].f() / m,
            4 => return lum(r),
            _ => {}
        }
    } else if cc == 3 {
        match mode {
            0 => return r[RED].f() / m,
            1 => return r[GRE].f() / m,
            2 => return r[BLU].f() / m,
            3 => return lum(r),
            _ => {}
        }
    } else if cc == 1 {
        return r[0].f() / m;
    }
    1.0
}

/// refchk_(int, int, double)
pub(crate) fn refchk_int(src: i32, tgt: i32, refv: f64) -> f64 {
    if src < tgt {
        (f64::from(tgt - src) + 0.999999) * refv + f64::from(src)
    } else {
        (f64::from(src - tgt) + 0.999999) * (1.0 - refv) + f64::from(tgt)
    }
}

/// refchk_(float, float, double): returns float
fn refchk_f32(src: f32, tgt: f32, refv: f64) -> f32 {
    (f64::from(tgt) * refv + f64::from(src) * (1.0 - refv)) as f32
}

fn change_int<I: IntPx, R: RefPx>(image: &mut [I], channels: usize, refimg: Option<&[R]>, ref_mode: i32, lv: &Levels) {
    let val_max = I::MAX;
    let div_val = f64::from(val_max);
    let mul_val = div_val + 0.999999;
    /* 1 tables, indexed by pixel position (B,G,R,A) */
    let mut tables: [Vec<i32>; 4] = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
    for (u, &pos) in POS.iter().enumerate() {
        tables[pos] = (0..=val_max)
            .map(|yy| {
                let v = f64::from(yy) / div_val;
                (levels(v, lv.in_min[u], lv.in_max[u], lv.gamma[u], lv.out_min[u], lv.out_max[u], lv.clamp_sw) * mul_val) as i32
            })
            .collect();
    }
    let apply = |t: &[i32], v: I, refv: f64| I::from_f64(refchk_int(v.to_i32(), t[v.to_i32() as usize], refv));

    /* 2 apply */
    let pixels = image.len() / channels;
    for ii in 0..pixels {
        let px = &mut image[ii * channels..(ii + 1) * channels];
        let mut refv = 1.0f64;
        if let Some(r) = refimg {
            refv *= ref_value(&r[ii * channels..(ii + 1) * channels], channels, R::MAX, ref_mode);
        }
        if channels == 4 {
            if lv.alpha_sw {
                px[ALP] = apply(&tables[ALP], px[ALP], refv);
            }
            if lv.add_blend_sw && px[ALP].to_i32() == 0 {
                continue;
            }
            if lv.add_blend_sw && (px[ALP].to_i32() as u32) < val_max {
                refv *= f64::from(px[ALP].to_i32()) / div_val;
            }
            px[RED] = apply(&tables[RED], px[RED], refv);
            px[GRE] = apply(&tables[GRE], px[GRE], refv);
            px[BLU] = apply(&tables[BLU], px[BLU], refv);
        } else if channels == 3 {
            px[RED] = apply(&tables[RED], px[RED], refv);
            px[GRE] = apply(&tables[GRE], px[GRE], refv);
            px[BLU] = apply(&tables[BLU], px[BLU], refv);
        } else {
            px[0] = apply(&tables[BLU], px[0], refv); // grayscale reads tables[0] = the *blue* (memory position 0) table
        }
    }
}

fn change_float(image: &mut [f32], channels: usize, refimg: Option<&[f32]>, ref_mode: i32, lv: &Levels) {
    const TABLE_SIZE: usize = 65536;
    let div_val = (TABLE_SIZE - 1) as f64;
    let mut tables: [Vec<f32>; 4] = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
    for (u, &pos) in POS.iter().enumerate() {
        /* colour channels are never clamped; alpha follows clamp_sw */
        let clamp = if u == 3 { lv.clamp_sw } else { false };
        tables[pos] = (0..TABLE_SIZE)
            .map(|yy| levels(yy as f64 / div_val, lv.in_min[u], lv.in_max[u], lv.gamma[u], lv.out_min[u], lv.out_max[u], clamp) as f32)
            .collect();
    }
    let table_val = |c: usize, inp: f32| -> f32 {
        let t = &tables[c];
        if inp <= 0.0 {
            t[0]
        } else if inp <= 1.0 {
            let index = (f64::from(inp) * div_val).floor() as i64;
            let index = index.clamp(0, TABLE_SIZE as i64 - 1) as usize;
            let ratio = (f64::from(inp) * div_val - index as f32 as f64) as f32;
            if almost_equal(f64::from(ratio), 0.0) || index + 1 >= TABLE_SIZE {
                return t[index];
            }
            t[index] * (1.0 - ratio) + t[index + 1] * ratio
        } else {
            let dv = (f64::from(t[TABLE_SIZE - 1] - t[TABLE_SIZE - 2]) * div_val) as f32;
            t[TABLE_SIZE - 1] + dv * (inp - 1.0)
        }
    };

    let pixels = image.len() / channels;
    for ii in 0..pixels {
        let px = &mut image[ii * channels..(ii + 1) * channels];
        let mut refv = 1.0f64;
        if let Some(r) = refimg {
            refv *= ref_value(&r[ii * channels..(ii + 1) * channels], channels, 1, ref_mode);
            // only the RGBA branch clamps the weight to 0..1 (in case the reference is more than 1)
            if channels == 4 {
                refv = 1.0f64.min(0.0f64.max(refv));
            }
        }
        if channels == 4 {
            if lv.alpha_sw {
                px[ALP] = refchk_f32(px[ALP], table_val(ALP, px[ALP]), refv);
            }
            if lv.add_blend_sw && almost_equal(f64::from(px[ALP]), 0.0) {
                continue;
            }
            if lv.add_blend_sw && px[ALP] < 1.0 {
                refv *= f64::from(px[ALP]);
            }
            px[RED] = refchk_f32(px[RED], table_val(RED, px[RED]), refv);
            px[GRE] = refchk_f32(px[GRE], table_val(GRE, px[GRE]), refv);
            px[BLU] = refchk_f32(px[BLU], table_val(BLU, px[BLU]), refv);
        } else if channels == 3 {
            px[RED] = refchk_f32(px[RED], table_val(RED, px[RED]), refv);
            px[GRE] = refchk_f32(px[GRE], table_val(GRE, px[GRE]), refv);
            px[BLU] = refchk_f32(px[BLU], table_val(BLU, px[BLU]), refv);
        } else {
            px[0] = refchk_f32(px[0], table_val(BLU, px[0]), refv); // tables[0] = blue table
        }
    }
}

/// Validated raw entry.
///
/// # Safety
/// `image_array`: `height*width*channels` samples of `bits` bits (aligned for the sample
/// type); `refp`: null or the same count of `ref_bits`-bit samples (aligned).
#[allow(clippy::too_many_arguments)]
pub unsafe fn change_raw(
    image_array: *mut u8,
    height: i32,
    width: i32,
    channels: i32,
    bits: i32,
    refp: *const u8,
    ref_bits: i32,
    ref_mode: i32,
    lv: &Levels,
) -> i32 {
    if channels != 4 && channels != 3 && channels != 1 {
        return BAD_CHANNELS;
    }
    // which (image, ref) combination, as in the C++ if-chain
    #[derive(PartialEq)]
    enum Kind {
        U8U8,
        U16U8,
        U16U16,
        U8U16,
        F32,
    }
    let kind = if bits == 8 && (ref_bits == 8 || ref_bits == 0) {
        Kind::U8U8
    } else if bits == 16 && (ref_bits == 8 || ref_bits == 0) {
        Kind::U16U8
    } else if bits == 16 && ref_bits == 16 {
        Kind::U16U16
    } else if bits == 8 && ref_bits == 16 {
        Kind::U8U16
    } else if bits == FLOAT_BITS {
        Kind::F32
    } else {
        return BAD_BITS;
    };
    if height < 0 || width < 0 {
        return BAD_ARG;
    }
    let c = channels as usize;
    let Some(n) = (height as usize).checked_mul(width as usize).and_then(|v| v.checked_mul(c)) else {
        return BAD_ARG;
    };
    if n == 0 {
        return OK;
    }
    if image_array.is_null() || n > isize::MAX as usize / 8 {
        return BAD_ARG;
    }
    let (isz, rsz) = match kind {
        Kind::U8U8 => (1, 1),
        Kind::U16U8 => (2, 1),
        Kind::U16U16 => (2, 2),
        Kind::U8U16 => (1, 2),
        Kind::F32 => (4, 4),
    };
    if (image_array as usize) % isz != 0 || (!refp.is_null() && (refp as usize) % rsz != 0) {
        return BAD_ARG;
    }
    macro_rules! r {
        ($t:ty) => {
            if refp.is_null() {
                None
            } else {
                Some(std::slice::from_raw_parts(refp as *const $t, n))
            }
        };
    }
    match kind {
        Kind::U8U8 => change_int::<u8, u8>(std::slice::from_raw_parts_mut(image_array, n), c, r!(u8), ref_mode, lv),
        Kind::U16U8 => change_int::<u16, u8>(std::slice::from_raw_parts_mut(image_array as *mut u16, n), c, r!(u8), ref_mode, lv),
        Kind::U16U16 => change_int::<u16, u16>(std::slice::from_raw_parts_mut(image_array as *mut u16, n), c, r!(u16), ref_mode, lv),
        Kind::U8U16 => change_int::<u8, u16>(std::slice::from_raw_parts_mut(image_array, n), c, r!(u16), ref_mode, lv),
        // for float images the reference is float whatever `ref_bits` says (assert in C++)
        Kind::F32 => change_float(std::slice::from_raw_parts_mut(image_array as *mut f32, n), c, r!(f32), ref_mode, lv),
    }
    OK
}

/// # Safety
/// See [`change_raw`].
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn flare_igs_levels_change(
    image_array: *mut u8,
    height: i32,
    width: i32,
    channels: i32,
    bits: i32,
    refp: *const u8,
    ref_bits: i32,
    ref_mode: i32,
    r_in_min: f64,
    r_in_max: f64,
    g_in_min: f64,
    g_in_max: f64,
    b_in_min: f64,
    b_in_max: f64,
    a_in_min: f64,
    a_in_max: f64,
    r_gamma: f64,
    g_gamma: f64,
    b_gamma: f64,
    a_gamma: f64,
    r_out_min: f64,
    r_out_max: f64,
    g_out_min: f64,
    g_out_max: f64,
    b_out_min: f64,
    b_out_max: f64,
    a_out_min: f64,
    a_out_max: f64,
    clamp_sw: bool,
    alpha_sw: bool,
    add_blend_sw: bool,
) -> i32 {
    let lv = Levels {
        in_min: [r_in_min, g_in_min, b_in_min, a_in_min],
        in_max: [r_in_max, g_in_max, b_in_max, a_in_max],
        gamma: [r_gamma, g_gamma, b_gamma, a_gamma],
        out_min: [r_out_min, g_out_min, b_out_min, a_out_min],
        out_max: [r_out_max, g_out_max, b_out_max, a_out_max],
        clamp_sw,
        alpha_sw,
        add_blend_sw,
    };
    change_raw(image_array, height, width, channels, bits, refp, ref_bits, ref_mode, &lv)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ident() -> Levels {
        Levels {
            in_min: [0.0; 4],
            in_max: [1.0; 4],
            gamma: [1.0; 4],
            out_min: [0.0; 4],
            out_max: [1.0; 4],
            clamp_sw: true,
            alpha_sw: true,
            add_blend_sw: false,
        }
    }
    fn run8(img: &mut [u8], ch: i32, lv: &Levels) -> i32 {
        unsafe { change_raw(img.as_mut_ptr(), 1, (img.len() / ch as usize) as i32, ch, 8, std::ptr::null(), 0, 5, lv) }
    }

    #[test]
    fn identity_levels_keep_every_8bit_value() {
        let mut img: Vec<u8> = (0..=255).collect();
        assert_eq!(run8(&mut img, 1, &ident()), OK);
        let expect: Vec<u8> = (0..=255).collect();
        assert_eq!(img, expect);
    }

    #[test]
    fn input_range_stretches_and_clamps() {
        let mut lv = ident();
        lv.in_min = [0.2; 4];
        lv.in_max = [0.6; 4];
        // gray uses the R parameters
        let mut img = [0u8, 51, 102, 153, 255];
        run8(&mut img, 1, &lv);
        assert_eq!(img[0], 0);
        assert_eq!(img[1], 0); // 51/255 == 0.2
        assert_eq!(img[3], 255); // 153/255 == 0.6
        assert_eq!(img[4], 255);
        assert!((i32::from(img[2]) - 127).abs() <= 1);
    }

    #[test]
    fn gamma_and_output_range() {
        let mut lv = ident();
        lv.gamma = [2.0; 4];
        let mut img = [0u8, 64, 255];
        run8(&mut img, 1, &lv);
        // pow(64/255, 1/2) ~= 0.5010 -> 128
        assert!((i32::from(img[1]) - 128).abs() <= 1);
        let mut lv = ident();
        lv.out_min = [0.5; 4];
        lv.out_max = [1.0; 4];
        let mut img = [0u8, 255];
        run8(&mut img, 1, &lv);
        assert_eq!(img, [127, 255]);
    }

    #[test]
    fn rgba_channel_parameters_follow_rgb_order_on_bgr_memory() {
        // B,G,R,A pixel; only red gets out_max 0.5
        let mut lv = ident();
        lv.out_max[0] = 0.5;
        let mut img = [255u8, 255, 255, 255];
        run8(&mut img, 4, &lv);
        assert_eq!(img, [255, 255, 127, 255]);
    }

    #[test]
    fn alpha_sw_off_leaves_alpha_alone() {
        let mut lv = ident();
        lv.out_max[3] = 0.5;
        lv.alpha_sw = false;
        let mut img = [255u8, 255, 255, 255];
        run8(&mut img, 4, &lv);
        assert_eq!(img[3], 255);
        lv.alpha_sw = true;
        run8(&mut img, 4, &lv);
        assert_eq!(img[3], 127);
    }

    #[test]
    fn add_blend_skips_transparent_pixels_and_weights_by_alpha() {
        let mut lv = ident();
        lv.in_min = [0.0; 4];
        lv.in_max = [0.5; 4];
        lv.add_blend_sw = true;
        let mut t = [100u8, 100, 100, 0];
        run8(&mut t, 4, &lv);
        assert_eq!(t, [100, 100, 100, 0]);
        // alpha 128: tgt=255 for colours; refv = 128/255 -> pulled partway
        let mut h = [100u8, 100, 100, 128];
        run8(&mut h, 4, &lv);
        assert!(h[2] > 100 && h[2] < 255);
        // alpha-composite mode changes colour even at alpha 0
        lv.add_blend_sw = false;
        let mut z = [100u8, 100, 100, 0];
        run8(&mut z, 4, &lv);
        assert!(z[2] > 100);
    }

    #[test]
    fn reference_image_weights_the_change() {
        let mut lv = ident();
        lv.out_max = [0.0; 4]; // everything -> 0 at full weight
        let mut img = [200u8, 200];
        let refimg = [255u8, 0]; // gray ref: full weight, none
        let r = unsafe { change_raw(img.as_mut_ptr(), 1, 2, 1, 8, refimg.as_ptr(), 8, 0, &lv) };
        assert_eq!(r, OK);
        assert_eq!(img[0], 0);
        assert_eq!(img[1], 200);
    }

    #[test]
    fn ref_value_modes() {
        let px = [10u8, 20, 30, 40]; // B,G,R,A
        assert_eq!(ref_value(&px, 4, 255, 0), 30.0 / 255.0);
        assert_eq!(ref_value(&px, 4, 255, 1), 20.0 / 255.0);
        assert_eq!(ref_value(&px, 4, 255, 2), 10.0 / 255.0);
        assert_eq!(ref_value(&px, 4, 255, 3), 40.0 / 255.0);
        let lum = 0.298912 * 30.0 / 255.0 + 0.586611 * 20.0 / 255.0 + 0.114478 * 10.0 / 255.0;
        assert_eq!(ref_value(&px, 4, 255, 4), lum);
        assert_eq!(ref_value(&px, 4, 255, 5), 1.0); // "Nothing"
        assert_eq!(ref_value(&px[..3], 3, 255, 3), lum);
        assert_eq!(ref_value(&px[..3], 3, 255, 4), 1.0);
        assert_eq!(ref_value(&px[..1], 1, 255, 9), 10.0 / 255.0);
        assert_eq!(ref_value(&px, 4, 0, 0), 0.0);
    }

    #[test]
    fn sixteen_bit_identity_and_mixed_ref_depths() {
        let mut v: Vec<u16> = vec![0, 1000, 40000, 65535];
        let r = unsafe { change_raw(v.as_mut_ptr() as *mut u8, 1, 4, 1, 16, std::ptr::null(), 0, 5, &ident()) };
        assert_eq!(r, OK);
        assert_eq!(v, [0, 1000, 40000, 65535]);
        // 16-bit image with 8-bit reference, 8-bit image with 16-bit reference are accepted
        let refb = [255u8; 4];
        let r = unsafe { change_raw(v.as_mut_ptr() as *mut u8, 1, 4, 1, 16, refb.as_ptr(), 8, 0, &ident()) };
        assert_eq!(r, OK);
        let mut b = [1u8, 2];
        let ref16 = [65535u16, 65535];
        let r = unsafe { change_raw(b.as_mut_ptr(), 1, 2, 1, 8, ref16.as_ptr() as *const u8, 16, 0, &ident()) };
        assert_eq!(r, OK);
        // 8-bit image + 32-bit ref is not a supported pair
        let r = unsafe { change_raw(b.as_mut_ptr(), 1, 2, 1, 8, refb.as_ptr(), 32, 0, &ident()) };
        assert_eq!(r, BAD_BITS);
    }

    #[test]
    fn float_identity_interpolates_and_extends_above_one() {
        let mut f = [0.0f32, 0.25, 0.5, 1.0, 1.5, -0.5];
        let r = unsafe { change_raw(f.as_mut_ptr() as *mut u8, 1, 6, 1, FLOAT_BITS, std::ptr::null(), 0, 5, &ident()) };
        assert_eq!(r, OK);
        let expect = [0.0f32, 0.25, 0.5, 1.0, 1.5, 0.0];
        for (a, b) in f.iter().zip(expect) {
            // the extension above 1 uses a float slope (difference of two floats near 1)
            assert!((a - b).abs() < 5e-3, "{a} vs {b}");
        }
    }

    #[test]
    fn float_gamma_curve_and_unclamped_colour() {
        let mut lv = ident();
        lv.gamma = [2.0; 4];
        let mut f = [0.25f32];
        unsafe { change_raw(f.as_mut_ptr() as *mut u8, 1, 1, 1, FLOAT_BITS, std::ptr::null(), 0, 5, &lv) };
        assert!((f[0] - 0.5).abs() < 1e-3);
        // values above 1 follow the straight extension of the gamma line (slope 1/gamma)
        let mut g = [3.0f32];
        unsafe { change_raw(g.as_mut_ptr() as *mut u8, 1, 1, 1, FLOAT_BITS, std::ptr::null(), 0, 5, &lv) };
        assert!((g[0] - 2.0).abs() < 5e-2, "{}", g[0]);
    }

    #[test]
    fn errors() {
        let mut b = [0u8; 8];
        let mut w = [0u16; 4];
        let lv = ident();
        unsafe {
            assert_eq!(change_raw(b.as_mut_ptr(), 1, 1, 2, 8, std::ptr::null(), 0, 0, &lv), BAD_CHANNELS);
            assert_eq!(change_raw(b.as_mut_ptr(), 1, 1, 1, 12, std::ptr::null(), 0, 0, &lv), BAD_BITS);
            assert_eq!(change_raw(b.as_mut_ptr(), 1, 1, 1, 32, std::ptr::null(), 0, 0, &lv), BAD_BITS); // 32 is not the float code
            assert_eq!(change_raw(std::ptr::null_mut(), 1, 1, 1, 8, std::ptr::null(), 0, 0, &lv), BAD_ARG);
            assert_eq!(change_raw(b.as_mut_ptr(), -1, 1, 1, 8, std::ptr::null(), 0, 0, &lv), BAD_ARG);
            assert_eq!(change_raw((w.as_mut_ptr() as *mut u8).add(1), 1, 1, 1, 16, std::ptr::null(), 0, 0, &lv), BAD_ARG); // misaligned u16
        }
    }
}
