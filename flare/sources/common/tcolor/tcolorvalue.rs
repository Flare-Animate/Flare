//! Port of common/tcolor/tcolorvalue.cpp: RGB <-> HSV/HLS conversions of the
//! double-precision colour holder `TColorValue` (members m_r, m_g, m_b, m_m).
//! `ColorValue` mirrors the C++ object layout (4 doubles) so tcolorvalue.h can
//! hand `this` straight to the C ABI below.

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColorValue {
    pub m_r: f64,
    pub m_g: f64,
    pub m_b: f64,
    pub m_m: f64,
}

fn tcrop(v: f64, lo: f64, hi: f64) -> f64 {
    if v < lo {
        lo
    } else if v > hi {
        hi
    } else {
        v
    }
}

fn max3(a: f64, b: f64, c: f64) -> f64 {
    // std::max({a, b, c}): first maximal element wins
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

impl ColorValue {
    /// TColorValue(): m_r = m_g = m_b = 0, m_m = 1
    pub fn new() -> ColorValue {
        ColorValue { m_r: 0.0, m_g: 0.0, m_b: 0.0, m_m: 1.0 }
    }

    pub fn get_hsv(&self) -> (i32, i32, i32) {
        let r = self.m_r;
        let g = self.m_g;
        let b = self.m_b;
        debug_assert!((0.0..=1.0).contains(&r));
        debug_assert!((0.0..=1.0).contains(&g));
        debug_assert!((0.0..=1.0).contains(&b));

        let max = max3(r, g, b);
        let min = min3(r, g, b);

        let v = max;

        let s = if max != 0.0 { (max - min) / max } else { 0.0 };

        let mut h = 0.0;
        if s == 0.0 {
            h = 0.0;
        } else {
            let delta = max - min;

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
        }
        debug_assert!((0.0..=360.0).contains(&h));
        debug_assert!((0.0..=1.0).contains(&s));
        debug_assert!((0.0..=1.0).contains(&v));
        (h as i32, (s * 100.0) as i32, (v * 100.0) as i32)
    }

    /// returns (h, l, s)
    pub fn get_hls(&self) -> (f64, f64, f64) {
        let max = max3(self.m_r, self.m_g, self.m_b);
        let min = min3(self.m_r, self.m_g, self.m_b);

        let l = (max + min) / 2.0;
        let (mut h, s);

        if max == min {
            s = 0.0;
            h = 0.0;
        } else {
            if l <= 0.5 {
                s = (max - min) / (max + min);
            } else {
                s = (max - min) / (2.0 - max - min);
            }

            let delta = max - min;
            h = 0.0;
            if self.m_r == max {
                h = (self.m_g - self.m_b) / delta;
            } else if self.m_g == max {
                h = 2.0 + (self.m_b - self.m_r) / delta;
            } else if self.m_b == max {
                h = 4.0 + (self.m_r - self.m_g) / delta;
            }

            h *= 60.0;
            if h < 0.0 {
                h += 360.0;
            }
        }
        (h, l, s)
    }

    pub fn set_hsv(&mut self, h: i32, s: i32, v: i32) {
        let mut hue = h as f64;
        let sat = 0.01 * s as f64;
        let value = 0.01 * v as f64;

        debug_assert!(0.0 <= hue && hue <= 360.0);
        debug_assert!(0.0 <= sat && sat <= 1.0);
        debug_assert!(0.0 <= value && value <= 1.0);

        if sat == 0.0 {
            self.m_r = value;
            self.m_g = value;
            self.m_b = value;
        } else {
            if hue == 360.0 {
                hue = 0.0;
            }

            hue /= 60.0;
            let i = hue as i32;
            let f = hue - i as f64;
            let p = tcrop(value * (1.0 - sat), 0.0, 1.0);
            let q = tcrop(value * (1.0 - (sat * f)), 0.0, 1.0);
            let t = tcrop(value * (1.0 - (sat * (1.0 - f))), 0.0, 1.0);

            // (no default case in the original: other sectors leave the colour untouched)
            match i {
                0 => {
                    self.m_r = value;
                    self.m_g = t;
                    self.m_b = p;
                }
                1 => {
                    self.m_r = q;
                    self.m_g = value;
                    self.m_b = p;
                }
                2 => {
                    self.m_r = p;
                    self.m_g = value;
                    self.m_b = t;
                }
                3 => {
                    self.m_r = p;
                    self.m_g = q;
                    self.m_b = value;
                }
                4 => {
                    self.m_r = t;
                    self.m_g = p;
                    self.m_b = value;
                }
                5 => {
                    self.m_r = value;
                    self.m_g = p;
                    self.m_b = q;
                }
                _ => {}
            }
        }
    }

    pub fn get_rgb(&self) -> (i32, i32, i32) {
        (
            (self.m_r * 255.0 + 0.5) as i32,
            (self.m_g * 255.0 + 0.5) as i32,
            (self.m_b * 255.0 + 0.5) as i32,
        )
    }

    /// alpha channel of getPixel(): (int)(m_m*255.0+0.5)
    pub fn get_pixel_m(&self) -> i32 {
        (self.m_m * 255.0 + 0.5) as i32
    }

    pub fn set_rgb(&mut self, r: i32, g: i32, b: i32) {
        self.m_r = r as f64 / 255.0;
        self.m_g = g as f64 / 255.0;
        self.m_b = b as f64 / 255.0;
    }

    /// setPixel(TPixel32): setRgb(src.r, src.g, src.b); m_m = src.m / 255.0
    pub fn set_pixel(&mut self, r: u8, g: u8, b: u8, m: u8) {
        self.set_rgb(r as i32, g as i32, b as i32);
        self.m_m = m as f64 / 255.0;
    }
}

impl Default for ColorValue {
    fn default() -> Self {
        ColorValue::new()
    }
}

//=============================================================================
// C ABI (tcolorvalue.h forwards `this` here)

/// # Safety
/// All pointers valid; `cv` points to a TColorValue (4 doubles).
#[no_mangle]
pub unsafe extern "C" fn flare_colorvalue_get_hsv(cv: *const ColorValue, h: *mut i32, s: *mut i32, v: *mut i32) {
    let (a, b, c) = (*cv).get_hsv();
    *h = a;
    *s = b;
    *v = c;
}

/// # Safety
/// All pointers valid.
#[no_mangle]
pub unsafe extern "C" fn flare_colorvalue_get_hls(cv: *const ColorValue, h: *mut f64, l: *mut f64, s: *mut f64) {
    let (a, b, c) = (*cv).get_hls();
    *h = a;
    *l = b;
    *s = c;
}

/// # Safety
/// `cv` valid.
#[no_mangle]
pub unsafe extern "C" fn flare_colorvalue_set_hsv(cv: *mut ColorValue, h: i32, s: i32, v: i32) {
    (*cv).set_hsv(h, s, v);
}

/// # Safety
/// All pointers valid.
#[no_mangle]
pub unsafe extern "C" fn flare_colorvalue_get_rgb(cv: *const ColorValue, r: *mut i32, g: *mut i32, b: *mut i32) {
    let (x, y, z) = (*cv).get_rgb();
    *r = x;
    *g = y;
    *b = z;
}

/// # Safety
/// `cv` valid.
#[no_mangle]
pub unsafe extern "C" fn flare_colorvalue_get_pixel_m(cv: *const ColorValue) -> i32 {
    (*cv).get_pixel_m()
}

/// # Safety
/// `cv` valid.
#[no_mangle]
pub unsafe extern "C" fn flare_colorvalue_set_rgb(cv: *mut ColorValue, r: i32, g: i32, b: i32) {
    (*cv).set_rgb(r, g, b);
}

/// # Safety
/// `cv` valid.
#[no_mangle]
pub unsafe extern "C" fn flare_colorvalue_set_pixel(cv: *mut ColorValue, r: u8, g: u8, b: u8, m: u8) {
    (*cv).set_pixel(r, g, b, m);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-12
    }

    #[test]
    fn default_is_opaque_black() {
        let c = ColorValue::new();
        assert_eq!((c.m_r, c.m_g, c.m_b, c.m_m), (0.0, 0.0, 0.0, 1.0));
        assert_eq!(c.get_rgb(), (0, 0, 0));
        assert_eq!(c.get_pixel_m(), 255);
        assert_eq!(c.get_hsv(), (0, 0, 0));
    }

    #[test]
    fn primaries_hsv() {
        let mut c = ColorValue::new();
        c.set_rgb(255, 0, 0);
        assert_eq!(c.get_hsv(), (0, 100, 100));
        c.set_rgb(0, 255, 0);
        assert_eq!(c.get_hsv(), (120, 100, 100));
        c.set_rgb(0, 0, 255);
        assert_eq!(c.get_hsv(), (240, 100, 100));
        // magenta: r==max, (g-b)/delta = -1 -> -60 -> +360
        c.set_rgb(255, 0, 255);
        assert_eq!(c.get_hsv(), (300, 100, 100));
        // grey: saturation 0 -> hue 0; v = 128/255 -> (int)(50.19) = 50
        c.set_rgb(128, 128, 128);
        assert_eq!(c.get_hsv(), (0, 0, 50));
    }

    #[test]
    fn set_hsv_sectors() {
        let mut c = ColorValue::new();
        c.set_hsv(0, 100, 100);
        assert_eq!(c.get_rgb(), (255, 0, 0));
        c.set_hsv(60, 100, 100); // sector 1, f = 0: q = value*(1-0)=1 ... r = q = 1, g = value, b = p = 0
        assert_eq!(c.get_rgb(), (255, 255, 0));
        c.set_hsv(120, 100, 100);
        assert_eq!(c.get_rgb(), (0, 255, 0));
        c.set_hsv(180, 100, 100);
        assert_eq!(c.get_rgb(), (0, 255, 255));
        c.set_hsv(240, 100, 100);
        assert_eq!(c.get_rgb(), (0, 0, 255));
        c.set_hsv(300, 100, 100);
        assert_eq!(c.get_rgb(), (255, 0, 255));
        // hue 360 wraps to 0
        c.set_hsv(360, 100, 100);
        assert_eq!(c.get_rgb(), (255, 0, 0));
        // sat == 0 -> grey of the given value
        c.set_hsv(200, 0, 50);
        assert!(near(c.m_r, 0.5) && near(c.m_g, 0.5) && near(c.m_b, 0.5));
    }

    #[test]
    fn hls_values() {
        let mut c = ColorValue::new();
        c.m_r = 1.0;
        assert_eq!(c.get_hls(), (0.0, 0.5, 1.0)); // pure red: l = .5, s = (1-0)/(1+0)
        c.set_rgb(0, 255, 0);
        assert_eq!(c.get_hls(), (120.0, 0.5, 1.0));
        // light colour (l > 0.5): s = (max-min)/(2-max-min)
        c.m_r = 1.0;
        c.m_g = 0.5;
        c.m_b = 0.5;
        let (h, l, s) = c.get_hls();
        assert!(near(h, 0.0) && near(l, 0.75) && near(s, 0.5 / 0.5));
        // grey
        c.m_r = 0.3;
        c.m_g = 0.3;
        c.m_b = 0.3;
        assert_eq!(c.get_hls(), (0.0, 0.3, 0.0));
        // blue is max: h = 4 + (r-g)/delta = 4 -> 240
        c.m_r = 0.0;
        c.m_g = 0.0;
        c.m_b = 1.0;
        assert_eq!(c.get_hls().0, 240.0);
    }

    #[test]
    fn pixel_roundtrip_and_rounding() {
        let mut c = ColorValue::new();
        c.set_pixel(10, 20, 30, 128);
        assert_eq!(c.get_rgb(), (10, 20, 30));
        assert_eq!(c.get_pixel_m(), 128);
        assert!(near(c.m_m, 128.0 / 255.0));
        // rounding is +0.5 then truncation
        c.m_r = 0.5; // 127.5 + .5 = 128
        assert_eq!(c.get_rgb().0, 128);
    }

    #[test]
    fn ffi_roundtrip() {
        let mut c = ColorValue::new();
        unsafe {
            flare_colorvalue_set_hsv(&mut c, 240, 100, 100);
            let (mut h, mut s, mut v) = (0, 0, 0);
            flare_colorvalue_get_hsv(&c, &mut h, &mut s, &mut v);
            assert_eq!((h, s, v), (240, 100, 100));
            flare_colorvalue_set_pixel(&mut c, 1, 2, 3, 4);
            assert_eq!(flare_colorvalue_get_pixel_m(&c), 4);
        }
    }
}
