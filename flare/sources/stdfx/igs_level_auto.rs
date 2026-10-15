//! Port of stdfx/igs_level_auto.cpp: auto-levels of an 8/16-bit image. The
//! per-channel min/max of the *whole* image become the input range (plus the
//! user shifts); a lookup table is built and applied.
//! Channel order is the Toonz6 one: B,G,R,A; `act_sw[0..4]` = R,G,B,A.

/// 0 = ok; 1 / 2 = std::domain_error texts below; 3 = bad argument.
pub const OK: i32 = 0;
pub const BAD_CHANNELS: i32 = 1; // "Bad channels,Not rgba/rgb/grayscale"
pub const BAD_BITS: i32 = 2; // "Bad bits,Not uchar/ushort"
pub const BAD_ARG: i32 = 3;

const BLU: usize = 0;
const GRE: usize = 1;
const RED: usize = 2;
const ALP: usize = 3;

/// One level value, as an integer-valued double (C++ `floor(value * mul_max)`).
#[allow(clippy::too_many_arguments)]
pub(crate) fn level_value(mut value: f64, mul_max: f64, act_sw: bool, in_min: f64, in_max: f64, out_min: f64, out_max: f64, gamma: f64) -> f64 {
    if act_sw {
        if in_max == in_min {
            value = in_max;
        } else {
            /* clamp to in_min..in_max (in either order) */
            if in_min < in_max {
                if value < in_min {
                    value = in_min;
                } else if in_max < value {
                    value = in_max;
                }
            } else if value < in_max {
                value = in_max;
            } else if in_min < value {
                value = in_min;
            }
            /* normalize 0..1 */
            value = (value - in_min) / (in_max - in_min);
            /* gamma */
            if 1.0 != gamma && 0.0 != gamma {
                value = value.powf(1.0 / gamma);
            }
        }
        /* scale to the output range */
        value = out_min + value * (out_max - out_min);
        /* clamp 0..1 */
        if value < 0.0 {
            value = 0.0;
        } else if 1.0 < value {
            value = 1.0;
        }
    }
    (value * mul_max).floor()
}

/// level_ctable_template_: `table[cc][0..=div_num]`
#[allow(clippy::too_many_arguments)]
pub(crate) fn level_ctable(
    channels: usize,
    act_sw: &[bool],
    in_min: &[i32],
    in_max: &[i32],
    in_min_shift: &[f64],
    in_max_shift: &[f64],
    out_min: &[f64],
    out_max: &[f64],
    gamma: &[f64],
    div_num: u32,
) -> Vec<Vec<u32>> {
    let div_val = f64::from(div_num);
    let mul_val = div_val + 0.999999;
    let mut table = Vec::with_capacity(channels);
    for cc in 0..channels {
        let imin = in_min_shift[cc] + f64::from(in_min[cc]) / div_val;
        let imax = in_max_shift[cc] + f64::from(in_max[cc]) / div_val;
        let t: Vec<u32> = (0..=div_num)
            .map(|yy| level_value(f64::from(yy) / div_val, mul_val, act_sw[cc], imin, imax, out_min[cc], out_max[cc], gamma[cc]) as u32)
            .collect();
        table.push(t);
    }
    table
}

pub(crate) trait Sample: Copy + Into<i32> {
    const MAX: u32;
    fn from_u32(v: u32) -> Self;
}
impl Sample for u8 {
    const MAX: u32 = u8::MAX as u32;
    fn from_u32(v: u32) -> u8 {
        v as u8
    }
}
impl Sample for u16 {
    const MAX: u32 = u16::MAX as u32;
    fn from_u32(v: u32) -> u16 {
        v as u16
    }
}

