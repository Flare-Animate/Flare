//! Port of `common/tcolor/tcolorfunctions.cpp`: the per-pixel arithmetic of the
//! `TColorFunction` subclasses (`TColorFader`, `TGenericColorFunction`, `TTranspFader`,
//! `TOnionFader`, `TColumnColorFilterFunction`).
//!
//! The classes (virtual `operator()`, `clone()`, `getParameters()`, the stored colour/fade
//! members) stay in `tcolorfunctions.h` and are now inline; each `operator()` /
//! `getParameters()` hands its scalar members to the entry points below and gets the new
//! channels (in r, g, b, m order, independent of the machine channel order of `TPixel32`)
//! back through `out`. The `assert(false)` in `TTranspFader::getParameters` and
//! `TColumnColorFilterFunction::getParameters` stays in the header.
//!
//! Faithful to the C++: `blend(color, m_color, fade)` rounds with `troundp` (`(int)(x + 0.5)`,
//! so negative results round toward zero) and stores the *int* into the 8 bit channel
//! (low byte); `(int)` of a double that does not fit follows x86 `cvttsd2si` (`INT_MIN`), see
//! [`f2i`] -- the C++ was undefined there, a given `fade` outside `[0, 1]` or a NaN gets that
//! deterministic answer on every platform.

/// `(int)x` as the x86 `cvttsd2si` instruction computes it: truncation toward zero, and the
/// "integer indefinite" value `INT_MIN` for NaN and for anything that does not fit in an
/// `int` (the C++ is undefined behaviour there, Rust's `as` would saturate).
pub fn f2i(x: f64) -> i32 {
    if x.is_nan() || x >= 2147483648.0 || x <= -2147483649.0 {
        i32::MIN
    } else {
        x as i32
    }
}

/// `troundp(x)`: `(int)(x + 0.5F)`.
pub fn troundp(x: f64) -> i32 {
    f2i(x + 0.5)
}

/// `blend(a, b, t)` for `TPixel32`: `troundp((1 - t) * a + t * b)` per channel, truncated to
/// the byte the `TPixel32(int, int, int, int)` constructor stores.
pub fn blend32(a: [u32; 4], b: [u32; 4], t: f64) -> [u8; 4] {
    let mut o = [0u8; 4];
    for i in 0..4 {
        o[i] = troundp((1.0 - t) * a[i] as f64 + t * b[i] as f64) as u8;
    }
    o
}

/// `tcrop(x, a, b) == std::min(std::max(x, a), b)` with the exact `std::` tie/NaN behaviour.
fn tcrop(x: f64, a: f64, b: f64) -> f64 {
    let mx = if x < a { a } else { x }; // std::max(x, a)
    if b < mx {
        b
    } else {
        mx
    } // std::min(mx, b)
}

type Px = [u32; 4];

fn px(r: u32, g: u32, b: u32, m: u32) -> Px {
    [r & 0xff, g & 0xff, b & 0xff, m & 0xff]
}

unsafe fn put<const N: usize, T: Copy>(out: *mut T, v: [T; N]) -> i32 {
    if out.is_null() {
        return 1;
    }
    for (i, x) in v.iter().enumerate() {
        *out.add(i) = *x;
    }
    0
}

/// `TColorFader::operator()`: `blend(color, m_color, m_fade)`. `out` receives r, g, b, m.
///
/// # Safety
/// `out` must be NULL or valid for 4 bytes.
#[no_mangle]
pub unsafe extern "C" fn flare_tcolorfader_apply(r: u32, g: u32, b: u32, m: u32, fr: u32, fg: u32, fb: u32, fm: u32, fade: f64, out: *mut u8) -> i32 {
    crate::guard(2, || put(out, blend32(px(r, g, b, m), px(fr, fg, fb, fm), fade)))
}

