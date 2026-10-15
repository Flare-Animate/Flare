//! Port of sound/tsioutils.cpp: byte-swapping sample copies (big/little endian).
//! Safe slice functions plus the C ABI used by tsio_wav / tsio_aiff.
use std::slice;

pub fn swap_and_copy_16_bits(src: &[i16], dst: &mut [i16]) {
    for (d, s) in dst.iter_mut().zip(src) {
        *d = s.swap_bytes();
    }
}

pub fn swap_and_copy_32_bits(src: &[i32], dst: &mut [i32]) {
    for (d, s) in dst.iter_mut().zip(src) {
        *d = s.swap_bytes();
    }
}

/// Matches the original loop `while (--sampleCount)`: copies `sample_count - 1`
/// 3-byte samples, and nothing when `sample_count <= 0`.
pub fn swap_and_copy_24_bits(src: &[u8], dst: &mut [u8], sample_count: i32) {
    if sample_count <= 0 {
        return;
    }
    let n = (sample_count - 1) as usize;
    for (d, s) in dst.chunks_exact_mut(3).zip(src.chunks_exact(3)).take(n) {
        d[0] = s[2];
        d[1] = s[1];
        d[2] = s[0];
    }
}

/// # Safety
/// `src`/`dst` must be valid for `sample_count` samples.
#[no_mangle]
pub unsafe extern "C" fn swapAndCopy16Bits(src: *const i16, dst: *mut i16, sample_count: i32) {
    let n = sample_count.max(0) as usize;
    swap_and_copy_16_bits(slice::from_raw_parts(src, n), slice::from_raw_parts_mut(dst, n));
}

/// # Safety
/// `src`/`dst` must be valid for `sample_count` samples.
#[no_mangle]
pub unsafe extern "C" fn swapAndCopy32Bits(src: *const i32, dst: *mut i32, sample_count: i32) {
    let n = sample_count.max(0) as usize;
    swap_and_copy_32_bits(slice::from_raw_parts(src, n), slice::from_raw_parts_mut(dst, n));
}

/// # Safety
/// `src`/`dst` must be valid for `sample_count * 3` bytes.
#[no_mangle]
pub unsafe extern "C" fn swapAndCopy24Bits(src: *const u8, dst: *mut u8, sample_count: i32) {
    let n = sample_count.max(0) as usize * 3;
    swap_and_copy_24_bits(slice::from_raw_parts(src, n), slice::from_raw_parts_mut(dst, n), sample_count);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swaps_16_and_32() {
        let mut d = [0i16; 2];
        swap_and_copy_16_bits(&[0x0102, 0x0304], &mut d);
        assert_eq!(d, [0x0201, 0x0403]);
        let mut d = [0i32; 1];
        swap_and_copy_32_bits(&[0x01020304], &mut d);
        assert_eq!(d, [0x04030201]);
    }

    #[test]
    fn swaps_24_like_original() {
        let mut d = [0u8; 6];
        swap_and_copy_24_bits(&[1, 2, 3, 4, 5, 6], &mut d, 2);
        assert_eq!(d, [3, 2, 1, 0, 0, 0]); // count-1 samples, as in C++
        swap_and_copy_24_bits(&[1, 2, 3], &mut d, 0);
    }
}
