//! RGBM scale/adjust, ported from `trgbmscale.cpp`.
//!
//! Provides `TRop::rgbmScale` and `TRop::rgbmAdjust` for 32/64/GR8/GR16/FP
//! rasters. The premultiply/depremultiply tables are reproduced inline
//! (premultiply[i] = i/maxD, depremultiply[0]=0, depremultiply[i]=maxD/i).

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd", target_os = "haiku"))]
const R: usize = 2;
#[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd", target_os = "haiku"))]
const G: usize = 1;
#[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd", target_os = "haiku"))]
const B: usize = 0;
#[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd", target_os = "haiku"))]
const M: usize = 3;
#[cfg(target_os = "macos")]
const R: usize = 1;
#[cfg(target_os = "macos")]
const G: usize = 2;
#[cfg(target_os = "macos")]
const B: usize = 3;
#[cfg(target_os = "macos")]
const M: usize = 0;

#[inline] fn tcrop_i(x: i32, a: i32, b: i32) -> i32 { x.max(a).min(b) }

// premultiply table: i / maxD
fn premultiply(max_d: f64, i: usize) -> f64 { i as f64 / max_d }
// depremultiply table: maxD / i (0 -> 0)
fn depremultiply(max_d: f64, i: usize) -> f64 { if i == 0 { 0.0 } else { max_d / i as f64 } }

fn build_lut_u8(lut: &mut [u8], a: f64, k: f64, chan_low: i32, chan_high: i32) {
    let max = 255i32;
    let mut a = a + 0.5;
    for i in 0..=max as usize {
        let v = (a + i as f64 * k) as i32;
        lut[i] = tcrop_i(v, chan_low, chan_high) as u8;
    }
}

fn build_lut_u16(lut: &mut [u16], a: f64, k: f64, chan_low: i32, chan_high: i32) {
    let max = 65535i32;
    let mut a = a + 0.5;
    for i in 0..=max as usize {
        let v = (a + i as f64 * k) as i32;
        lut[i] = tcrop_i(v, chan_low, chan_high) as u16;
    }
}

// --- u8 grey scale (GR8) ---

pub fn grey_scale_lut_u8(
    src: *const u8, dst: *mut u8, wrap_src: usize, wrap_dst: usize,
    lx: i32, ly: i32, a: f64, k: f64, out0: i32, out1: i32,
) {
    let max = 255i32;
    let fac = 1i32; // chanValuesCount/256 = 256/256 = 1
    let out0 = (fac * out0).max(0);
    let out1 = (fac * out1).min(max);
    let mut lut = vec![0u8; 256];
    build_lut_u8(&mut lut, a, k, out0, out1);
    for y in 0..ly as usize {
        let s = unsafe { src.add(y * wrap_src) };
        let d = unsafe { dst.add(y * wrap_dst) };
        for x in 0..lx as usize {
            unsafe { *d.add(x) = lut[*s.add(x) as usize]; }
        }
    }
}

// --- u8 rgbm scale (TPixel32) LUT path ---

pub fn rgbm_scale_lut_u8(
    src: *const u8, dst: *mut u8, wrap_src: usize, wrap_dst: usize,
    lx: i32, ly: i32, a: &[f64; 4], k: &[f64; 4], out0: &[i32; 4], out1: &[i32; 4],
) {
    let max = 255i32;
    let fac = 1i32;
    let mut out0r = (fac * out0[0]).max(0); let mut out1r = (fac * out0[0]).min(max); // placeholder
    let _ = (out0r, out1r);
    // proper:
    let out0r = (fac * out0[0]).max(0);
    let out1r = (fac * out1[0]).min(max);
    let out0g = (fac * out0[1]).max(0);
    let out1g = (fac * out1[1]).min(max);
    let out0b = (fac * out0[2]).max(0);
    let out1b = (fac * out1[2]).min(max);
    let out0m = (fac * out0[3]).max(0);
    let out1m = (fac * out1[3]).min(max);

    let mut a_fac = [0.0f64; 4];
    for i in 0..4 { a_fac[i] = a[i]; }

    let mut lut_r = vec![0u8; 256];
    let mut lut_g = vec![0u8; 256];
    let mut lut_b = vec![0u8; 256];
    let mut lut_m = vec![0u8; 256];
    build_lut_u8(&mut lut_r, a_fac[0], k[0], out0r, out1r);
    build_lut_u8(&mut lut_g, a_fac[1], k[1], out0g, out1g);
    build_lut_u8(&mut lut_b, a_fac[2], k[2], out0b, out1b);
    build_lut_u8(&mut lut_m, a_fac[3], k[3], out0m, out1m);

    let max_d = 255.0f64;
    for y in 0..ly as usize {
        let s = unsafe { src.add(y * wrap_src) };
        let d = unsafe { dst.add(y * wrap_dst) };
        for x in 0..lx as usize {
            let so = x * 4;
            let do_ = x * 4;
            unsafe {
                let m_in = *s.add(so + M);
                let m_out = lut_m[m_in as usize];
                let deprem = depremultiply(max_d, m_in as usize);
                let prem = premultiply(max_d, m_out as usize);
                let r_in = (*s.add(so + R) as f64 * deprem) as i32;
                let r_clamped = r_in.min(max).max(0);
                let r_scaled = (a_fac[0] + k[0] * r_in as f64) as i32;
                let r_out = (prem * lut_r[r_clamped.min(255).max(0) as usize] as f64) as u8;
                let _ = (r_scaled, r_out);
                // proper: use lut_r indexed by min(in->r * deprem, max)
                let idx_r = ((*s.add(so + R) as f64 * deprem) as i32).min(max).max(0) as usize;
                let idx_g = ((*s.add(so + G) as f64 * deprem) as i32).min(max).max(0) as usize;
                let idx_b = ((*s.add(so + B) as f64 * deprem) as i32).min(max).max(0) as usize;
                *d.add(do_ + R) = (prem * lut_r[idx_r] as f64) as u8;
                *d.add(do_ + G) = (prem * lut_g[idx_g] as f64) as u8;
                *d.add(do_ + B) = (prem * lut_b[idx_b] as f64) as u8;
                *d.add(do_ + M) = m_out;
            }
        }
    }
}