/// `TColorFader::getParameters`: `out` receives mR, mG, mB, mM, cR, cG, cB, cM.
///
/// # Safety
/// `out` must be NULL or valid for 8 doubles.
#[no_mangle]
pub unsafe extern "C" fn flare_tcolorfader_params(fr: u32, fg: u32, fb: u32, fm: u32, fade: f64, out: *mut f64) -> i32 {
    crate::guard(2, || {
        let k = 1.0 - fade;
        put(out, [k, k, k, k, fade * (fr & 0xff) as f64, fade * (fg & 0xff) as f64, fade * (fb & 0xff) as f64, fade * (fm & 0xff) as f64])
    })
}

/// `TGenericColorFunction::operator()`: per channel `(int)tcrop(m[i] * c + k[i], 0, 255)`.
///
/// # Safety
/// `mult` and `add` must be NULL or valid for 4 doubles, `out` for 4 bytes.
#[no_mangle]
pub unsafe extern "C" fn flare_tgeneric_apply(r: u32, g: u32, b: u32, m: u32, mult: *const f64, add: *const f64, out: *mut u8) -> i32 {
    if mult.is_null() || add.is_null() {
        return 1;
    }
    crate::guard(2, || {
        let c = px(r, g, b, m);
        let mut o = [0u8; 4];
        for i in 0..4 {
            o[i] = f2i(tcrop(*mult.add(i) * c[i] as f64 + *add.add(i), 0.0, 255.0)) as u8;
        }
        put(out, o)
    })
}

/// `TTranspFader::operator()`: the colour with `m = (int)(transp * m)`.
///
/// # Safety
/// `out` must be NULL or valid for 4 bytes.
#[no_mangle]
pub unsafe extern "C" fn flare_ttranspfader_apply(r: u32, g: u32, b: u32, m: u32, transp: f64, out: *mut u8) -> i32 {
    crate::guard(2, || {
        let c = px(r, g, b, m);
        put(out, [c[0] as u8, c[1] as u8, c[2] as u8, f2i(transp * c[3] as f64) as u8])
    })
}

/// `TOnionFader::operator()`: transparent pixels stay, the others are blended with the fade
/// colour and get the fixed matte 180.
///
/// # Safety
/// `out` must be NULL or valid for 4 bytes.
#[no_mangle]
pub unsafe extern "C" fn flare_tonionfader_apply(r: u32, g: u32, b: u32, m: u32, fr: u32, fg: u32, fb: u32, fm: u32, fade: f64, out: *mut u8) -> i32 {
    crate::guard(2, || {
        let c = px(r, g, b, m);
        if c[3] == 0 {
            return put(out, [c[0] as u8, c[1] as u8, c[2] as u8, 0]);
        }
        let mut o = blend32(c, px(fr, fg, fb, fm), fade);
        o[3] = 180;
        put(out, o)
    })
}

/// `TOnionFader::getParameters`: `out` receives mR, mG, mB, mM, cR, cG, cB, cM.
///
/// # Safety
/// `out` must be NULL or valid for 8 doubles.
#[no_mangle]
pub unsafe extern "C" fn flare_tonionfader_params(fr: u32, fg: u32, fb: u32, fm: u32, fade: f64, out: *mut f64) -> i32 {
    crate::guard(2, || {
        let k = 1.0 - fade;
        put(out, [k, k, k, 1.0, fade * (fr & 0xff) as f64, fade * (fg & 0xff) as f64, fade * (fb & 0xff) as f64, (fm & 0xff) as f64])
    })
}

