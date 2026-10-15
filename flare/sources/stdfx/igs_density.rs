//! Port of stdfx/igs_density.cpp: scales the "density" (opacity build-up) of a
//! float RGBA image, optionally per pixel through a reference map.
//! Channel order is the Toonz6 one (blu, gre, red, alp).

const BLU: usize = 0;
const GRE: usize = 1;
const RED: usize = 2;
const ALP: usize = 3;

pub const OK: i32 = 0;
pub const BAD_CHANNELS: i32 = 1; // C++ throws std::domain_error("Bad channels,Not rgba")

/// accum_by_trans_: mixed float/double arithmetic exactly as in C++
pub fn accum_by_trans(
    src_value: f32,     /* source value (R,G,B) */
    transparent: f32,   /* transparency of src_value (0...1) */
    integer_part: i32,  /* integer part of the density (0...) */
    fractional_part: f64, /* fractional part of the density (0...1) */
) -> f32 {
    let mut accumulation: f32 = src_value;
    if 1 <= integer_part {
        for _ in 1..integer_part {
            accumulation = accumulation * transparent + src_value;
        }
        if 0.0f64 < fractional_part {
            let diff: f32 = (accumulation * transparent + src_value) - accumulation;
            accumulation = ((accumulation as f64) + (diff as f64) * fractional_part) as f32;
        }
    } else {
        /* integer part <= 0 */
        if 0.0f64 < fractional_part {
            accumulation = ((accumulation as f64) * fractional_part) as f32;
        } else {
            /* fractional part <= 0 too */
            accumulation = 0.0;
        }
    }
    if 1.0f32 < accumulation {
        1.0
    } else if accumulation < 0.0f32 {
        0.0
    } else {
        accumulation
    }
}

/// Safe core of igs::density::change: `image` is RGBA (4 floats per pixel),
/// `ref_` holds one float per pixel when present.
fn change_(image: &mut [f32], ref_: Option<&[f32]>, density: f64) {
    let integer_part = density as i32;
    let fractional_part = density - (density as i32) as f64;

    for (i, px) in image.chunks_exact_mut(4).enumerate() {
        let rr1 = px[RED];
        let gg1 = px[GRE];
        let bb1 = px[BLU];
        let aa1 = px[ALP];

        let t = (1.0f64 - aa1 as f64) as f32;
        let mut rr2 = accum_by_trans(rr1, t, integer_part, fractional_part);
        let mut gg2 = accum_by_trans(gg1, t, integer_part, fractional_part);
        let mut bb2 = accum_by_trans(bb1, t, integer_part, fractional_part);
        let mut aa2 = accum_by_trans(aa1, t, integer_part, fractional_part);

        /* per-pixel change through the reference image, if any */
        if let Some(r) = ref_ {
            let k = r[i];
            rr2 = (rr2 - rr1) * k + rr1;
            gg2 = (gg2 - gg1) * k + gg1;
            bb2 = (bb2 - bb1) * k + bb1;
            aa2 = (aa2 - aa1) * k + aa1;
        }

        px[RED] = rr2;
        px[GRE] = gg2;
        px[BLU] = bb2;
        px[ALP] = aa2;
    }
}

/// igs::density::change.  Returns 1 when the C++ header must throw
/// `std::domain_error("Bad channels,Not rgba")`.
///
/// # Safety
/// `image_array` must hold `height*width*channels` floats (null or non-positive sizes are a no-op); `ref_` is null or
/// holds `height*width` floats.
#[no_mangle]
pub unsafe extern "C" fn flare_igs_density_change(
    image_array: *mut f32, height: i32, width: i32, channels: i32, ref_: *const f32, density: f64,
) -> i32 {
    // a panic must never unwind into C++: report invalid_argument (3)
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    if 4 != channels {
        return BAD_CHANNELS;
    }
    if image_array.is_null() || height <= 0 || width <= 0 {
        return OK; // nothing to change; the C++ loop runs zero times
    }
    let pixels = match (height as usize).checked_mul(width as usize) {
        Some(n) => n,
        None => return OK,
    };
    let Some(len) = pixels.checked_mul(4) else { return OK };
    let image = std::slice::from_raw_parts_mut(image_array, len);
    let reference = if ref_.is_null() { None } else { Some(std::slice::from_raw_parts(ref_, pixels)) };
    change_(image, reference, density);
    OK
})).unwrap_or(3)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(px: &mut [f32], r: Option<&[f32]>, d: f64) -> i32 {
        let n = px.len() as i32 / 4;
        unsafe {
            flare_igs_density_change(px.as_mut_ptr(), 1, n, 4, r.map_or(std::ptr::null(), |s| s.as_ptr()), d)
        }
    }

    #[test]
    fn accum_cases() {
        // density 1.0: integer_part 1, frac 0 -> unchanged value
        assert_eq!(accum_by_trans(0.5, 0.5, 1, 0.0), 0.5);
        // integer 2: 0.5*0.5 + 0.5 = 0.75
        assert_eq!(accum_by_trans(0.5, 0.5, 2, 0.0), 0.75);
        // integer 2 + frac .5: 0.75 + ((0.75*.5+.5) - .75) * .5 = 0.75 + 0.125*0.5 = 0.8125
        assert_eq!(accum_by_trans(0.5, 0.5, 2, 0.5), 0.8125);
        // integer 0, frac .5 -> halves
        assert_eq!(accum_by_trans(0.5, 0.5, 0, 0.5), 0.25);
        // nothing left -> 0
        assert_eq!(accum_by_trans(0.5, 0.5, 0, 0.0), 0.0);
        // clamped to 1
        assert_eq!(accum_by_trans(1.0, 1.0, 5, 0.0), 1.0);
    }

    #[test]
    fn density_one_is_identity_and_two_builds_up() {
        // B,G,R,A
        let mut px = [0.2f32, 0.4, 0.5, 0.5];
        assert_eq!(run(&mut px, None, 1.0), OK);
        assert_eq!(px, [0.2, 0.4, 0.5, 0.5]);
        // density 2: c' = c*(1-a) + c  (t = 1-a = 0.5)
        let mut px = [0.2f32, 0.4, 0.5, 0.5];
        run(&mut px, None, 2.0);
        assert_eq!(px, [0.2 * 0.5 + 0.2, 0.4 * 0.5 + 0.4, 0.5 * 0.5 + 0.5, 0.5 * 0.5 + 0.5]);
    }

    #[test]
    fn reference_map_blends_per_pixel() {
        let mut px = [0.0f32, 0.0, 0.5, 0.5, 0.0, 0.0, 0.5, 0.5];
        let r = [0.0f32, 1.0];
        run(&mut px, Some(&r), 2.0);
        // pixel 0: ref 0 -> unchanged ; pixel 1: ref 1 -> fully density 2
        assert_eq!(&px[..4], &[0.0, 0.0, 0.5, 0.5]);
        assert_eq!(&px[4..], &[0.0, 0.0, 0.75, 0.75]);
    }

    #[test]
    fn rejects_non_rgba() {
        let mut px = [0.0f32; 3];
        assert_eq!(unsafe { flare_igs_density_change(px.as_mut_ptr(), 1, 1, 3, std::ptr::null(), 1.0) }, BAD_CHANNELS);
    }
}
