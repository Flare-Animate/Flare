//! Port of `common/tcolor/tpixelutils.cpp`: the colour-space conversions (RGB <-> HSV, RGB <->
//! HLS) and the conversions between the pixel types (`toPixel32/64/D/F`, `toLinear`).
//!
//! `tpixelutils.h` keeps every public C++ signature as an inline forwarder (the templates
//! `blend`/`overPix`/`premult`/... were always header-only). The exports of tnzcore are gone:
//! each target that includes the header links `flare_formats` (tnzcore and everything that
//! links it transitively, plus the targets that link tnzcore by file name on Apple).
//!
//! ABI: scalar channels in (r, g, b, m order, so the machine channel order of the C++ structs
//! is irrelevant), results through `out` pointers (r, g, b, m order; `u8`/`u16`/`f32`/`f64`
//! arrays), every entry point returning 0, 1 (NULL pointer) or 2 (caught panic) -- the header
//! throws `std::invalid_argument` for a non-zero status.
//!
//! Faithful to the C++, including the quirks:
//!
//! * `hsv2rgb(TPixel32&, int[3], int)` writes the r/g/b bytes only in the six hue sectors; a
//!   hue that is still outside `[0, 360)` after the single wrap-around adjustment (e.g. a
//!   source of `3 * maxHsv`) leaves them unchanged and just sets `m = 255`. `HSV2RGB` leaves
//!   its outputs untouched the same way. `RGB2HSV` / `rgb2hls` read `*h` back (and scale it)
//!   when no channel equals the maximum (NaN input), exactly as the C++ did with whatever the
//!   caller had there.
//! * `std::max({r, g, b})` is `max_element` (first of equal maxima, `<` comparisons).
//! * `toPixel32(TPixelD/F)` / `toPixel64(TPixelD/F)` round with `tround` (so a negative value
//!   rounds away from zero before the cast), then clamp with `byteCrop`/`wordCrop`, which map
//!   anything negative or `INT_MIN` to 0 and anything above the maximum to the maximum.
//! * `toPixel32(TPixelGR8)` / `toPixel64(TPixelGR8)` produce a fully opaque grey; the
//!   `TPixelD` / `TPixelF` ones (`toPixelD(TPixelGR8)`) get the default matte 1.
//! * `toLinear` on the integer pixel types is `(Q)(max * pow(v / max, gamma) + 0.5)` (the
//!   matte is copied); the float type keeps negative values (`powf` only for `v >= 0`); the
//!   double type is a plain `pow`.
//!
//! Deviations (undefined behaviour in the C++): `(int)` of a NaN / out-of-range double
//! follows the x86 `cvttsd2si` answer `INT_MIN` ([`crate::tcolorfunctions::f2i`]) on every
//! platform instead of depending on the CPU, and the `(Q)` casts of `toLinear` keep the low
//! bits of that int (what gcc/clang emit for `unsigned char`/`unsigned short`). `maxHsv == 0`
//! (an `assert` in debug builds) divides by zero in floating point like the release C++.
//! `pow` is the platform libm, as before: results can differ between platforms by an ULP.

use crate::tcolorfunctions::f2i;

// ---------------------------------------------------------------------------------------
// HSV / HLS

fn tcrop_i(x: i32, a: i32, b: i32) -> i32 {
    // std::min(std::max(x, a), b)
    let mx = if x < a { a } else { x };
    if b < mx {
        b
    } else {
        mx
    }
}

fn max3(a: f64, b: f64, c: f64) -> f64 {
    let mut m = a;
    if m < b {
        m = b;
    }
    if m < c {
        m = c;
    }
    m
}

fn min3(a: f64, b: f64, c: f64) -> f64 {
    let mut m = a;
    if b < m {
        m = b;
    }
    if c < m {
        m = c;
    }
    m
}

fn byte_of(v: f64) -> u8 {
    tcrop_i(f2i(v * 255.0), 0, 255) as u8
}

/// `hsv2rgb(TPixel32 &dstRgb, int srcHsv[3], int maxHsv)` without the matte: updates `rgb`
/// (r, g, b) exactly where the C++ assigned `dstRgb`.
pub fn hsv2rgb(src: [i32; 3], max_hsv: i32, rgb: &mut [u8; 3]) {
    let mx = max_hsv as f64;
    let mut hue = (src[0] as f64 / mx) * 360.0;
    let mut sat = src[1] as f64 / mx;
    let mut value = src[2] as f64 / mx;

    if hue > 360.0 {
        hue -= 360.0;
    }
    if hue < 0.0 {
        hue += 360.0;
    }
    if sat < 0.0 {
        sat = 0.0;
    }
    if sat > 1.0 {
        sat = 1.0;
    }
    if value < 0.0 {
        value = 0.0;
    }
    if value > 1.0 {
        value = 1.0;
    }
    if sat == 0.0 {
        let v = byte_of(value);
        *rgb = [v, v, v];
    } else {
        if hue == 360.0 {
            hue = 0.0;
        }
        hue /= 60.0;
        let i = f2i(hue);
        let f = hue - i as f64;
        let p = value * (1.0 - sat);
        let q = value * (1.0 - (sat * f));
        let t = value * (1.0 - (sat * (1.0 - f)));
        match i {
            0 => *rgb = [byte_of(value), byte_of(t), byte_of(p)],
            1 => *rgb = [byte_of(q), byte_of(value), byte_of(p)],
            2 => *rgb = [byte_of(p), byte_of(value), byte_of(t)],
            3 => *rgb = [byte_of(p), byte_of(q), byte_of(value)],
            4 => *rgb = [byte_of(t), byte_of(p), byte_of(value)],
            5 => *rgb = [byte_of(value), byte_of(p), byte_of(q)],
            _ => {}
        }
    }
}

