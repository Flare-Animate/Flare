//! Port of `flarelib/sandor_fxs/PatternMapParam.cpp`: the argv parser of the pattern-map
//! effect (`CPatternMapParam(argc, argv, shrink)` with the 12 positional arguments).
//!
//! `CPatternMapParam` keeps its public fields, `null()`, `testValue()` and `isOK()` inline
//! in `PatternMapParam.h`. Argument 0 (the ink list) is handed to `CCIL::set` by the
//! header; [`parse`] covers arguments 1..=11.
//!
//! Number parsing calls the C runtime's own `atof` / `atoi` rather than reimplementing
//! them, so inputs like `"1e3"`, `"  .5"`, `"0x10"`, `"inf"`, `"nan"` or `"12abc"` parse
//! exactly as before on whatever platform the binary runs on. `std::max` / `std::min` are
//! reproduced with their NaN behaviour (`max(a,b) = a < b ? b : a`, `min(a,b) = b < a ? b : a`).
//!
//! Deviation: a null `argv` or null argument pointer, which crashed the C++, returns -1
//! and leaves the output untouched (the header then keeps the `null()` defaults).

use std::os::raw::{c_char, c_double, c_int};

extern "C" {
    fn atof(s: *const c_char) -> c_double;
    fn atoi(s: *const c_char) -> c_int;
}

/// The numeric/boolean fields of `CPatternMapParam` (everything but `m_ink`/`m_patternFn`).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PmpFields {
    pub max_scale: f64,
    pub min_scale: f64,
    pub max_dir_angle: f64,
    pub min_dir_angle: f64,
    pub max_dist: f64,
    pub min_dist: f64,
    pub density: f64,
    pub is_random_dir: c_int,
    pub is_keep_contour: c_int,
    pub is_use_ink_color: c_int,
    pub is_include_alpha: c_int,
}

fn cmax(a: f64, b: f64) -> f64 {
    if a < b { b } else { a }
}
fn cmin(a: f64, b: f64) -> f64 {
    if b < a { b } else { a }
}

/// # Safety
/// `argv[1..12]` must be valid NUL-terminated strings.
unsafe fn parse(argv: *const *const c_char, shrink: c_int) -> PmpFields {
    let f = |i: usize| atof(*argv.add(i));
    let n = |i: usize| atoi(*argv.add(i));
    let scale = if shrink > 0 { 1.0 / shrink as f64 } else { 1.0 };

    let (s1, s2) = (f(1) * scale, f(2) * scale);
    let max_scale = cmax(s1, s2);
    let min_scale = cmin(s1, s2);

    let (a1, a2) = (f(3), f(4));
    let max_dir_angle = cmax(a1, a2);
    let min_dir_angle = cmin(a1, a2);
    let is_random_dir = n(5) > 0;

    let dmax = cmax(f(6) * scale, 1.0);
    let dmin = cmax(f(7) * scale, 1.0);
    let max_dist = cmax(dmax, dmin);
    let min_dist = cmin(dmax, dmin);
    let density = if shrink > 0 { f(8) * shrink as f64 } else { f(8) };

    PmpFields {
        max_scale,
        min_scale,
        max_dir_angle,
        min_dir_angle,
        max_dist,
        min_dist,
        density,
        is_random_dir: is_random_dir as c_int,
        is_keep_contour: (n(9) > 0) as c_int,
        is_use_ink_color: (n(10) > 0) as c_int,
        is_include_alpha: (n(11) > 0) as c_int,
    }
}

