//! Port of tnzext/Types.cpp: the parity tests behind `ToonzExt::OddInt` /
//! `EvenInt`.
//!
//! Only the arithmetic moves. The conversion operators throw
//! `std::range_error("Value is Even!!!")` / `("Value is Odd!!!")`, and a C++
//! exception cannot be thrown from Rust, so `include/ext/Types.h` keeps the
//! `throw` inline and asks Rust whether the value is odd/even.

/// `(bool)(v & 1)` -- also correct for negative `v` (two's complement).
pub fn is_odd(v: i32) -> bool {
    v & 1 != 0
}

/// `(bool)(!(v & 1))`.
pub fn is_even(v: i32) -> bool {
    v & 1 == 0
}

#[no_mangle]
pub extern "C" fn flare_ext_is_odd(v: i32) -> bool {
    std::panic::catch_unwind(|| is_odd(v)).unwrap_or(false)
}

#[no_mangle]
pub extern "C" fn flare_ext_is_even(v: i32) -> bool {
    std::panic::catch_unwind(|| is_even(v)).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_cpp_bit_tests() {
        // The C++ expressions, evaluated verbatim, over a spread incl. negatives and extremes.
        let mut vals: Vec<i32> = (-1000..=1000).collect();
        vals.extend([i32::MIN, i32::MIN + 1, i32::MAX, i32::MAX - 1]);
        for v in vals {
            assert_eq!(flare_ext_is_odd(v), (v & 1) != 0, "{v}");
            assert_eq!(flare_ext_is_even(v), (v & 1) == 0, "{v}");
            assert_ne!(flare_ext_is_odd(v), flare_ext_is_even(v));
        }
        assert!(is_odd(-3) && is_even(-4) && is_even(0));
    }
}