/// `HSV2RGB`: `out` is `Some((r, g, b))` where the C++ assigned them.
pub fn hsv2rgb_d(mut hue: f64, mut sat: f64, mut value: f64) -> Option<(f64, f64, f64)> {
    if hue > 360.0 {
        hue -= 360.0;
    }
    if hue < 0.0 {
        hue += 360.0;
    }
    if sat < 0.0 {
        sat = 0.0;
    }
    if sat > 1.0 {
        sat = 1.0;
    }
    if value < 0.0 {
        value = 0.0;
    }
    if value > 1.0 {
        value = 1.0;
    }
    if sat == 0.0 {
        return Some((value, value, value));
    }
    if hue == 360.0 {
        hue = 0.0;
    }
    hue /= 60.0;
    let i = f2i(hue);
    let f = hue - i as f64;
    let p = value * (1.0 - sat);
    let q = value * (1.0 - (sat * f));
    let t = value * (1.0 - (sat * (1.0 - f)));
    match i {
        0 => Some((value, t, p)),
        1 => Some((q, value, p)),
        2 => Some((p, value, t)),
        3 => Some((p, q, value)),
        4 => Some((t, p, value)),
        5 => Some((value, p, q)),
        _ => None,
    }
}

/// The hue/saturation/value shared by `RGB2HSV` and `rgb2hsv`. `h_in` is what `*h` held
/// before the call (read back when no channel equals the maximum).
fn rgb_to_hsv(r: f64, g: f64, b: f64, h_in: f64) -> (f64, f64, f64) {
    let max = max3(r, g, b);
    let min = min3(r, g, b);
    let v = max;
    let s = if max != 0.0 { (max - min) / max } else { 0.0 };
    let h = if s == 0.0 {
        0.0
    } else {
        let delta = max - min;
        let mut h = h_in;
        if r == max {
            h = (g - b) / delta;
        } else if g == max {
            h = 2.0 + (b - r) / delta;
        } else if b == max {
            h = 4.0 + (r - g) / delta;
        }
        h *= 60.0;
        if h < 0.0 {
            h += 360.0;
        }
        h
    };
    (h, s, v)
}

/// `rgb2hsv(int dstHsv[3], const TPixel32 &, int maxHsv)`.
pub fn rgb2hsv_int(r: u8, g: u8, b: u8, max_hsv: i32) -> [i32; 3] {
    let (h, s, v) = rgb_to_hsv(r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0, 0.0);
    let mx = max_hsv as f64;
    [
        tcrop_i(f2i((h / 360.0) * mx), 0, max_hsv),
        tcrop_i(f2i(s * mx), 0, max_hsv),
        tcrop_i(f2i(v * mx), 0, max_hsv),
    ]
}

/// `HLS2RGB`'s helper.
fn hls_value(n1: f64, n2: f64, mut h: f64) -> f64 {
    if h > 360.0 {
        h -= 360.0;
    } else if h < 0.0 {
        h += 360.0;
    }
    if h < 60.0 {
        n1 + (n2 - n1) * h / 60.0
    } else if h < 180.0 {
        n2
    } else if h < 240.0 {
        n1 + (n2 - n1) * (240.0 - h) / 60.0
    } else {
        n1
    }
}

/// `HLS2RGB(h, l, s)`.
pub fn hls2rgb(h: f64, l: f64, s: f64) -> (f64, f64, f64) {
    if s == 0.0 {
        return (l, l, l);
    }
    let m2 = if l < 0.5 { l * (1.0 + s) } else { l + s + l * s };
    let m1 = 2.0 * l - m2;
    (hls_value(m1, m2, h + 120.0), hls_value(m1, m2, h), hls_value(m1, m2, h - 120.0))
}

/// `rgb2hls(r, g, b)`; `h_in` is the previous `*h` (read back when no channel is the maximum).
pub fn rgb2hls(r: f64, g: f64, b: f64, h_in: f64) -> (f64, f64, f64) {
    let max = max3(r, g, b);
    let min = min3(r, g, b);
    let l = (max + min) / 2.0;
    if max == min {
        return (0.0, l, 0.0);
    }
    let s = if l <= 0.5 { (max - min) / (max + min) } else { (max - min) / (2.0 - max - min) };
    let delta = max - min;
    let mut h = h_in;
    if r == max {
        h = (g - b) / delta;
    } else if g == max {
        h = 2.0 + (b - r) / delta;
    } else if b == max {
        h = 4.0 + (r - g) / delta;
    }
    h *= 60.0;
    if h < 0.0 {
        h += 360.0;
    }
    (h, l, s)
}

unsafe fn guarded(f: impl FnOnce() -> i32) -> i32 {
    crate::guard(2, f)
}

/// `hsv2rgb(TPixel32 &, int[3], int)`: `rgb` (3 bytes, in/out) is assigned only in the six hue
/// sectors, as the C++ assigned the destination pixel.
///
/// # Safety
/// `rgb` must be NULL or valid for 3 bytes.
#[no_mangle]
pub unsafe extern "C" fn flare_hsv2rgb_pixel(h: i32, s: i32, v: i32, max_hsv: i32, rgb: *mut u8) -> i32 {
    if rgb.is_null() {
        return 1;
    }
    guarded(|| {
        let mut c = [*rgb, *rgb.add(1), *rgb.add(2)];
        hsv2rgb([h, s, v], max_hsv, &mut c);
        for (i, x) in c.iter().enumerate() {
            *rgb.add(i) = *x;
        }
        0
    })
}

/// `HSV2RGB(hue, sat, value, *red, *green, *blue)`: the outputs are assigned only where the
/// C++ assigned them.
///
/// # Safety
/// The three output pointers must be NULL or writable.
#[no_mangle]
pub unsafe extern "C" fn flare_hsv2rgb(hue: f64, sat: f64, value: f64, red: *mut f64, green: *mut f64, blue: *mut f64) -> i32 {
    if red.is_null() || green.is_null() || blue.is_null() {
        return 1;
    }
    guarded(|| {
        if let Some((r, g, b)) = hsv2rgb_d(hue, sat, value) {
            *red = r;
            *green = g;
            *blue = b;
        }
        0
    })
}