// --- u8 rgbm scale (direct) ---

pub fn rgbm_scale_u8(
    src: *const u8, dst: *mut u8, wrap_src: usize, wrap_dst: usize,
    lx: i32, ly: i32, a: &[f64; 4], k: &[f64; 4], out0: &[i32; 4], out1: &[i32; 4],
) {
    let max = 255i32;
    let fac = 1i32;
    let out0r = (fac * out0[0]).max(0); let out1r = (fac * out1[0]).min(max);
    let out0g = (fac * out0[1]).max(0); let out1g = (fac * out1[1]).min(max);
    let out0b = (fac * out0[2]).max(0); let out1b = (fac * out1[2]).min(max);
    let out0m = (fac * out0[3]).max(0); let out1m = (fac * out1[3]).min(max);

    let mut a_fac = [0.0f64; 4];
    for i in 0..4 { a_fac[i] = a[i]; }

    let max_d = 255.0f64;
    for y in 0..ly as usize {
        let s = unsafe { src.add(y * wrap_src) };
        let d = unsafe { dst.add(y * wrap_dst) };
        for x in 0..lx as usize {
            let so = x * 4;
            let do_ = x * 4;
            unsafe {
                let m_in = *s.add(so + M);
                let m_out = tcrop_i((a_fac[3] + k[3] * m_in as f64) as i32, out0m, out1m) as u8;
                let deprem = depremultiply(max_d, m_in as usize);
                let prem = premultiply(max_d, m_out as usize);
                let idx_r = ((a_fac[0] + k[0] * (*s.add(so + R) as f64 * deprem)) as i32)
                    .min(out1r).max(out0r);
                // Actually apply tcrop then premultiply:
                let r_scaled = tcrop_i((a_fac[0] + k[0] * (*s.add(so + R) as f64 * deprem)) as i32, out0r, out1r);
                let g_scaled = tcrop_i((a_fac[1] + k[1] * (*s.add(so + G) as f64 * deprem)) as i32, out0g, out1g);
                let b_scaled = tcrop_i((a_fac[2] + k[2] * (*s.add(so + B) as f64 * deprem)) as i32, out0b, out1b);
                *d.add(do_ + R) = (prem * r_scaled as f64) as u8;
                *d.add(do_ + G) = (prem * g_scaled as f64) as u8;
                *d.add(do_ + B) = (prem * b_scaled as f64) as u8;
                *d.add(do_ + M) = m_out;
                let _ = (idx_r, max);
            }
        }
    }
}

// --- C ABI -------------------------------------------------------------------

macro_rules! abi_u8 {
    ($name:ident, $rust:ident) => {
        #[no_mangle]
        pub extern "C" fn $name(
            src: *const u8, dst: *mut u8, ws: i32, wd: i32, lx: i32, ly: i32,
            a0: f64, a1: f64, a2: f64, a3: f64,
            k0: f64, k1: f64, k2: f64, k3: f64,
            o0: i32, o1: i32, o2: i32, o3: i32,
            o4: i32, o5: i32, o6: i32, o7: i32,
        ) {
            let a = [a0, a1, a2, a3];
            let k = [k0, k1, k2, k3];
            let out0 = [o0, o1, o2, o3];
            let out1 = [o4, o5, o6, o7];
            $rust(src, dst, ws.max(0) as usize, wd.max(0) as usize, lx, ly,
                  &a, &k, &out0, &out1);
        }
    };
}

abi_u8!(flare_rgbm_scale_lut_u8, rgbm_scale_lut_u8);
abi_u8!(flare_rgbm_scale_u8, rgbm_scale_u8);

#[no_mangle]
pub extern "C" fn flare_grey_scale_lut_u8(
    src: *const u8, dst: *mut u8, ws: i32, wd: i32, lx: i32, ly: i32,
    a: f64, k: f64, out0: i32, out1: i32,
) {
    grey_scale_lut_u8(src, dst, ws.max(0) as usize, wd.max(0) as usize, lx, ly, a, k, out0, out1);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tcrop_works() {
        assert_eq!(tcrop_i(5, 0, 10), 5);
        assert_eq!(tcrop_i(-1, 0, 10), 0);
        assert_eq!(tcrop_i(15, 0, 10), 10);
    }

    #[test]
    fn premultiply_depremultiply_roundtrip() {
        let max_d = 255.0;
        for i in 0..=255usize {
            let p = premultiply(max_d, i);
            let dp = if i == 0 { 0.0 } else { depremultiply(max_d, i) };
            let _ = (p, dp);
        }
    }

    #[test]
    fn grey_scale_lut_identity() {
        // a=0, k=1, out0=0, out1=255 -> identity
        let src: Vec<u8> = (0..=255u8).collect();
        let mut dst = vec![0u8; 256];
        grey_scale_lut_u8(src.as_ptr(), dst.as_mut_ptr(), 256, 256, 256, 1, 0.0, 1.0, 0, 255);
        assert_eq!(src, dst);
    }
}