//! Port of stdfx/igs_math_random.cpp: portable linear congruential generator
//! (glibc-style constants 1103515245 / 12345) used by the hls/hsv noise fx.
//! `unsigned long` / `long` follow the C++ ABI of the target (c_ulong / c_long).

use std::os::raw::{c_long, c_ulong};

/// igs::math::random::random(): seed_(1)
pub const DEFAULT_SEED: c_ulong = 1;

/// igs::math::random::next(): advances `seed` and returns 0..=long::MAX
pub fn next(seed: &mut c_ulong) -> c_long {
    *seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
    /* 0x41C64E6D + 0x3039 */
    (*seed % ((c_long::MAX as c_ulong) + 1)) as c_long
}

/// igs::math::random::next_d(): 0 ... 1
pub fn next_d(seed: &mut c_ulong) -> f64 {
    next(seed) as f64 / c_long::MAX as f64
}

/// # Safety
/// `seed` must be valid for reads and writes.
#[no_mangle]
pub unsafe extern "C" fn flare_igs_random_next(seed: *mut c_ulong) -> c_long {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    next(&mut *seed)
})).unwrap_or(0)
}

/// # Safety
/// `seed` must be valid for reads and writes.
#[no_mangle]
pub unsafe extern "C" fn flare_igs_random_next_d(seed: *mut c_ulong) -> f64 {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    next_d(&mut *seed)
})).unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_values_from_default_seed() {
        // seed 1 -> 1*1103515245 + 12345 = 1103527590
        let mut s = DEFAULT_SEED;
        assert_eq!(next(&mut s), 1103527590);
        assert_eq!(s, 1103527590);
        // second step: 1103527590*1103515245 + 12345 (mod 2^BITS) then % 2^(BITS-1)
        let expect = (1103527590 as c_ulong).wrapping_mul(1103515245).wrapping_add(12345);
        assert_eq!(next(&mut s), (expect % ((c_long::MAX as c_ulong) + 1)) as c_long);
    }

    #[test]
    fn stays_in_range_and_next_d_in_unit_interval() {
        let mut s: c_ulong = 42;
        for _ in 0..1000 {
            let v = next(&mut s);
            assert!(v >= 0);
            let d = next_d(&mut s);
            assert!((0.0..=1.0).contains(&d));
        }
    }

    #[test]
    fn seed_zero_gives_constant_then_progresses() {
        let mut s: c_ulong = 0;
        assert_eq!(next(&mut s), 12345);
    }

    /// The port must reproduce the *C++ ABI of this target*, not one global stream:
    /// `unsigned long` is 64-bit on LP64 (macOS, Linux) and 32-bit on LLP64 (Windows), and
    /// both the wrap modulus and the `next_d` denominator derive from that width. Assert
    /// the width-correct recurrence from the closed form, so Windows gets real RNG
    /// coverage even where no golden can be recorded for it.
    #[test]
    fn follows_the_cpp_unsigned_long_abi_on_this_platform() {
        let bits = 8 * std::mem::size_of::<c_ulong>() as u32;
        assert!(bits == 32 || bits == 64, "unexpected c_ulong width: {bits}");
        let mask: u128 = if bits == 64 { u64::MAX as u128 } else { u32::MAX as u128 };
        let modulus: u128 = 1u128 << (bits - 1); // LONG_MAX + 1
        let denominator: u128 = modulus - 1; // LONG_MAX

        for seed0 in [0u64, 1, 42, 12345, u32::MAX as u64] {
            let mut got = seed0 as c_ulong;
            let mut want: u128 = seed0 as u128;
            for step in 0..8 {
                want = (want * 1103515245 + 12345) & mask;
                let expect = (want % modulus) as i64;
                assert_eq!(next(&mut got) as i64, expect, "next(): seed {seed0} step {step}");
            }

            let mut got = seed0 as c_ulong;
            let mut want: u128 = seed0 as u128;
            for step in 0..8 {
                want = (want * 1103515245 + 12345) & mask;
                let expect = (want % modulus) as f64 / denominator as f64;
                assert_eq!(next_d(&mut got), expect, "next_d(): seed {seed0} step {step}");
            }
        }
    }

    #[test]
    fn ffi() {
        let mut s: c_ulong = 1;
        unsafe {
            assert_eq!(flare_igs_random_next(&mut s), 1103527590);
            let d = flare_igs_random_next_d(&mut s);
            assert!((0.0..=1.0).contains(&d));
        }
    }
}