/// `RGB2HSV(r, g, b, *h, *s, *v)`.
///
/// # Safety
/// The three output pointers must be NULL or valid.
#[no_mangle]
pub unsafe extern "C" fn flare_rgb2hsv(r: f64, g: f64, b: f64, h: *mut f64, s: *mut f64, v: *mut f64) -> i32 {
    if h.is_null() || s.is_null() || v.is_null() {
        return 1;
    }
    guarded(|| {
        let (hh, ss, vv) = rgb_to_hsv(r, g, b, *h);
        *h = hh;
        *s = ss;
        *v = vv;
        0
    })
}

/// `rgb2hsv(int dstHsv[3], const TPixel32 &, int maxHsv)`.
///
/// # Safety
/// `dst` must be NULL or valid for 3 ints.
#[no_mangle]
pub unsafe extern "C" fn flare_rgb2hsv_pixel(r: u32, g: u32, b: u32, max_hsv: i32, dst: *mut i32) -> i32 {
    if dst.is_null() {
        return 1;
    }
    guarded(|| {
        let o = rgb2hsv_int(r as u8, g as u8, b as u8, max_hsv);
        for (i, x) in o.iter().enumerate() {
            *dst.add(i) = *x;
        }
        0
    })
}

/// `HLS2RGB(h, l, s, *r, *g, *b)`.
///
/// # Safety
/// The three output pointers must be NULL or writable.
#[no_mangle]
pub unsafe extern "C" fn flare_hls2rgb(h: f64, l: f64, s: f64, r: *mut f64, g: *mut f64, b: *mut f64) -> i32 {
    if r.is_null() || g.is_null() || b.is_null() {
        return 1;
    }
    guarded(|| {
        let (rr, gg, bb) = hls2rgb(h, l, s);
        *r = rr;
        *g = gg;
        *b = bb;
        0
    })
}

/// `rgb2hls(r, g, b, *h, *l, *s)`.
///
/// # Safety
/// The three output pointers must be NULL or valid.
#[no_mangle]
pub unsafe extern "C" fn flare_rgb2hls(r: f64, g: f64, b: f64, h: *mut f64, l: *mut f64, s: *mut f64) -> i32 {
    if h.is_null() || l.is_null() || s.is_null() {
        return 1;
    }
    guarded(|| {
        let (hh, ll, ss) = rgb2hls(r, g, b, *h);
        *h = hh;
        *l = ll;
        *s = ss;
        0
    })
}

// ---------------------------------------------------------------------------------------
// pixel type conversions

/// `tround(x)`: `(int)(x < 0 ? x - 0.5 : x + 0.5)` (the `(int)(-0.9) == 0` test is always true).
fn tround(x: f64) -> i32 {
    f2i(if x < 0.0 { x - 0.5 } else { x + 0.5 })
}

fn byte_crop(v: i32) -> u8 {
    (if (v as u32) <= 255 {
        v
    } else if v > 255 {
        255
    } else {
        0
    }) as u8
}

fn word_crop(v: i32) -> u16 {
    (if (v as u32) <= 65535 {
        v
    } else if v > 65535 {
        65535
    } else {
        0
    }) as u16
}

/// `byteFromUshort`.
pub fn byte_from_ushort(u: u16) -> u8 {
    ((256u32 * 255 + 1).wrapping_mul(u as u32).wrapping_add(1 << 23) >> 24) as u8
}

/// `ushortFromByte`.
pub fn ushort_from_byte(u: u8) -> u16 {
    u as u16 | (u as u16) << 8
}

unsafe fn put<T: Copy, const N: usize>(out: *mut T, v: [T; N]) -> i32 {
    if out.is_null() {
        return 1;
    }
    for (i, x) in v.iter().enumerate() {
        *out.add(i) = *x;
    }
    0
}

/// `toPixel32(const TPixel64 &)`.
///
/// # Safety
/// `out` must be NULL or valid for 4 bytes (r, g, b, m); likewise for every `flare_topixel*`.
#[no_mangle]
pub unsafe extern "C" fn flare_topixel32_from_pixel64(r: u16, g: u16, b: u16, m: u16, out: *mut u8) -> i32 {
    guarded(|| put(out, [byte_from_ushort(r), byte_from_ushort(g), byte_from_ushort(b), byte_from_ushort(m)]))
}

/// `toPixel32(const TPixelD &)`.
///
/// # Safety
/// See [`flare_topixel32_from_pixel64`].
#[no_mangle]
pub unsafe extern "C" fn flare_topixel32_from_pixeld(r: f64, g: f64, b: f64, m: f64, out: *mut u8) -> i32 {
    guarded(|| {
        let f = |x: f64| byte_crop(tround(x * 255.0));
        put(out, [f(r), f(g), f(b), f(m)])
    })
}

/// `toPixel32(const TPixelGR8 &)`: opaque grey.
///
/// # Safety
/// See [`flare_topixel32_from_pixel64`].
#[no_mangle]
pub unsafe extern "C" fn flare_topixel32_from_gr8(v: u8, out: *mut u8) -> i32 {
    guarded(|| put(out, [v, v, v, 255]))
}

/// `toPixel32(const TPixelF &)`: the factor is `double(255.0f)`, the product `float * double`.
///
/// # Safety
/// See [`flare_topixel32_from_pixel64`].
#[no_mangle]
pub unsafe extern "C" fn flare_topixel32_from_pixelf(r: f32, g: f32, b: f32, m: f32, out: *mut u8) -> i32 {
    guarded(|| {
        let f = |x: f32| byte_crop(tround(x as f64 * 255.0));
        put(out, [f(r), f(g), f(b), f(m)])
    })
}

