//! Port of stdfx/igs_negate.cpp: channel-wise negate of 8/16-bit (and 1-bit) images.
//! Channel order is the Toonz6/GDI one (blu, gre, red, alp), as fixed by
//! `RGBA_ORDER_OF_TOONZ6` in igs_ifx_common.h.

use std::ops::Not;

const BLU: usize = 0;
const GRE: usize = 1;
const RED: usize = 2;
const ALP: usize = 3;
const RGBA_SIZ: i32 = 4;
const RGB_SIZ: i32 = 3;

/// 0 = ok; the C++ wrapper throws std::domain_error for the others
pub const OK: i32 = 0;
pub const BAD_CHANNELS: i32 = 1; // "Bad channels,Not rgba/rgb/grayscale"
pub const BAD_BITS: i32 = 2; // "Bad bits,Not uchar/ushort/bit"

trait Chan: Copy + PartialOrd + Not<Output = Self> {
    fn zero() -> Self;
    fn sub(self, o: Self) -> Self;
}
impl Chan for u8 {
    fn zero() -> u8 {
        0
    }
    fn sub(self, o: u8) -> u8 {
        self.wrapping_sub(o)
    }
}
impl Chan for u16 {
    fn zero() -> u16 {
        0
    }
    fn sub(self, o: u16) -> u16 {
        self.wrapping_sub(o)
    }
}

unsafe fn change_channel<T: Chan>(ima: *mut T, pix_size: i32, channels: i32) {
    let mut ima = ima;
    for _ in 0..pix_size {
        *ima = !*ima;
        ima = ima.offset(channels as isize);
    }
}

unsafe fn change_multiplied_rgb<T: Chan>(ima: *mut T, alp: *mut T, pix_size: i32, channels: i32) {
    let (mut ima, mut alp) = (ima, alp);
    for _ in 0..pix_size {
        if *alp < *ima {
            *ima = T::zero();
        } else {
            *ima = (*alp).sub(*ima);
        }
        ima = ima.offset(channels as isize);
        alp = alp.offset(channels as isize);
    }
}

unsafe fn change_template<T: Chan>(ima: *mut T, hh: i32, ww: i32, ch: i32, sw: *const bool) {
    let sz = hh * ww;

    if RGBA_SIZ == ch {
        if *sw.add(0) {
            change_multiplied_rgb(ima.add(RED), ima.add(ALP), sz, ch);
        }
        if *sw.add(1) {
            change_multiplied_rgb(ima.add(GRE), ima.add(ALP), sz, ch);
        }
        if *sw.add(2) {
            change_multiplied_rgb(ima.add(BLU), ima.add(ALP), sz, ch);
        }
        if *sw.add(3) {
            change_channel(ima.add(ALP), sz, ch);
        }
    } else if RGB_SIZ == ch {
        if *sw.add(0) {
            change_channel(ima.add(RED), sz, ch);
        }
        if *sw.add(1) {
            change_channel(ima.add(GRE), sz, ch);
        }
        if *sw.add(2) {
            change_channel(ima.add(BLU), sz, ch);
        }
    } else if 1 == ch {
        /* Grayscale */
        if *sw.add(0) {
            change_channel(ima, sz, ch);
        }
    }
}

/// igs::negate::change
///
/// # Safety
/// `image_array` must cover the image (`height*width*channels` samples of `bits` bits,
/// aligned for 16-bit samples) and `sw_array` must hold `channels` flags.
pub unsafe fn change(image_array: *mut u8, height: i32, width: i32, channels: i32, bits: i32, sw_array: *const bool) -> i32 {
    if RGBA_SIZ != channels && RGB_SIZ != channels && 1 != channels {
        return BAD_CHANNELS;
    }
    if u8::BITS as i32 == bits {
        change_template(image_array, height, width, channels, sw_array);
    } else if u16::BITS as i32 == bits {
        change_template(image_array as *mut u16, height, width, channels, sw_array);
    } else if 1 == bits {
        let bi = width * channels * bits;
        let dg = u8::BITS as i32;
        let sl_bytes = bi / dg + if 0 != (bi % dg) { 1 } else { 0 };
        let image_size = height * sl_bytes;
        for ii in 0..image_size {
            let p = image_array.offset(ii as isize);
            *p = !*p;
        }
    } else {
        return BAD_BITS;
    }
    OK
}

