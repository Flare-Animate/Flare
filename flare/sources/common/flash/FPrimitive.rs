//! Port of `common/flash/FPrimitive.cpp`: the SWF record writers of the low-level
//! primitives `FColor`, `FMatrix`, `FRect` and `FString`, plus `FMatrix::MinBits`.
//!
//! The classes stay in `FPrimitive.h` (they are copied around by value and carry public
//! fields), with their constructors and `FMatrix::operator*` inline; each
//! `WriteToSWFStream` hands its fields to the matching `flare_f*_write` here, together with
//! the `FSWFStream` handle (see `FSWFStream.rs`). The bit sequences are the original ones, so
//! the output is byte-identical (golden `swfwriter_cases.txt`).
//!
//! Deviation (header side): the identity `FMatrix()` constructor left the bit counts and the
//! translation uninitialised; the header now zero-initialises them (never observable, since an
//! identity matrix is written as one zero byte).
//! `abs(INT_MIN)` keeps its x86 value, as in `FSWFStream::MaxNum`.

use crate::fswfstream::{max_num, min_bits, Stream};
use std::os::raw::c_void;

type H = *mut Stream;

unsafe fn with(h: *mut c_void, f: impl FnOnce(&mut Stream)) -> i32 {
    if h.is_null() {
        return 1;
    }
    crate::guard(2, || {
        f(&mut *(h as H));
        0
    })
}

/// `FColor::WriteToSWFStream`: three bytes, plus the alpha byte when `alpha_t != 0`.
///
/// # Safety
/// `h` must be NULL or a live `FSWFStream` handle.
#[no_mangle]
pub unsafe extern "C" fn flare_fcolor_write(h: *mut c_void, red: u32, green: u32, blue: u32, alpha: u32, alpha_t: i32) -> i32 {
    with(h, |s| {
        s.write_byte(red);
        s.write_byte(green);
        s.write_byte(blue);
        if alpha_t != 0 {
            s.write_byte(alpha);
        }
    })
}

/// `FMatrix::MinBits(x, y)`.
pub fn matrix_min_bits(x: i32, y: i32) -> u32 {
    let x_abs = x.wrapping_abs();
    let y_abs = y.wrapping_abs();
    if x_abs > y_abs {
        min_bits(x_abs as u32, 1)
    } else {
        min_bits(y_abs as u32, 1)
    }
}

#[no_mangle]
pub extern "C" fn flare_fmatrix_min_bits(x: i32, y: i32) -> u32 {
    crate::guard(0, || matrix_min_bits(x, y))
}

/// `FMatrix::WriteToSWFStream`. `identity` is the `U16` flag; the rest are the public fields.
///
/// # Safety
/// `h` must be NULL or a live `FSWFStream` handle.
#[no_mangle]
pub unsafe extern "C" fn flare_fmatrix_write(
    h: *mut c_void,
    identity: u16,
    has_scale: u32,
    n_scale_bits: u32,
    scale_x: i32,
    scale_y: i32,
    has_rotate: u32,
    n_rotate_bits: u32,
    rotate_skew0: i32,
    rotate_skew1: i32,
    n_translate_bits: u32,
    translate_x: i32,
    translate_y: i32,
) -> i32 {
    with(h, |s| {
        if identity != 0 {
            s.write_byte(0);
        } else {
            s.flush_bits();
            s.write_bits(has_scale, 1);
            if has_scale != 0 {
                s.write_bits(n_scale_bits, 5);
                s.write_bits(scale_x as u32, n_scale_bits);
                s.write_bits(scale_y as u32, n_scale_bits);
            }
            s.write_bits(has_rotate, 1);
            if has_rotate != 0 {
                s.write_bits(n_rotate_bits, 5);
                s.write_bits(rotate_skew0 as u32, n_rotate_bits);
                s.write_bits(rotate_skew1 as u32, n_rotate_bits);
            }
            if translate_x == 0 && translate_y == 0 {
                s.write_bits(0, 5);
            } else {
                s.write_bits(n_translate_bits, 5);
                s.write_bits(translate_x as u32, n_translate_bits);
                s.write_bits(translate_y as u32, n_translate_bits);
            }
        }
    })
}

/// `FRect::MinBits`: bits for the largest coordinate magnitude, signed.
pub fn rect_min_bits(xmin: i32, xmax: i32, ymin: i32, ymax: i32) -> u32 {
    min_bits(max_num(xmin, xmax, ymin, ymax), 1)
}

#[no_mangle]
pub extern "C" fn flare_frect_min_bits(xmin: i32, xmax: i32, ymin: i32, ymax: i32) -> u32 {
    crate::guard(0, || rect_min_bits(xmin, xmax, ymin, ymax))
}