/// `toPixel64(const TPixel32 &)`.
///
/// # Safety
/// `out` must be NULL or valid for 4 `u16`.
#[no_mangle]
pub unsafe extern "C" fn flare_topixel64_from_pixel32(r: u8, g: u8, b: u8, m: u8, out: *mut u16) -> i32 {
    guarded(|| put(out, [ushort_from_byte(r), ushort_from_byte(g), ushort_from_byte(b), ushort_from_byte(m)]))
}

/// `toPixel64(const TPixelD &)`.
///
/// # Safety
/// See [`flare_topixel64_from_pixel32`].
#[no_mangle]
pub unsafe extern "C" fn flare_topixel64_from_pixeld(r: f64, g: f64, b: f64, m: f64, out: *mut u16) -> i32 {
    guarded(|| {
        let f = |x: f64| word_crop(tround(x * 65535.0));
        put(out, [f(r), f(g), f(b), f(m)])
    })
}

/// `toPixel64(const TPixelGR8 &)`: opaque grey.
///
/// # Safety
/// See [`flare_topixel64_from_pixel32`].
#[no_mangle]
pub unsafe extern "C" fn flare_topixel64_from_gr8(v: u8, out: *mut u16) -> i32 {
    guarded(|| {
        let x = ushort_from_byte(v);
        put(out, [x, x, x, 65535])
    })
}

/// `toPixel64(const TPixelF &)`.
///
/// # Safety
/// See [`flare_topixel64_from_pixel32`].
#[no_mangle]
pub unsafe extern "C" fn flare_topixel64_from_pixelf(r: f32, g: f32, b: f32, m: f32, out: *mut u16) -> i32 {
    guarded(|| {
        let f = |x: f32| word_crop(tround(x as f64 * 65535.0));
        put(out, [f(r), f(g), f(b), f(m)])
    })
}

/// `toPixelD(const TPixel32 &)`: `(1.0 / 255.0) * channel`.
///
/// # Safety
/// `out` must be NULL or valid for 4 doubles.
#[no_mangle]
pub unsafe extern "C" fn flare_topixeld_from_pixel32(r: u8, g: u8, b: u8, m: u8, out: *mut f64) -> i32 {
    guarded(|| {
        let k = 1.0 / 255.0;
        put(out, [k * r as f64, k * g as f64, k * b as f64, k * m as f64])
    })
}

/// `toPixelD(const TPixel64 &)`: `(1.0 / 65535.0) * channel`.
///
/// # Safety
/// See [`flare_topixeld_from_pixel32`].
#[no_mangle]
pub unsafe extern "C" fn flare_topixeld_from_pixel64(r: u16, g: u16, b: u16, m: u16, out: *mut f64) -> i32 {
    guarded(|| {
        let k = 1.0 / 65535.0;
        put(out, [k * r as f64, k * g as f64, k * b as f64, k * m as f64])
    })
}

/// `toPixelD(const TPixelGR8 &)`: `value / 255.0`, matte 1.
///
/// # Safety
/// See [`flare_topixeld_from_pixel32`].
#[no_mangle]
pub unsafe extern "C" fn flare_topixeld_from_gr8(v: u8, out: *mut f64) -> i32 {
    guarded(|| {
        let x = v as f64 / 255.0;
        put(out, [x, x, x, 1.0])
    })
}

/// `toPixelD(const TPixelF &)`.
///
/// # Safety
/// See [`flare_topixeld_from_pixel32`].
#[no_mangle]
pub unsafe extern "C" fn flare_topixeld_from_pixelf(r: f32, g: f32, b: f32, m: f32, out: *mut f64) -> i32 {
    guarded(|| put(out, [r as f64, g as f64, b as f64, m as f64]))
}

/// `toPixelF(const TPixel32 &)`: `(1.f / 255.f) * channel` in single precision.
///
/// # Safety
/// `out` must be NULL or valid for 4 floats.
#[no_mangle]
pub unsafe extern "C" fn flare_topixelf_from_pixel32(r: u8, g: u8, b: u8, m: u8, out: *mut f32) -> i32 {
    guarded(|| {
        let k = 1.0f32 / 255.0f32;
        put(out, [k * r as f32, k * g as f32, k * b as f32, k * m as f32])
    })
}

/// `toPixelF(const TPixelD &)`.
///
/// # Safety
/// See [`flare_topixelf_from_pixel32`].
#[no_mangle]
pub unsafe extern "C" fn flare_topixelf_from_pixeld(r: f64, g: f64, b: f64, m: f64, out: *mut f32) -> i32 {
    guarded(|| put(out, [r as f32, g as f32, b as f32, m as f32]))
}

/// `toPixelF(const TPixel64 &)`: `(1.f / 65535.f) * channel` in single precision.
///
/// # Safety
/// See [`flare_topixelf_from_pixel32`].
#[no_mangle]
pub unsafe extern "C" fn flare_topixelf_from_pixel64(r: u16, g: u16, b: u16, m: u16, out: *mut f32) -> i32 {
    guarded(|| {
        let k = 1.0f32 / 65535.0f32;
        put(out, [k * r as f32, k * g as f32, k * b as f32, k * m as f32])
    })
}

/// `toPixelF(const TPixelGR8 &)`: `(float)value / 255.f`, matte 1.
///
/// # Safety
/// See [`flare_topixelf_from_pixel32`].
#[no_mangle]
pub unsafe extern "C" fn flare_topixelf_from_gr8(v: u8, out: *mut f32) -> i32 {
    guarded(|| {
        let x = v as f32 / 255.0f32;
        put(out, [x, x, x, 1.0])
    })
}

// ---------------------------------------------------------------------------------------
// toLinear

/// `toLin<T, Q>(val, gamma)` for the integer pixel types: `(Q)(max * pow(val / max, gamma) +
/// 0.5)`, `Q` being the low `bits` of the int the cast produced.
fn to_lin_int(val: u32, max: u32, gamma: f64) -> u32 {
    f2i(max as f64 * (val as f64 / max as f64).powf(gamma) + 0.5) as u32
}