/// Returns 0, or 1 / 2 when the C++ header must throw `std::domain_error`.
///
/// # Safety
/// See [`change`].
#[no_mangle]
pub unsafe extern "C" fn flare_igs_negate_change(
    image_array: *mut u8, height: i32, width: i32, channels: i32, bits: i32, sw_array: *const bool,
) -> i32 {
    change(image_array, height, width, channels, bits, sw_array)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgba8_rgb_use_alpha_and_alpha_negated_last() {
        // one pixel B,G,R,A = 10,20,30,100 ; negate R,G,B and A
        let mut px = [10u8, 20, 30, 100];
        let sw = [true, true, true, true];
        assert_eq!(unsafe { change(px.as_mut_ptr(), 1, 1, 4, 8, sw.as_ptr()) }, OK);
        // colour = alpha - colour (premultiplied negate), alpha = !alpha
        assert_eq!(px, [90, 80, 70, 155]);
    }

    #[test]
    fn colour_larger_than_alpha_clamps_to_zero() {
        let mut px = [200u8, 100, 50, 100];
        let sw = [true, true, true, false];
        unsafe { change(px.as_mut_ptr(), 1, 1, 4, 8, sw.as_ptr()) };
        assert_eq!(px, [0, 0, 50, 100]);
    }

    #[test]
    fn switches_are_in_rgb_order_on_toonz6_layout() {
        // sw[0] drives red = index 2 of B,G,R,A
        let mut px = [10u8, 20, 30, 100];
        let sw = [true, false, false, false];
        unsafe { change(px.as_mut_ptr(), 1, 1, 4, 8, sw.as_ptr()) };
        assert_eq!(px, [10, 20, 70, 100]);
    }

    #[test]
    fn rgb_plain_complement() {
        let mut px = [1u8, 2, 3, 10, 20, 30];
        let sw = [true, false, true];
        unsafe { change(px.as_mut_ptr(), 1, 2, 3, 8, sw.as_ptr()) };
        // sw[0]=red (index 2), sw[2]=blue (index 0)
        assert_eq!(px, [!1u8, 2, !3, !10u8, 20, !30]);
    }

    #[test]
    fn gray_and_16bit() {
        let mut g = [0u8, 255, 17];
        unsafe { change(g.as_mut_ptr(), 1, 3, 1, 8, [true].as_ptr()) };
        assert_eq!(g, [255, 0, 238]);
        let mut p = [1000u16, 2000, 3000, 4000];
        unsafe { change(p.as_mut_ptr() as *mut u8, 1, 1, 4, 16, [true, true, true, true].as_ptr()) };
        // B,G,R,A: colours = alpha - c ; alpha = !alpha
        assert_eq!(p, [3000, 2000, 1000, !4000u16]);
    }

    #[test]
    fn one_bit_inverts_all_bytes_of_padded_rows() {
        // width 10, 1 channel, 1 bit -> 10 bits = 2 bytes per row; 2 rows
        let mut b = [0x0Fu8, 0xF0, 0xAA, 0x55];
        assert_eq!(unsafe { change(b.as_mut_ptr(), 2, 10, 1, 1, [true].as_ptr()) }, OK);
        assert_eq!(b, [0xF0, 0x0F, 0x55, 0xAA]);
    }

    #[test]
    fn errors() {
        let mut b = [0u8; 8];
        let sw = [true; 4];
        assert_eq!(unsafe { change(b.as_mut_ptr(), 1, 1, 2, 8, sw.as_ptr()) }, BAD_CHANNELS);
        assert_eq!(unsafe { change(b.as_mut_ptr(), 1, 1, 4, 32, sw.as_ptr()) }, BAD_BITS);
        // switch off: untouched
        let mut px = [1u8, 2, 3, 4];
        unsafe { change(px.as_mut_ptr(), 1, 1, 4, 8, [false; 4].as_ptr()) };
        assert_eq!(px, [1, 2, 3, 4]);
    }
}