/// `FRect::WriteToSWFStream`.
///
/// # Safety
/// `h` must be NULL or a live `FSWFStream` handle.
#[no_mangle]
pub unsafe extern "C" fn flare_frect_write(h: *mut c_void, xmin: i32, xmax: i32, ymin: i32, ymax: i32) -> i32 {
    with(h, |s| {
        let n_bits = rect_min_bits(xmin, xmax, ymin, ymax);
        s.write_bits(n_bits, 5);
        s.write_bits(xmin as u32, n_bits);
        s.write_bits(xmax as u32, n_bits);
        s.write_bits(ymin as u32, n_bits);
        s.write_bits(ymax as u32, n_bits);
        s.flush_bits();
    })
}

/// `FString::WriteToSWFStream`: the bytes, then a NUL when `null_end != 0`.
/// (The C++ looped with a `U16` counter, which never finished for strings of 65536+ bytes.)
///
/// # Safety
/// `h` must be NULL or a live `FSWFStream` handle; `text` valid for `len` bytes (or NULL
/// with `len == 0`).
#[no_mangle]
pub unsafe extern "C" fn flare_fstring_write(h: *mut c_void, text: *const u8, len: usize, null_end: i32) -> i32 {
    if h.is_null() || (text.is_null() && len > 0) {
        return 1;
    }
    let bytes: &[u8] = if len == 0 { &[] } else { std::slice::from_raw_parts(text, len) };
    with(h, |s| {
        for &b in bytes {
            s.write_byte(b as u32);
        }
        if null_end != 0 {
            s.write_byte(0);
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bytes(s: &mut Stream) -> Vec<u8> {
        let mut v = vec![0u8; s.full_size() as usize];
        assert!(s.write_to_memory(&mut v));
        v
    }

    #[test]
    fn color_writes_rgb_or_rgba() {
        let mut h = Stream::new();
        let p = &mut h as H as *mut c_void;
        unsafe {
            assert_eq!(flare_fcolor_write(p, 1, 2, 0x1ff, 9, 0), 0);
            assert_eq!(flare_fcolor_write(p, 4, 5, 6, 7, 1), 0);
            assert_eq!(flare_fcolor_write(std::ptr::null_mut(), 0, 0, 0, 0, 0), 1);
        }
        assert_eq!(bytes(&mut h), vec![1, 2, 0xff, 4, 5, 6, 7]);
    }

    #[test]
    fn matrix_min_bits_uses_the_larger_magnitude() {
        assert_eq!(matrix_min_bits(0, 0), 1);
        assert_eq!(matrix_min_bits(1, 0), 2);
        assert_eq!(matrix_min_bits(-5, 3), 4);
        assert_eq!(matrix_min_bits(3, -5), 4);
        assert_eq!(matrix_min_bits(255, 256), 10);
    }

    #[test]
    fn identity_matrix_is_one_zero_byte_and_translate_zero_is_five_bits() {
        let mut h = Stream::new();
        let p = &mut h as H as *mut c_void;
        unsafe {
            assert_eq!(flare_fmatrix_write(p, 1, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0), 0);
            // no scale, no rotate, zero translation: 1 + 1 + 5 bits, padded by the next write
            assert_eq!(flare_fmatrix_write(p, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0), 0);
        }
        assert_eq!(h.size(), 1);
        h.flush_bits();
        assert_eq!(bytes(&mut h), vec![0, 0]);
    }

    #[test]
    fn rect_matches_the_documented_layout() {
        // (0,0)-(20,10): max 20 -> 6 bits; 00110 then four 6-bit fields
        let mut h = Stream::new();
        unsafe {
            assert_eq!(flare_frect_write(&mut h as H as *mut c_void, 0, 20, 0, 10), 0);
        }
        assert_eq!(rect_min_bits(0, 20, 0, 10), 6);
        // 00110 000000 010100 000000 001010 + pad = 30 0a 00 50
        assert_eq!(bytes(&mut h), vec![0x30, 0x0a, 0x00, 0x50]);
    }

    #[test]
    fn string_and_null_checks() {
        let mut h = Stream::new();
        let p = &mut h as H as *mut c_void;
        unsafe {
            assert_eq!(flare_fstring_write(p, b"ab\xff".as_ptr(), 3, 1), 0);
            assert_eq!(flare_fstring_write(p, std::ptr::null(), 2, 0), 1);
            assert_eq!(flare_fstring_write(p, std::ptr::null(), 0, 0), 0);
        }
        assert_eq!(bytes(&mut h), vec![b'a', b'b', 0xff, 0]);
    }
    // Replays the primitives recorded from the original C++ (swfwriter_cases.txt).
    #[test]
    fn replay_recorded_cpp_cases() {
        fn unhex(s: &str) -> Vec<u8> {
            if s == "-" {
                return vec![];
            }
            (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap()).collect()
        }
        fn prefixed(p: u32) -> Stream {
            let mut s = Stream::new();
            if p > 0 {
                s.write_bits((1 << p) - 1, p);
            }
            s
        }
        // writes a matrix record the way FMatrix::WriteToSWFStream hands over its fields
        fn write_matrix(s: &mut Stream, f: [i32; 8], identity: u16) {
            let (hs, sx, sy, hr, r0, r1, tx, ty) = (f[0] as u32, f[1], f[2], f[3] as u32, f[4], f[5], f[6], f[7]);
            let (ns, nr, nt) = (matrix_min_bits(sx, sy), matrix_min_bits(r0, r1), matrix_min_bits(tx, ty));
            let p = s as *mut Stream as *mut c_void;
            unsafe { assert_eq!(flare_fmatrix_write(p, identity, hs, ns, sx, sy, hr, nr, r0, r1, nt, tx, ty), 0) };
        }
        let fields = |t: &str| -> [i32; 8] {
            let v: Vec<i32> = t.split(',').map(|x| x.parse().unwrap()).collect();
            [v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7]]
        };
        let mut counts = std::collections::BTreeMap::<String, usize>::new();
        for line in include_str!("swfwriter_cases.txt").lines() {
            let f: Vec<&str> = line.split('|').collect();
            let n = |i: usize| f[i].parse::<i64>().unwrap();
            match f[0] {
                "color" => {
                    let kind = n(1);
                    let alpha = if kind == 0 || kind == 2 { 255 } else { n(5) as u32 };
                    let mut s = prefixed(n(6) as u32);
                    unsafe { assert_eq!(flare_fcolor_write(&mut s as *mut Stream as *mut c_void, n(2) as u32, n(3) as u32, n(4) as u32, alpha, n(7) as i32), 0) };
                    assert_eq!(bytes(&mut s), unhex(f[8]), "{line}");
                }
                "matrix" => {
                    let mut s = prefixed(n(9) as u32);
                    let v = [n(1) as i32, n(2) as i32, n(3) as i32, n(4) as i32, n(5) as i32, n(6) as i32, n(7) as i32, n(8) as i32];
                    // the constructor's bit counts
                    let bits: Vec<u32> = f[10].split(',').map(|x| x.parse().unwrap()).collect();
                    assert_eq!(bits, vec![matrix_min_bits(v[1], v[2]), matrix_min_bits(v[4], v[5]), matrix_min_bits(v[6], v[7])], "{line}");
                    write_matrix(&mut s, v, 0);
                    assert_eq!(bytes(&mut s), unhex(f[11]), "{line}");
                }
                "matrixid" => {
                    let mut s = prefixed(n(1) as u32);
                    write_matrix(&mut s, [0, 1, 1, 0, 0, 0, 0, 0], 1);
                    assert_eq!(bytes(&mut s), unhex(f[3]), "{line}");
                }
                "matmul" => {
                    let c: Vec<&str> = f[3].split(',').collect(); // product fields + identity
                    let mut s = Stream::new();
                    write_matrix(&mut s, fields(&c[..8].join(",")), c[8].parse().unwrap());
                    assert_eq!(bytes(&mut s), unhex(f[4]), "{line}");
                }
                "fmmin" => assert_eq!(matrix_min_bits(n(1) as i32, n(2) as i32) as i64, n(3), "{line}"),
                "rect" => {
                    let mut s = prefixed(n(5) as u32);
                    unsafe { assert_eq!(flare_frect_write(&mut s as *mut Stream as *mut c_void, n(1) as i32, n(2) as i32, n(3) as i32, n(4) as i32), 0) };
                    assert_eq!(bytes(&mut s), unhex(f[6]), "{line}");
                }
                "string" => {
                    let t = unhex(f[1]);
                    assert_eq!(t.len() as i64, n(4));
                    let mut s = prefixed(n(3) as u32);
                    unsafe { assert_eq!(flare_fstring_write(&mut s as *mut Stream as *mut c_void, t.as_ptr(), t.len(), n(2) as i32), 0) };
                    assert_eq!(bytes(&mut s), unhex(f[5]), "{line}");
                }
                _ => continue,
            }
            *counts.entry(f[0].to_string()).or_default() += 1;
        }
        for (k, c) in [("color", 40), ("matrix", 160), ("matrixid", 3), ("matmul", 60), ("fmmin", 11), ("rect", 80), ("string", 30)] {
            assert_eq!(counts[k], c, "{k}");
        }
    }
}