/// `toLinear(const TPixel32 &, double)`.
///
/// # Safety
/// `out` must be NULL or valid for 4 bytes.
#[no_mangle]
pub unsafe extern "C" fn flare_tolinear32(r: u8, g: u8, b: u8, m: u8, gamma: f64, out: *mut u8) -> i32 {
    guarded(|| {
        let f = |v: u8| to_lin_int(v as u32, 255, gamma) as u8;
        put(out, [f(r), f(g), f(b), m])
    })
}

/// `toLinear(const TPixel64 &, double)`.
///
/// # Safety
/// `out` must be NULL or valid for 4 `u16`.
#[no_mangle]
pub unsafe extern "C" fn flare_tolinear64(r: u16, g: u16, b: u16, m: u16, gamma: f64, out: *mut u16) -> i32 {
    guarded(|| {
        let f = |v: u16| to_lin_int(v as u32, 65535, gamma) as u16;
        put(out, [f(r), f(g), f(b), m])
    })
}

/// `toLinear(const TPixelD &, double)`: `pow(channel, gamma)`, the matte unchanged.
///
/// # Safety
/// `out` must be NULL or valid for 4 doubles.
#[no_mangle]
pub unsafe extern "C" fn flare_tolinear_d(r: f64, g: f64, b: f64, m: f64, gamma: f64, out: *mut f64) -> i32 {
    guarded(|| put(out, [r.powf(gamma), g.powf(gamma), b.powf(gamma), m]))
}

/// `toLinear(const TPixelF &, double)`: negative channels are kept, the others get
/// `powf(channel, (float)gamma)`; the matte is unchanged.
///
/// # Safety
/// `out` must be NULL or valid for 4 floats.
#[no_mangle]
pub unsafe extern "C" fn flare_tolinear_f(r: f32, g: f32, b: f32, m: f32, gamma: f64, out: *mut f32) -> i32 {
    guarded(|| {
        let f = |v: f32| if v < 0.0 { v } else { v.powf(gamma as f32) };
        put(out, [f(r), f(g), f(b), m])
    })
}