/// Parse arguments 1..=11 of a 12-argument list. Returns 0 on success, -1 on null
/// pointers or `argc != 12` (nothing written).
#[no_mangle]
pub unsafe extern "C" fn flare_patternmapparam_parse(argc: c_int, argv: *const *const c_char, shrink: c_int, out: *mut PmpFields) -> c_int {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if argc != 12 || argv.is_null() || out.is_null() {
            return -1;
        }
        for i in 1..12 {
            if (*argv.add(i)).is_null() {
                return -1;
            }
        }
        *out = parse(argv, shrink);
        0
    }))
    .unwrap_or(-1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    fn run(args: [&str; 12], shrink: i32) -> PmpFields {
        let c: Vec<CString> = args.iter().map(|s| CString::new(*s).unwrap()).collect();
        let p: Vec<*const c_char> = c.iter().map(|s| s.as_ptr()).collect();
        let mut out = std::mem::MaybeUninit::<PmpFields>::uninit();
        unsafe {
            assert_eq!(flare_patternmapparam_parse(12, p.as_ptr(), shrink, out.as_mut_ptr()), 0);
            out.assume_init()
        }
    }

    #[test]
    fn matches_the_cpp_reference_bit_for_bit() {
        let text = include_str!("PatternMapParam_cases.txt").replace("\r\n", "\n");
        let mut n = 0;
        for block in text.split("\n\n").filter(|b| !b.trim().is_empty()) {
            let mut lines = block.lines();
            let shrink: i32 = lines.next().unwrap().strip_prefix("C shrink=").unwrap().parse().unwrap();
            let args_line = lines.next().unwrap().strip_prefix("A ").unwrap();
            // arguments are hex-escaped so that spaces/odd bytes survive the file
            let args: Vec<String> = args_line
                .split(' ')
                .map(|h| {
                    let b: Vec<u8> = (0..h.len() / 2).map(|i| u8::from_str_radix(&h[2 * i..2 * i + 2], 16).unwrap()).collect();
                    String::from_utf8(b).unwrap()
                })
                .collect();
            assert_eq!(args.len(), 12);
            let a: [&str; 12] = std::array::from_fn(|i| args[i].as_str());
            let g = run(a, shrink);
            let got = format!(
                "O {:016x} {:016x} {:016x} {:016x} {:016x} {:016x} {:016x} {} {} {} {}",
                g.max_scale.to_bits(),
                g.min_scale.to_bits(),
                g.max_dir_angle.to_bits(),
                g.min_dir_angle.to_bits(),
                g.max_dist.to_bits(),
                g.min_dist.to_bits(),
                g.density.to_bits(),
                g.is_random_dir,
                g.is_keep_contour,
                g.is_use_ink_color,
                g.is_include_alpha
            );
            let want = lines.next().unwrap();
            // NaN payload/sign is whatever the platform's atof/arith produced; compare NaN as class.
            assert_eq!(norm_nan(&got), norm_nan(want), "case {n}: shrink={shrink} args={args:?}");
            n += 1;
        }
        assert!(n > 100, "golden looks empty: {n}");
    }

    fn norm_nan(s: &str) -> String {
        s.split(' ')
            .map(|t| {
                if t.len() == 16 {
                    if let Ok(b) = u64::from_str_radix(t, 16) {
                        if f64::from_bits(b).is_nan() {
                            return "nan".to_string();
                        }
                    }
                }
                t.to_string()
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn ordinary_arguments() {
        let g = run(["ink", "2", "1", "90", "10", "1", "4", "6", "0.5", "1", "0", "1"], 2);
        assert_eq!((g.max_scale, g.min_scale), (1.0, 0.5));
        assert_eq!((g.max_dir_angle, g.min_dir_angle), (90.0, 10.0));
        assert_eq!((g.max_dist, g.min_dist), (3.0, 2.0));
        assert_eq!(g.density, 1.0);
        assert_eq!((g.is_random_dir, g.is_keep_contour, g.is_use_ink_color, g.is_include_alpha), (1, 1, 0, 1));
    }

    /// glibc's `atoi` wraps out-of-range values ("3000000000" -> negative), Windows' UCRT
    /// saturates; kept out of the shared golden for that reason.
    #[cfg(target_os = "linux")]
    #[test]
    fn atoi_overflow_wraps_on_glibc() {
        let g = run(["i", "1", "1", "0", "0", "3000000000", "1", "1", "0", "3000000000", "1", "1"], 0);
        assert_eq!((g.is_random_dir, g.is_keep_contour, g.is_use_ink_color), (0, 0, 1));
    }

    #[test]
    fn distances_are_floored_at_one() {
        let g = run(["i", "1", "1", "0", "0", "0", "0.2", "-5", "0", "0", "0", "0"], 0);
        assert_eq!((g.max_dist, g.min_dist), (1.0, 1.0));
    }

    #[test]
    fn null_checks() {
        unsafe {
            let mut out = std::mem::MaybeUninit::<PmpFields>::uninit();
            assert_eq!(flare_patternmapparam_parse(12, std::ptr::null(), 1, out.as_mut_ptr()), -1);
            let s = CString::new("1").unwrap();
            let mut p: Vec<*const c_char> = vec![s.as_ptr(); 12];
            assert_eq!(flare_patternmapparam_parse(11, p.as_ptr(), 1, out.as_mut_ptr()), -1);
            assert_eq!(flare_patternmapparam_parse(12, p.as_ptr(), 1, std::ptr::null_mut()), -1);
            p[7] = std::ptr::null();
            assert_eq!(flare_patternmapparam_parse(12, p.as_ptr(), 1, out.as_mut_ptr()), -1);
            p[7] = s.as_ptr();
            p[0] = std::ptr::null(); // arg 0 is the header's business
            assert_eq!(flare_patternmapparam_parse(12, p.as_ptr(), 1, out.as_mut_ptr()), 0);
        }
    }
}
