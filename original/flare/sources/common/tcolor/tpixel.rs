//! Port of the functions of `common/tcolor/tpixel.cpp`: the grey conversions
//! `TPixelGR8::from(TPixel32)`, `TPixelGR16::from(TPixel64)` and `TPixelGRF::from(TPixelF)`.
//!
//! `tpixel.cpp` itself stays (trimmed to the static constants): `TPixelRGBM32::Red`,
//! `TPixelD::Transparent`, `TPixelGR8::White`, `maxChannelValue`, ... are `static const`
//! *objects* of the pixel classes -- C++ constructed, exported from tnzcore -- and cannot be
//! owned by Rust. What moved is the arithmetic; the three `from()` members are now inline in
//! `tpixelgr.h` and call the scalar entry points below (channels in, grey value out, so the
//! machine channel order of the C++ structs does not matter).
//!
//! The weights are the ITU-R 601 luma in 16.16 fixed point (`19594 + 38472 + 7470 == 65536`),
//! rounded by adding half (`1 << 15`) before the shift; the float variant is the plain
//! `0.299 r + 0.587 g + 0.114 b` evaluated left to right in single precision (no fused
//! multiply-add, as the C++ was built).

/// `(UINT)r * 19594 + (UINT)g * 38472 + (UINT)b * 7470 + (1 << 15)) >> 16`, 32 bit unsigned
/// arithmetic. 8 bit channels (TPixelGR8) and 16 bit channels (TPixelGR16) share it: the
/// largest possible sum, `65535 * 65536 + 32768`, still fits in 32 bits.
pub fn gray_fixed(r: u32, g: u32, b: u32) -> u32 {
    r.wrapping_mul(19594).wrapping_add(g.wrapping_mul(38472)).wrapping_add(b.wrapping_mul(7470)).wrapping_add(1 << 15) >> 16
}

/// `TPixelGRF::from(TPixelF)`.
pub fn gray_float(r: f32, g: f32, b: f32) -> f32 {
    r * 0.299f32 + g * 0.587f32 + b * 0.114f32
}

/// `TPixelGR8::from(const TPixel32 &)`: the 8 bit grey of an 8 bit pixel.
#[no_mangle]
pub extern "C" fn flare_pixel_gr8_from_rgbm32(r: u32, g: u32, b: u32) -> u32 {
    crate::guard(0, || gray_fixed(r & 0xff, g & 0xff, b & 0xff))
}

/// `TPixelGR16::from(const TPixel64 &)`: the 16 bit grey of a 16 bit pixel.
#[no_mangle]
pub extern "C" fn flare_pixel_gr16_from_rgbm64(r: u32, g: u32, b: u32) -> u32 {
    crate::guard(0, || gray_fixed(r & 0xffff, g & 0xffff, b & 0xffff))
}

/// `TPixelGRF::from(const TPixelF &)`.
#[no_mangle]
pub extern "C" fn flare_pixel_grf_from_rgbmf(r: f32, g: f32, b: f32) -> f32 {
    crate::guard(0.0, || gray_float(r, g, b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weights_sum_to_one() {
        assert_eq!(19594 + 38472 + 7470, 65536);
        // white maps to white, black to black, at both depths
        assert_eq!(gray_fixed(255, 255, 255), 255);
        assert_eq!(gray_fixed(0, 0, 0), 0);
        assert_eq!(gray_fixed(65535, 65535, 65535), 65535);
    }

    #[test]
    fn primaries_and_rounding() {
        // 255 * 19594 / 65536 = 76.2 -> 76; 255 * 38472 / 65536 = 149.7 -> 150; 255 * 7470 / 65536 = 29.07 -> 29
        assert_eq!(gray_fixed(255, 0, 0), 76);
        assert_eq!(gray_fixed(0, 255, 0), 150);
        assert_eq!(gray_fixed(0, 0, 255), 29);
        assert_eq!(flare_pixel_gr8_from_rgbm32(255, 0, 0), 76);
        assert_eq!(flare_pixel_gr16_from_rgbm64(65535, 0, 0), (65535 * 19594 + 32768) >> 16);
        // the 8 bit entry masks to a byte, as the UCHAR channels did
        assert_eq!(flare_pixel_gr8_from_rgbm32(0x1ff, 0, 0), flare_pixel_gr8_from_rgbm32(0xff, 0, 0));
    }

    #[test]
    fn exhaustive_8bit_matches_the_formula() {
        for v in 0..=255u32 {
            // grey in -> same grey out (the weights sum to exactly 65536)
            assert_eq!(gray_fixed(v, v, v), v);
        }
        for v in (0..=65535u32).step_by(257) {
            assert_eq!(gray_fixed(v, v, v), v);
        }
    }

    #[test]
    fn float_is_left_to_right_single_precision() {
        assert_eq!(gray_float(1.0, 1.0, 1.0), 1.0f32 * 0.299f32 + 1.0f32 * 0.587f32 + 1.0f32 * 0.114f32);
        assert_eq!(flare_pixel_grf_from_rgbmf(0.5, 0.25, 0.125), 0.5f32 * 0.299f32 + 0.25f32 * 0.587f32 + 0.125f32 * 0.114f32);
        assert!(gray_float(f32::NAN, 0.0, 0.0).is_nan());
    }

    fn parse(line: &str) -> (&str, Vec<Vec<u64>>) {
        // "tag a b c | x y" : decimal tokens, or hex when the token is 8/16 hex digits long
        let (tag, rest) = line.split_once(' ').unwrap();
        let groups = rest
            .split(" | ")
            .map(|g| {
                g.split(' ')
                    .map(|t| if t.len() == 16 || t.len() == 8 { u64::from_str_radix(t, 16).unwrap() } else { t.parse::<i64>().unwrap() as u64 })
                    .collect()
            })
            .collect();
        (tag, groups)
    }

    // Replays the grey conversions recorded from the original C++ (tcolor_cases.txt).
    #[test]
    fn replay_recorded_cpp_cases() {
        let mut n = 0;
        for line in include_str!("tcolor_cases.txt").lines().filter(|l| l.starts_with("gr ")) {
            let (_, g) = parse(line);
            let (i, o) = (&g[0], &g[1]);
            assert_eq!(flare_pixel_gr8_from_rgbm32(i[0] as u32, i[1] as u32, i[2] as u32) as u64, o[0], "{line}");
            assert_eq!(flare_pixel_gr16_from_rgbm64(i[3] as u32, i[4] as u32, i[5] as u32) as u64, o[1], "{line}");
            n += 1;
        }
        assert_eq!(n, 400);
    }
}