/// `TColumnColorFilterFunction::operator()`: `255 - (255 - c) * (255 - s) / 255` per colour
/// channel, `m * s.m / 255` for the matte (all `int` arithmetic).
///
/// # Safety
/// `out` must be NULL or valid for 4 bytes.
#[no_mangle]
pub unsafe extern "C" fn flare_tcolumnfilter_apply(r: u32, g: u32, b: u32, m: u32, sr: u32, sg: u32, sb: u32, sm: u32, out: *mut u8) -> i32 {
    crate::guard(2, || {
        let (c, s) = (px(r, g, b, m), px(sr, sg, sb, sm));
        let f = |c: u32, s: u32| (255 - (255 - c as i32) * (255 - s as i32) / 255) as u8;
        put(out, [f(c[0], s[0]), f(c[1], s[1]), f(c[2], s[2]), (c[3] as i32 * s[3] as i32 / 255) as u8])
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f2i_follows_cvttsd2si() {
        assert_eq!(f2i(1.9), 1);
        assert_eq!(f2i(-1.9), -1);
        assert_eq!(f2i(2147483647.5), 2147483647);
        assert_eq!(f2i(-2147483648.9), i32::MIN);
        assert_eq!(f2i(2147483648.0), i32::MIN);
        assert_eq!(f2i(f64::NAN), i32::MIN);
        assert_eq!(f2i(f64::INFINITY), i32::MIN);
        assert_eq!(f2i(f64::NEG_INFINITY), i32::MIN);
    }

    #[test]
    fn fader_endpoints_and_rounding() {
        let mut o = [0u8; 4];
        unsafe {
            assert_eq!(flare_tcolorfader_apply(10, 20, 30, 40, 200, 100, 50, 255, 0.0, o.as_mut_ptr()), 0);
            assert_eq!(o, [10, 20, 30, 40]);
            assert_eq!(flare_tcolorfader_apply(10, 20, 30, 40, 200, 100, 50, 255, 1.0, o.as_mut_ptr()), 0);
            assert_eq!(o, [200, 100, 50, 255]);
            assert_eq!(flare_tcolorfader_apply(10, 20, 30, 40, 200, 100, 50, 255, 0.5, o.as_mut_ptr()), 0);
            assert_eq!(o, [105, 60, 40, 148]); // 147.5 + 0.5 truncates up
            assert_eq!(flare_tcolorfader_apply(0, 0, 0, 0, 0, 0, 0, 0, 0.5, std::ptr::null_mut()), 1);
        }
    }

    #[test]
    fn fader_params() {
        let mut p = [0.0f64; 8];
        unsafe {
            assert_eq!(flare_tcolorfader_params(10, 20, 30, 40, 0.25, p.as_mut_ptr()), 0);
            assert_eq!(p, [0.75, 0.75, 0.75, 0.75, 2.5, 5.0, 7.5, 10.0]);
            assert_eq!(flare_tonionfader_params(10, 20, 30, 40, 0.25, p.as_mut_ptr()), 0);
            assert_eq!(p, [0.75, 0.75, 0.75, 1.0, 2.5, 5.0, 7.5, 40.0]);
            assert_eq!(flare_tonionfader_params(0, 0, 0, 0, 0.25, std::ptr::null_mut()), 1);
        }
    }

    #[test]
    fn generic_clamps_and_truncates() {
        let m = [1.0, 0.5, 2.0, 1.0];
        let k = [0.0, 10.5, -100.0, 300.0];
        let mut o = [0u8; 4];
        unsafe {
            assert_eq!(flare_tgeneric_apply(100, 100, 100, 100, m.as_ptr(), k.as_ptr(), o.as_mut_ptr()), 0);
            assert_eq!(o, [100, 60, 100, 255]);
            assert_eq!(flare_tgeneric_apply(0, 0, 0, 0, std::ptr::null(), k.as_ptr(), o.as_mut_ptr()), 1);
            // NaN passes through tcrop untouched and (int)NaN is INT_MIN: low byte 0
            let nan = [f64::NAN; 4];
            assert_eq!(flare_tgeneric_apply(1, 2, 3, 4, nan.as_ptr(), k.as_ptr(), o.as_mut_ptr()), 0);
            assert_eq!(o, [0, 0, 0, 0]);
        }
    }

    #[test]
    fn transp_onion_and_column_filter() {
        let mut o = [0u8; 4];
        unsafe {
            assert_eq!(flare_ttranspfader_apply(1, 2, 3, 200, 0.5, o.as_mut_ptr()), 0);
            assert_eq!(o, [1, 2, 3, 100]);
            assert_eq!(flare_tonionfader_apply(9, 9, 9, 0, 255, 255, 255, 255, 0.5, o.as_mut_ptr()), 0);
            assert_eq!(o, [9, 9, 9, 0]);
            assert_eq!(flare_tonionfader_apply(0, 0, 0, 255, 255, 255, 255, 255, 0.5, o.as_mut_ptr()), 0);
            assert_eq!(o, [128, 128, 128, 180]);
            assert_eq!(flare_tcolumnfilter_apply(0, 128, 255, 255, 255, 255, 255, 128, o.as_mut_ptr()), 0);
            assert_eq!(o, [255, 255, 255, 128]); // a white scale leaves everything white
            assert_eq!(flare_tcolumnfilter_apply(100, 100, 100, 100, 0, 0, 0, 255, o.as_mut_ptr()), 0);
            assert_eq!(o, [100, 100, 100, 100]); // a black scale is the identity
        }
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

    fn dbl(b: u64) -> f64 {
        f64::from_bits(b)
    }

    // Replays the colour functions recorded from the original C++ (tcolor_cases.txt).
    #[test]
    fn replay_recorded_cpp_cases() {
        let (mut nf, mut np, mut ng) = (0, 0, 0);
        for line in include_str!("tcolor_cases.txt").lines() {
            let (tag, g) = parse(line);
            let u = |v: u64| v as u32;
            match tag {
                "fn" => {
                    let i = &g[0];
                    let fade = dbl(i[8]);
                    let mut o = [0u8; 4];
                    let want = |k: usize| g[k].iter().map(|&x| x as u8).collect::<Vec<u8>>();
                    unsafe {
                        assert_eq!(flare_tcolorfader_apply(u(i[0]), u(i[1]), u(i[2]), u(i[3]), u(i[4]), u(i[5]), u(i[6]), u(i[7]), fade, o.as_mut_ptr()), 0);
                        assert_eq!(o.to_vec(), want(1), "{line}");
                        assert_eq!(flare_tonionfader_apply(u(i[0]), u(i[1]), u(i[2]), u(i[3]), u(i[4]), u(i[5]), u(i[6]), u(i[7]), fade, o.as_mut_ptr()), 0);
                        assert_eq!(o.to_vec(), want(2), "{line}");
                        assert_eq!(flare_ttranspfader_apply(u(i[0]), u(i[1]), u(i[2]), u(i[3]), fade, o.as_mut_ptr()), 0);
                        assert_eq!(o.to_vec(), want(3), "{line}");
                        assert_eq!(flare_tcolumnfilter_apply(u(i[0]), u(i[1]), u(i[2]), u(i[3]), u(i[4]), u(i[5]), u(i[6]), u(i[7]), o.as_mut_ptr()), 0);
                        assert_eq!(o.to_vec(), want(4), "{line}");
                    }
                    nf += 1;
                }
                "fnp" => {
                    let i = &g[0];
                    let fade = dbl(i[4]);
                    let mut p = [0f64; 8];
                    unsafe {
                        assert_eq!(flare_tcolorfader_params(u(i[0]), u(i[1]), u(i[2]), u(i[3]), fade, p.as_mut_ptr()), 0);
                        assert_eq!(p.map(f64::to_bits).to_vec(), g[1], "{line}");
                        assert_eq!(flare_tonionfader_params(u(i[0]), u(i[1]), u(i[2]), u(i[3]), fade, p.as_mut_ptr()), 0);
                        assert_eq!(p.map(f64::to_bits).to_vec(), g[2], "{line}");
                    }
                    np += 1;
                }
                "fng" => {
                    let i = &g[0];
                    let m: Vec<f64> = i[4..8].iter().map(|&b| dbl(b)).collect();
                    let k: Vec<f64> = i[8..12].iter().map(|&b| dbl(b)).collect();
                    let mut o = [0u8; 4];
                    unsafe { assert_eq!(flare_tgeneric_apply(u(i[0]), u(i[1]), u(i[2]), u(i[3]), m.as_ptr(), k.as_ptr(), o.as_mut_ptr()), 0) };
                    assert_eq!(o.iter().map(|&x| x as u64).collect::<Vec<_>>(), g[1], "{line}");
                    ng += 1;
                }
                _ => {}
            }
        }
        assert_eq!((nf, np, ng), (300, 300, 300));
    }
}