/// `toLinear(const TPixelGR8 &, double)`.
///
/// # Safety
/// `out` must be NULL or valid for 1 byte.
#[no_mangle]
pub unsafe extern "C" fn flare_tolinear_gr8(v: u8, gamma: f64, out: *mut u8) -> i32 {
    guarded(|| put(out, [to_lin_int(v as u32, 255, gamma) as u8]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hsv_primaries_and_grey() {
        let mut c = [9u8; 3];
        hsv2rgb([0, 255, 255], 255, &mut c);
        assert_eq!(c, [255, 0, 0]);
        hsv2rgb([85, 255, 255], 255, &mut c); // 120 degrees
        assert_eq!(c, [0, 255, 0]); // 85 / 255 * 360 is exactly 120 degrees: sector 2, t = 1
        hsv2rgb([10, 0, 128], 255, &mut c);
        assert_eq!(c, [128, 128, 128]);
    }

    #[test]
    fn hsv_out_of_range_hue_leaves_the_destination() {
        // 3 * maxHsv -> hue 1080 -> wraps once to 720 -> sector 12: no assignment
        let mut c = [1u8, 2, 3];
        hsv2rgb([765, 255, 255], 255, &mut c);
        assert_eq!(c, [1, 2, 3]);
        // NaN hue (maxHsv 0 with a zero numerator): (int)NaN = INT_MIN -> no sector
        let mut c = [4u8, 5, 6];
        hsv2rgb([0, 255, 255], 0, &mut c);
        assert_eq!(c, [4, 5, 6]);
        assert_eq!(hsv2rgb_d(1000.0, 1.0, 1.0), None);
        assert_eq!(hsv2rgb_d(f64::NAN, 1.0, 1.0), None);
    }

    #[test]
    fn double_hsv_roundtrip_and_clamps() {
        let (r, g, b) = hsv2rgb_d(200.0, 0.5, 0.8).unwrap();
        let (h, s, v) = rgb_to_hsv(r, g, b, 0.0);
        assert!((h - 200.0).abs() < 1e-9 && (s - 0.5).abs() < 1e-12 && (v - 0.8).abs() < 1e-12);
        assert_eq!(hsv2rgb_d(-30.0, 2.0, 2.0), hsv2rgb_d(330.0, 1.0, 1.0));
        assert_eq!(hsv2rgb_d(360.0, 1.0, 1.0), Some((1.0, 0.0, 0.0)));
    }

    #[test]
    fn rgb2hsv_pixel_cases() {
        assert_eq!(rgb2hsv_int(255, 0, 0, 255), [0, 255, 255]);
        assert_eq!(rgb2hsv_int(0, 0, 0, 255), [0, 0, 0]);
        assert_eq!(rgb2hsv_int(255, 255, 255, 100), [0, 0, 100]);
        // a negative maxHsv collapses every channel to the "upper bound" of tcrop
        assert_eq!(rgb2hsv_int(10, 20, 30, -5), [-5, -5, -5]);
    }

    #[test]
    fn hls_roundtrip_and_quirks() {
        let (h, l, s) = rgb2hls(0.2, 0.4, 0.6, 7.0);
        let (r, g, b) = hls2rgb(h, l, s);
        assert!((r - 0.2).abs() < 1e-12 && (g - 0.4).abs() < 1e-12 && (b - 0.6).abs() < 1e-12);
        assert_eq!(rgb2hls(0.5, 0.5, 0.5, 7.0), (0.0, 0.5, 0.0));
        assert_eq!(hls2rgb(123.0, 0.3, 0.0), (0.3, 0.3, 0.3));
        // no channel equals the maximum (NaN): *h is read back and scaled
        let (h, l, s) = rgb2hls(f64::NAN, 0.0, 1.0, 2.0);
        assert_eq!(h, 120.0);
        assert!(l.is_nan() && s.is_nan());
        let (h, s, v) = rgb_to_hsv(f64::NAN, f64::NAN, f64::NAN, 2.0);
        assert_eq!(h, 120.0);
        assert!(s.is_nan() && v.is_nan());
    }

    #[test]
    fn pixel_conversions() {
        let mut o8 = [0u8; 4];
        let mut o16 = [0u16; 4];
        let mut od = [0f64; 4];
        let mut of = [0f32; 4];
        unsafe {
            assert_eq!(flare_topixel32_from_pixel64(0, 65535, 257, 32896, o8.as_mut_ptr()), 0);
            assert_eq!(o8, [0, 255, 1, 128]);
            assert_eq!(flare_topixel32_from_pixeld(-0.5, 0.5, 1.2, f64::NAN, o8.as_mut_ptr()), 0);
            assert_eq!(o8, [0, 128, 255, 0]);
            assert_eq!(flare_topixel32_from_gr8(7, o8.as_mut_ptr()), 0);
            assert_eq!(o8, [7, 7, 7, 255]);
            assert_eq!(flare_topixel32_from_pixelf(1.0, 0.0, 0.5, 2.0, o8.as_mut_ptr()), 0);
            assert_eq!(o8, [255, 0, 128, 255]);
            assert_eq!(flare_topixel64_from_pixel32(255, 0, 1, 128, o16.as_mut_ptr()), 0);
            assert_eq!(o16, [65535, 0, 257, 32896]);
            assert_eq!(flare_topixel64_from_pixeld(1.0, 0.5, -3.0, 0.25, o16.as_mut_ptr()), 0);
            assert_eq!(o16, [65535, 32768, 0, 16384]);
            assert_eq!(flare_topixel64_from_gr8(1, o16.as_mut_ptr()), 0);
            assert_eq!(o16, [257, 257, 257, 65535]);
            assert_eq!(flare_topixel64_from_pixelf(0.5, 1.0, 0.0, 9.0, o16.as_mut_ptr()), 0);
            assert_eq!(o16, [32768, 65535, 0, 65535]);
            assert_eq!(flare_topixeld_from_pixel32(255, 51, 0, 1, od.as_mut_ptr()), 0);
            assert_eq!(od, [1.0 / 255.0 * 255.0, 1.0 / 255.0 * 51.0, 0.0, 1.0 / 255.0]);
            assert_eq!(flare_topixeld_from_pixel64(65535, 0, 1, 2, od.as_mut_ptr()), 0);
            assert_eq!(od[0], 1.0 / 65535.0 * 65535.0);
            assert_eq!(flare_topixeld_from_gr8(255, od.as_mut_ptr()), 0);
            assert_eq!(od, [1.0, 1.0, 1.0, 1.0]);
            assert_eq!(flare_topixeld_from_pixelf(0.5, 0.25, 0.125, 1.0, od.as_mut_ptr()), 0);
            assert_eq!(od, [0.5, 0.25, 0.125, 1.0]);
            assert_eq!(flare_topixelf_from_pixel32(255, 0, 51, 255, of.as_mut_ptr()), 0);
            assert_eq!(of, [1.0f32 / 255.0f32 * 255.0f32, 0.0, 1.0f32 / 255.0f32 * 51.0f32, 1.0f32 / 255.0f32 * 255.0f32]);
            assert_eq!(flare_topixelf_from_pixeld(0.1, 0.2, 0.3, 0.4, of.as_mut_ptr()), 0);
            assert_eq!(of, [0.1f32, 0.2f32, 0.3f32, 0.4f32]);
            assert_eq!(flare_topixelf_from_pixel64(65535, 1, 0, 3, of.as_mut_ptr()), 0);
            assert_eq!(of[0], 1.0f32 / 65535.0f32 * 65535.0f32);
            assert_eq!(flare_topixelf_from_gr8(51, of.as_mut_ptr()), 0);
            assert_eq!(of, [51.0f32 / 255.0f32, 51.0f32 / 255.0f32, 51.0f32 / 255.0f32, 1.0]);
            // NULL outputs
            assert_eq!(flare_topixel32_from_gr8(1, std::ptr::null_mut()), 1);
            assert_eq!(flare_topixelf_from_gr8(1, std::ptr::null_mut()), 1);
        }
    }

    #[test]
    fn byte_and_ushort_helpers_match_tcommon() {
        for u in (0..=65535u32).step_by(7) {
            let want = ((256u32 * 255 + 1) * u + (1 << 23)) >> 24;
            assert_eq!(byte_from_ushort(u as u16) as u32, want);
        }
        assert_eq!(byte_from_ushort(65535), 255);
        for v in 0..=255u8 {
            assert_eq!(ushort_from_byte(v), v as u16 * 257);
        }
        assert_eq!(tround(-1.5), -2);
        assert_eq!(tround(1.5), 2);
        assert_eq!(tround(-0.4), 0);
        assert_eq!(byte_crop(i32::MIN), 0);
        assert_eq!(byte_crop(300), 255);
        assert_eq!(word_crop(-1), 0);
        assert_eq!(word_crop(70000), 65535);
    }

    #[test]
    fn to_linear() {
        let (mut o8, mut o16, mut od, mut of) = ([0u8; 4], [0u16; 4], [0f64; 4], [0f32; 4]);
        unsafe {
            assert_eq!(flare_tolinear32(0, 255, 128, 77, 1.0, o8.as_mut_ptr()), 0);
            assert_eq!(o8, [0, 255, 128, 77]); // gamma 1 is the identity, matte copied
            assert_eq!(flare_tolinear32(0, 255, 128, 77, 2.0, o8.as_mut_ptr()), 0);
            assert_eq!(o8, [0, 255, 64, 77]);
            assert_eq!(flare_tolinear64(0, 65535, 32768, 9, 1.0, o16.as_mut_ptr()), 0);
            assert_eq!(o16, [0, 65535, 32768, 9]);
            assert_eq!(flare_tolinear_d(0.25, 1.0, 0.0, 0.5, 2.0, od.as_mut_ptr()), 0);
            assert_eq!(od, [0.0625, 1.0, 0.0, 0.5]);
            assert_eq!(flare_tolinear_f(-1.0, 0.25, 1.0, 0.5, 2.0, of.as_mut_ptr()), 0);
            assert_eq!(of, [-1.0, 0.0625, 1.0, 0.5]);
            assert_eq!(flare_tolinear_gr8(255, 2.0, o8.as_mut_ptr()), 0);
            assert_eq!(o8[0], 255);
            assert_eq!(flare_tolinear_gr8(1, 2.0, std::ptr::null_mut()), 1);
        }
    }

    #[test]
    fn null_outputs_are_refused() {
        unsafe {
            assert_eq!(flare_hsv2rgb_pixel(0, 0, 0, 255, std::ptr::null_mut()), 1);
            assert_eq!(flare_rgb2hsv_pixel(0, 0, 0, 255, std::ptr::null_mut()), 1);
            let mut x = 0.0f64;
            assert_eq!(flare_hsv2rgb(0.0, 0.0, 0.0, &mut x, std::ptr::null_mut(), &mut x), 1);
            assert_eq!(flare_hls2rgb(0.0, 0.0, 0.0, &mut x, &mut x, std::ptr::null_mut()), 1);
            assert_eq!(flare_rgb2hls(0.0, 0.0, 0.0, std::ptr::null_mut(), &mut x, &mut x), 1);
            assert_eq!(flare_rgb2hsv(0.0, 0.0, 0.0, &mut x, std::ptr::null_mut(), &mut x), 1);
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

    fn d(b: u64) -> f64 {
        f64::from_bits(b)
    }
    fn f(b: u64) -> f32 {
        f32::from_bits(b as u32)
    }
    fn ulps(a: f64, b: f64) -> u64 {
        if a == b {
            return 0;
        }
        (a.to_bits() as i64 - b.to_bits() as i64).unsigned_abs()
    }

    // Replays the conversions recorded from the original C++ (tcolor_cases.txt). Everything but
    // `toLinear` is exact arithmetic and must match bit for bit everywhere; `toLinear` goes
    // through libm `pow`, hard-asserted only on linux-gnu (the platform of the reference), a
    // few ULP / one step of the integer result elsewhere.
    #[test]
    fn replay_recorded_cpp_cases() {
        let strict = cfg!(all(target_os = "linux", target_env = "gnu"));
        let mut counts = std::collections::BTreeMap::<String, usize>::new();
        for line in include_str!("tcolor_cases.txt").lines() {
            let (tag, g) = parse(line);
            let (i, o) = (&g[0], &g[g.len() - 1]);
            let mut o8 = [0u8; 4];
            let mut o16 = [0u16; 4];
            let mut od = [0f64; 4];
            let mut of = [0f32; 4];
            let w8: Vec<u64> = o.to_vec();
            unsafe {
                match tag {
                    "hsv" => {
                        let mut c = [7u8, 8, 9];
                        hsv2rgb([i[0] as i32, i[1] as i32, i[2] as i32], i[3] as i32, &mut c);
                        assert_eq!(c.map(|x| x as u64).to_vec(), w8[..3].to_vec(), "{line}");
                        assert_eq!(w8[3], 255); // the header sets the matte
                    }
                    "HSV2RGB" => {
                        let (mut a, mut b, mut c) = (123.5f64, 456.5, 789.5);
                        assert_eq!(flare_hsv2rgb(d(i[0]), d(i[1]), d(i[2]), &mut a, &mut b, &mut c), 0);
                        assert_eq!([a.to_bits(), b.to_bits(), c.to_bits()].to_vec(), w8, "{line}");
                    }
                    "RGB2HSV" => {
                        let (mut h, mut s, mut v) = (0f64, 0f64, 0f64);
                        assert_eq!(flare_rgb2hsv(d(i[0]), d(i[1]), d(i[2]), &mut h, &mut s, &mut v), 0);
                        assert_eq!([h.to_bits(), s.to_bits(), v.to_bits()].to_vec(), w8, "{line}");
                    }
                    "rgb2hls" => {
                        let (mut h, mut l, mut s) = (0f64, 0f64, 0f64);
                        assert_eq!(flare_rgb2hls(d(i[0]), d(i[1]), d(i[2]), &mut h, &mut l, &mut s), 0);
                        assert_eq!([h.to_bits(), l.to_bits(), s.to_bits()].to_vec(), w8, "{line}");
                    }
                    "HLS2RGB" => {
                        let (mut a, mut b, mut c) = (0f64, 0f64, 0f64);
                        assert_eq!(flare_hls2rgb(d(i[0]), d(i[1]), d(i[2]), &mut a, &mut b, &mut c), 0);
                        assert_eq!([a.to_bits(), b.to_bits(), c.to_bits()].to_vec(), w8, "{line}");
                    }
                    "rgb2hsv" => {
                        let mut dst = [0i32; 3];
                        assert_eq!(flare_rgb2hsv_pixel(i[0] as u32, i[1] as u32, i[2] as u32, i[3] as i32, dst.as_mut_ptr()), 0);
                        assert_eq!(dst.map(|x| x as i64 as u64).to_vec(), w8, "{line}");
                    }
                    "to32" => {
                        flare_topixel32_from_pixel64(i[0] as u16, i[1] as u16, i[2] as u16, i[3] as u16, o8.as_mut_ptr());
                        assert_eq!(o8.map(|x| x as u64).to_vec(), w8, "{line}");
                    }
                    "to32d" => {
                        flare_topixel32_from_pixeld(d(i[0]), d(i[1]), d(i[2]), d(i[3]), o8.as_mut_ptr());
                        assert_eq!(o8.map(|x| x as u64).to_vec(), w8, "{line}");
                    }
                    "to32g" => {
                        flare_topixel32_from_gr8(i[0] as u8, o8.as_mut_ptr());
                        assert_eq!(o8.map(|x| x as u64).to_vec(), w8, "{line}");
                    }
                    "to32f" => {
                        flare_topixel32_from_pixelf(f(i[0]), f(i[1]), f(i[2]), f(i[3]), o8.as_mut_ptr());
                        assert_eq!(o8.map(|x| x as u64).to_vec(), w8, "{line}");
                    }
                    "to64" => {
                        flare_topixel64_from_pixel32(i[0] as u8, i[1] as u8, i[2] as u8, i[3] as u8, o16.as_mut_ptr());
                        assert_eq!(o16.map(|x| x as u64).to_vec(), w8, "{line}");
                    }
                    "to64d" => {
                        flare_topixel64_from_pixeld(d(i[0]), d(i[1]), d(i[2]), d(i[3]), o16.as_mut_ptr());
                        assert_eq!(o16.map(|x| x as u64).to_vec(), w8, "{line}");
                    }
                    "to64g" => {
                        flare_topixel64_from_gr8(i[0] as u8, o16.as_mut_ptr());
                        assert_eq!(o16.map(|x| x as u64).to_vec(), w8, "{line}");
                    }
                    "to64f" => {
                        flare_topixel64_from_pixelf(f(i[0]), f(i[1]), f(i[2]), f(i[3]), o16.as_mut_ptr());
                        assert_eq!(o16.map(|x| x as u64).to_vec(), w8, "{line}");
                    }
                    "toD32" => {
                        flare_topixeld_from_pixel32(i[0] as u8, i[1] as u8, i[2] as u8, i[3] as u8, od.as_mut_ptr());
                        assert_eq!(od.map(f64::to_bits).to_vec(), w8, "{line}");
                    }
                    "toD64" => {
                        flare_topixeld_from_pixel64(i[0] as u16, i[1] as u16, i[2] as u16, i[3] as u16, od.as_mut_ptr());
                        assert_eq!(od.map(f64::to_bits).to_vec(), w8, "{line}");
                    }
                    "toDg" => {
                        flare_topixeld_from_gr8(i[0] as u8, od.as_mut_ptr());
                        assert_eq!(od.map(f64::to_bits).to_vec(), w8, "{line}");
                    }
                    "toDf" => {
                        flare_topixeld_from_pixelf(f(i[0]), f(i[1]), f(i[2]), f(i[3]), od.as_mut_ptr());
                        assert_eq!(od.map(f64::to_bits).to_vec(), w8, "{line}");
                    }
                    "toF32" => {
                        flare_topixelf_from_pixel32(i[0] as u8, i[1] as u8, i[2] as u8, i[3] as u8, of.as_mut_ptr());
                        assert_eq!(of.map(|x| x.to_bits() as u64).to_vec(), w8, "{line}");
                    }
                    "toFd" => {
                        flare_topixelf_from_pixeld(d(i[0]), d(i[1]), d(i[2]), d(i[3]), of.as_mut_ptr());
                        assert_eq!(of.map(|x| x.to_bits() as u64).to_vec(), w8, "{line}");
                    }
                    "toF64" => {
                        flare_topixelf_from_pixel64(i[0] as u16, i[1] as u16, i[2] as u16, i[3] as u16, of.as_mut_ptr());
                        assert_eq!(of.map(|x| x.to_bits() as u64).to_vec(), w8, "{line}");
                    }
                    "toFg" => {
                        flare_topixelf_from_gr8(i[0] as u8, of.as_mut_ptr());
                        assert_eq!(of.map(|x| x.to_bits() as u64).to_vec(), w8, "{line}");
                    }
                    "lin32" => {
                        flare_tolinear32(i[1] as u8, i[2] as u8, i[3] as u8, i[4] as u8, d(i[0]), o8.as_mut_ptr());
                        for k in 0..4 {
                            let diff = (o8[k] as i64 - w8[k] as i64).abs();
                            assert!(diff <= if strict { 0 } else { 1 }, "{line}");
                        }
                    }
                    "lin64" => {
                        flare_tolinear64(i[1] as u16, i[2] as u16, i[3] as u16, i[4] as u16, d(i[0]), o16.as_mut_ptr());
                        for k in 0..4 {
                            let diff = (o16[k] as i64 - w8[k] as i64).abs();
                            assert!(diff <= if strict { 0 } else { 1 }, "{line}");
                        }
                    }
                    "linD" => {
                        flare_tolinear_d(d(i[1]), d(i[2]), d(i[3]), d(i[4]), d(i[0]), od.as_mut_ptr());
                        for k in 0..4 {
                            assert!(ulps(od[k], d(w8[k])) <= if strict { 0 } else { 4 }, "{line}");
                        }
                    }
                    "linF" => {
                        flare_tolinear_f(f(i[1]), f(i[2]), f(i[3]), f(i[4]), d(i[0]), of.as_mut_ptr());
                        for k in 0..4 {
                            let (a, b) = (of[k].to_bits() as i64, w8[k] as i64);
                            assert!((a - b).abs() <= if strict { 0 } else { 4 }, "{line}");
                        }
                    }
                    "lingr" => {
                        flare_tolinear_gr8(i[1] as u8, d(i[0]), o8.as_mut_ptr());
                        assert!((o8[0] as i64 - w8[0] as i64).abs() <= if strict { 0 } else { 1 }, "{line}");
                    }
                    _ => continue,
                }
            }
            *counts.entry(tag.to_string()).or_default() += 1;
        }
        for t in ["hsv", "HSV2RGB", "RGB2HSV", "rgb2hls", "HLS2RGB", "rgb2hsv"] {
            assert_eq!(counts[t], 600, "{t}");
        }
        for t in ["to32", "to32d", "to32g", "to32f", "to64", "to64d", "to64g", "to64f", "toD32", "toD64", "toDg", "toDf", "toF32", "toFd", "toF64", "toFg"] {
            assert_eq!(counts[t], 400, "{t}");
        }
        for t in ["lin32", "lin64", "linD", "linF", "lingr"] {
            assert_eq!(counts[t], 300, "{t}");
        }
    }
}