/// Applies `table` (index = channel in R,G,B,A user order) to every pixel.
pub(crate) fn apply_table<T: Sample>(image: &mut [T], channels: usize, table: &[Vec<u32>]) {
    let tab = |c: usize, v: T| T::from_u32(table[c][Into::<i32>::into(v) as usize]);
    if channels == 4 {
        for px in image.chunks_exact_mut(4) {
            px[RED] = tab(0, px[RED]);
            px[GRE] = tab(1, px[GRE]);
            px[BLU] = tab(2, px[BLU]);
            px[ALP] = tab(3, px[ALP]);
        }
    } else if channels == 3 {
        for px in image.chunks_exact_mut(3) {
            px[RED] = tab(0, px[RED]);
            px[GRE] = tab(1, px[GRE]);
            px[BLU] = tab(2, px[BLU]);
        }
    } else {
        for v in image.iter_mut() {
            *v = tab(0, *v);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn change_template<T: Sample>(
    image: &mut [T],
    channels: usize,
    act_sw: &[bool],
    in_min_shift: &[f64],
    in_max_shift: &[f64],
    out_min: &[f64],
    out_max: &[f64],
    gamma: &[f64],
) {
    if image.is_empty() {
        return;
    }
    /* 1. min / max of the whole image */
    let mut in_min = [0i32; 4];
    let mut in_max = [0i32; 4];
    for zz in 0..channels {
        in_min[zz] = image[zz].into();
        in_max[zz] = in_min[zz];
    }
    for px in image.chunks_exact(channels) {
        for zz in 0..channels {
            let v: i32 = px[zz].into();
            if v < in_min[zz] {
                in_min[zz] = v;
            } else if in_max[zz] < v {
                in_max[zz] = v;
            }
        }
    }
    /* 2. table, 3. apply */
    let table = level_ctable(channels, act_sw, &in_min, &in_max, in_min_shift, in_max_shift, out_min, out_max, gamma, T::MAX);
    apply_table(image, channels, &table);
}

/// Safe core. Slices hold `channels` entries each.
#[allow(clippy::too_many_arguments)]
pub fn change(
    image: &mut [u8],
    channels: usize,
    bits: i32,
    act_sw: &[bool],
    in_min_shift: &[f64],
    in_max_shift: &[f64],
    out_min: &[f64],
    out_max: &[f64],
    gamma: &[f64],
) -> i32 {
    if channels != 4 && channels != 3 && channels != 1 {
        return BAD_CHANNELS;
    }
    if u8::BITS as i32 == bits {
        change_template::<u8>(image, channels, act_sw, in_min_shift, in_max_shift, out_min, out_max, gamma);
    } else if u16::BITS as i32 == bits {
        // SAFETY: u8 -> u16 reinterpretation is only done on the aligned middle part;
        // an unaligned buffer goes through an aligned copy.
        let (pre, mid, post) = unsafe { image.align_to_mut::<u16>() };
        if pre.is_empty() && post.is_empty() {
            change_template::<u16>(mid, channels, act_sw, in_min_shift, in_max_shift, out_min, out_max, gamma);
        } else {
            let mut tmp: Vec<u16> = image.chunks_exact(2).map(|b| u16::from_ne_bytes([b[0], b[1]])).collect();
            change_template::<u16>(&mut tmp, channels, act_sw, in_min_shift, in_max_shift, out_min, out_max, gamma);
            for (b, v) in image.chunks_exact_mut(2).zip(tmp) {
                b.copy_from_slice(&v.to_ne_bytes());
            }
        }
    } else {
        return BAD_BITS;
    }
    OK
}

/// Validated raw entry.
///
/// # Safety
/// `image_array` covers `height*width*channels` samples of `bits` bits; each of the
/// five arrays and `act_sw` holds `channels` entries.
#[allow(clippy::too_many_arguments)]
pub unsafe fn change_raw(
    image_array: *mut u8,
    height: i32,
    width: i32,
    channels: i32,
    bits: i32,
    act_sw: *const bool,
    in_min_shift: *const f64,
    in_max_shift: *const f64,
    out_min: *const f64,
    out_max: *const f64,
    gamma: *const f64,
) -> i32 {
    if channels != 4 && channels != 3 && channels != 1 {
        return BAD_CHANNELS;
    }
    if bits != 8 && bits != 16 {
        return BAD_BITS;
    }
    if height < 0 || width < 0 || act_sw.is_null() || in_min_shift.is_null() || in_max_shift.is_null() || out_min.is_null() || out_max.is_null() || gamma.is_null() {
        return BAD_ARG;
    }
    let c = channels as usize;
    let Some(n) = (height as usize).checked_mul(width as usize).and_then(|v| v.checked_mul(c)) else {
        return BAD_ARG;
    };
    if n == 0 {
        return OK;
    }
    let bytes = n * (bits as usize / 8);
    if image_array.is_null() || bytes > isize::MAX as usize / 2 {
        return BAD_ARG;
    }
    let img = std::slice::from_raw_parts_mut(image_array, bytes);
    change(
        img,
        c,
        bits,
        std::slice::from_raw_parts(act_sw, c),
        std::slice::from_raw_parts(in_min_shift, c),
        std::slice::from_raw_parts(in_max_shift, c),
        std::slice::from_raw_parts(out_min, c),
        std::slice::from_raw_parts(out_max, c),
        std::slice::from_raw_parts(gamma, c),
    )
}

/// # Safety
/// See [`change_raw`].
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn flare_igs_level_auto_change(
    image_array: *mut u8,
    height: i32,
    width: i32,
    channels: i32,
    bits: i32,
    act_sw: *const bool,
    in_min_shift: *const f64,
    in_max_shift: *const f64,
    out_min: *const f64,
    out_max: *const f64,
    gamma: *const f64,
) -> i32 {
    // a panic must never unwind into C++: report invalid_argument (3)
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    change_raw(image_array, height, width, channels, bits, act_sw, in_min_shift, in_max_shift, out_min, out_max, gamma)
})).unwrap_or(3)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ON: [bool; 4] = [true; 4];
    const Z: [f64; 4] = [0.0; 4];
    const ONE: [f64; 4] = [1.0; 4];

    fn run8(img: &mut [u8], ch: usize, act: &[bool], omin: &[f64], omax: &[f64], gamma: &[f64]) -> i32 {
        change(img, ch, 8, act, &Z, &Z, omin, omax, gamma)
    }

    #[test]
    fn gray_stretches_min_max_to_full_range() {
        let mut img = [50u8, 100, 150];
        assert_eq!(run8(&mut img, 1, &ON, &Z, &ONE, &ONE), OK);
        assert_eq!(img[0], 0);
        assert_eq!(img[2], 255);
        assert!((i32::from(img[1]) - 127).abs() <= 1);
    }

    #[test]
    fn inactive_channel_is_left_unchanged() {
        // B,G,R,A = (30,20,30,40) and (200,210,200,230); only act_sw[0] (red) active.
        // Blue and red share the same range so the quirk below does not matter here.
        let mut img = [30u8, 20, 30, 40, 200, 210, 200, 230];
        let act = [true, false, false, false];
        assert_eq!(run8(&mut img, 4, &act, &Z, &ONE, &ONE), OK);
        assert_eq!(img, [30, 20, 0, 40, 200, 210, 255, 230]);
    }

    #[test]
    fn quirk_kept_from_cpp_user_channel_n_uses_the_min_max_of_memory_position_n() {
        // The C++ measures min/max per memory position (B,G,R,A) but builds table[0..4] in user
        // order (R,G,B,A): the red table (user 0) is built from the *blue* range (memory 0).
        // blue: 10..200, red: 30..220  -> red 30 maps through blue's range: (30-10)/190 * 255.999999
        let mut img = [10u8, 20, 30, 40, 200, 210, 220, 230];
        let act = [true, false, false, false];
        assert_eq!(run8(&mut img, 4, &act, &Z, &ONE, &ONE), OK);
        assert_eq!(img[2], ((30.0f64 - 10.0) / 190.0 * (255.0 + 0.999999)).floor() as u8);
        assert_eq!(img[6], 255);
    }

    #[test]
    fn flat_channel_maps_via_in_max_equal_in_min() {
        // all equal -> in_max == in_min -> value = in_max = 100/255 -> then scaled by out range
        let mut img = [100u8, 100, 100];
        assert_eq!(run8(&mut img, 1, &ON, &Z, &ONE, &ONE), OK);
        assert_eq!(img, [100, 100, 100]);
        let mut img = [100u8, 100];
        let omax = [0.5f64; 4];
        run8(&mut img, 1, &ON, &Z, &omax, &ONE);
        // out_min + value*(out_max-out_min) = 100/255*0.5 ; *255.999999 floored
        assert_eq!(img[0], ((100.0f64 / 255.0 * 0.5) * (255.0 + 0.999999)).floor() as u8);
    }

    #[test]
    fn gamma_above_one_brightens_midtones() {
        let mut a = [0u8, 128, 255];
        let g = [2.0f64; 4];
        run8(&mut a, 1, &ON, &Z, &ONE, &g);
        assert!(a[1] > 128);
        assert_eq!((a[0], a[2]), (0, 255));
    }

    #[test]
    fn shifts_move_the_input_range() {
        // in_min_shift +0.2 makes dark pixels clip to black
        let mut img = [0u8, 51, 255];
        let shift = [0.2f64; 4];
        change(&mut img, 1, 8, &ON, &shift, &Z, &Z, &ONE, &ONE);
        assert_eq!(img[0], 0);
        assert_eq!(img[1], 0);
        assert_eq!(img[2], 255);
    }

    #[test]
    fn sixteen_bit_uses_16bit_table() {
        let mut v = [1000u16, 2000, 3000];
        let mut bytes: Vec<u8> = v.iter().flat_map(|x| x.to_ne_bytes()).collect();
        assert_eq!(change(&mut bytes, 1, 16, &ON, &Z, &Z, &Z, &ONE, &ONE), OK);
        for (i, c) in bytes.chunks_exact(2).enumerate() {
            v[i] = u16::from_ne_bytes([c[0], c[1]]);
        }
        assert_eq!((v[0], v[2]), (0, 65535));
    }

    #[test]
    fn errors() {
        let mut img = [0u8; 8];
        assert_eq!(change(&mut img, 2, 8, &ON, &Z, &Z, &Z, &ONE, &ONE), BAD_CHANNELS);
        assert_eq!(change(&mut img, 1, 32, &ON, &Z, &Z, &Z, &ONE, &ONE), BAD_BITS);
        unsafe {
            assert_eq!(change_raw(img.as_mut_ptr(), 1, 1, 1, 8, std::ptr::null(), Z.as_ptr(), Z.as_ptr(), Z.as_ptr(), ONE.as_ptr(), ONE.as_ptr()), BAD_ARG);
            assert_eq!(change_raw(std::ptr::null_mut(), 1, 1, 1, 8, ON.as_ptr(), Z.as_ptr(), Z.as_ptr(), Z.as_ptr(), ONE.as_ptr(), ONE.as_ptr()), BAD_ARG);
            assert_eq!(change_raw(img.as_mut_ptr(), -1, 1, 1, 8, ON.as_ptr(), Z.as_ptr(), Z.as_ptr(), Z.as_ptr(), ONE.as_ptr(), ONE.as_ptr()), BAD_ARG);
            assert_eq!(change_raw(img.as_mut_ptr(), 2, 2, 2, 8, ON.as_ptr(), Z.as_ptr(), Z.as_ptr(), Z.as_ptr(), ONE.as_ptr(), ONE.as_ptr()), BAD_CHANNELS);
            assert_eq!(change_raw(img.as_mut_ptr(), 2, 2, 1, 8, ON.as_ptr(), Z.as_ptr(), Z.as_ptr(), Z.as_ptr(), ONE.as_ptr(), ONE.as_ptr()), OK);
        }
    }
}
