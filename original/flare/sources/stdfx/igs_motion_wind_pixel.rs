//! Port of stdfx/igs_motion_wind_pixel.cpp: stateful per-pixel motion wind effect.
//!
//! Applies directional wind streaks to pixels based on lightness peaks. When a pixel's
//! lightness drops below a "key" (peak) value, a wind trail is generated using the
//! table from `igs_motion_wind_table`.

use std::os::raw::c_ulong;

/// Lightweight wrapper around igs_math_random
pub struct IgsMathRandom {
    seed: c_ulong,
}

impl IgsMathRandom {
    pub fn new(seed: u64) -> Self {
        Self { seed: seed as c_ulong }
    }

    pub fn next_d(&mut self) -> f64 {
        crate::igs_math_random::next_d(&mut self.seed)
    }
}
use crate::igs_motion_wind_table;

const RGBA_CHANNELS: i32 = 4;
const RGB_CHANNELS: i32 = 3;

// Channel order is Toonz6, not RGB: `enum num { blu = 0, gre, red, alp, siz }`
// (igs_ifx_common.h). Only `get_lightness` mixes channels, so this is the one place
// where the order matters; `invert_pixel` and `blow_wind` pair each channel with
// itself and are order-independent.
const BLU: usize = 0;
const GRE: usize = 1;
const RED: usize = 2;

/// Motion wind pixel processor (stateful, one per scanline or processing unit).
pub struct MotionWindPixel {
    // Config
    blow_dark_sw: bool,
    blow_alpha_sw: bool,
    length_min: f64,
    length_max: f64,
    length_bias: f64,
    length_ref_sw: bool,
    force_min: f64,
    force_max: f64,
    force_bias: f64,
    force_ref_sw: bool,
    density_min: f64,
    density_max: f64,
    density_bias: f64,
    density_ref_sw: bool,

    // State
    pixel_key: [f64; 4], // max 4 channels (RGBA)
    key_lightness: f64,
    table: Vec<f64>,
    table_len: usize,
    table_pos: usize,

    // RNGs
    length_random: IgsMathRandom,
    force_random: IgsMathRandom,
    density_random: IgsMathRandom,
}

impl MotionWindPixel {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        blow_dark_sw: bool,
        blow_alpha_sw: bool,
        length_random_seed: u64,
        length_min: f64,
        length_max: f64,
        length_bias: f64,
        length_ref_sw: bool,
        force_random_seed: u64,
        force_min: f64,
        force_max: f64,
        force_bias: f64,
        force_ref_sw: bool,
        density_random_seed: u64,
        density_min: f64,
        density_max: f64,
        density_bias: f64,
        density_ref_sw: bool,
    ) -> Self {
        let table_size =
            igs_motion_wind_table::table_size(length_min, length_max).max(0) as usize;
        Self {
            blow_dark_sw,
            blow_alpha_sw,
            length_min,
            length_max,
            length_bias,
            length_ref_sw,
            force_min,
            force_max,
            force_bias,
            force_ref_sw,
            density_min,
            density_max,
            density_bias,
            density_ref_sw,
            pixel_key: [0.0; 4],
            key_lightness: 0.0,
            table: vec![0.0; table_size],
            table_len: 0,
            table_pos: 0,
            length_random: IgsMathRandom::new(length_random_seed),
            force_random: IgsMathRandom::new(force_random_seed),
            density_random: IgsMathRandom::new(density_random_seed),
        }
    }

    /// Process one pixel: returns `true` if inside wind trail, `false` outside.
    pub fn change(
        &mut self,
        key_reset_sw: bool,
        ref_val: f64,
        channels: i32,
        pixel_tgt: &mut [f64],
    ) -> bool {
        // Invert if processing dark areas
        if self.blow_dark_sw {
            invert_pixel(channels, pixel_tgt);
        }

        let crnt_lightness = get_lightness(channels, pixel_tgt);

        // Reset key at scanline start
        if key_reset_sw {
            self.key_lightness = crnt_lightness;
            for (i, val) in pixel_tgt.iter().enumerate().take(channels as usize) {
                self.pixel_key[i] = *val;
            }
        }

        // Below key → inside potential wind zone
        if crnt_lightness < self.key_lightness {
            // Generate new wind table if not already in one
            if self.table_len == 0 {
                let mut length_min = self.length_min;
                let mut length_max = self.length_max;
                let mut force_min = self.force_min;
                let mut force_max = self.force_max;
                let mut density_min = self.density_min;
                let mut density_max = self.density_max;

                let ref_value = if ref_val >= 0.0 {
                    ref_val
                } else {
                    get_lightness(channels, pixel_tgt)
                };

                if self.length_ref_sw {
                    length_min *= ref_value;
                    length_max *= ref_value;
                }
                if self.force_ref_sw {
                    force_min *= ref_value;
                    force_max *= ref_value;
                }
                if self.density_ref_sw {
                    density_min *= ref_value;
                    density_max *= ref_value;
                }

                // Generate table (same coefficients as the C++ original; the
                // shared helper lives in igs_motion_wind_table.rs).
                let len = igs_motion_wind_table::make_table_inplace(
                    &mut self.table,
                    length_min,
                    length_max,
                    self.length_bias,
                    force_min,
                    force_max,
                    self.force_bias,
                    density_min,
                    density_max,
                    self.density_bias,
                    || self.length_random.next_d(),
                    || self.force_random.next_d(),
                    || self.density_random.next_d(),
                );
                self.table_len = if len > 0 { len as usize } else { 0 };
                self.table_pos = 1;
            }

            // Apply wind if still in range
            if self.table_pos < self.table_len {
                blow_wind(
                    channels,
                    &self.pixel_key,
                    &self.table,
                    self.table_pos,
                    self.blow_alpha_sw,
                    pixel_tgt,
                );
                self.table_pos += 1;

                if self.blow_dark_sw {
                    invert_pixel(channels, pixel_tgt);
                }
                return true; // inside wind
            }
        }

        // Outside wind: update key
        self.key_lightness = crnt_lightness;
        for (i, val) in pixel_tgt.iter().enumerate().take(channels as usize) {
            self.pixel_key[i] = *val;
        }
        self.table_len = 0; // reset table

        false // outside wind
    }

    /// Release the current wind table (C++ `pixel::clear`).
    pub fn clear(&mut self) {
        self.table_len = 0;
        self.table_pos = 0;
    }
}

// (The former C ABI that backed the C++ class `igs::motion_wind::pixel` is gone:
// `igs_motion_wind.rs`, the only caller, now drives `MotionWindPixel` directly.)

#[inline]
fn wind_rgba(key: f64, alp: f64, ratio: f64, sw: &mut bool, tgt: &mut f64) {
    if *tgt < key {
        *tgt += ratio * (key - *tgt) * alp;
        *tgt = tgt.clamp(0.0, 1.0);
        *sw = true;
    }
}

#[inline]
fn wind_rgb(key: f64, ratio: f64, sw: &mut bool, tgt: &mut f64) {
    if *tgt < key {
        *tgt += ratio * (key - *tgt);
        *tgt = tgt.clamp(0.0, 1.0);
        *sw = true;
    }
}

#[inline]
fn wind_a(key: f64, ratio: f64, sw: bool, tgt: &mut f64) {
    if *tgt < key || sw {
        let val = *tgt + ratio * (key - *tgt);
        if *tgt < val {
            *tgt = val.clamp(0.0, 1.0);
        }
    }
}

fn blow_wind(
    channels: i32,
    key: &[f64],
    tbl_a: &[f64],
    tbl_p: usize,
    blow_alpha_sw: bool,
    tgt: &mut [f64],
) {
    let mut sw = false;

    if channels == RGBA_CHANNELS {
        if blow_alpha_sw {
            wind_rgb(key[0], tbl_a[tbl_p], &mut sw, &mut tgt[0]);
            wind_rgb(key[1], tbl_a[tbl_p], &mut sw, &mut tgt[1]);
            wind_rgb(key[2], tbl_a[tbl_p], &mut sw, &mut tgt[2]);
            wind_a(key[3], tbl_a[tbl_p], sw, &mut tgt[3]);
        } else {
            wind_rgba(key[0], tgt[3], tbl_a[tbl_p], &mut sw, &mut tgt[0]);
            wind_rgba(key[1], tgt[3], tbl_a[tbl_p], &mut sw, &mut tgt[1]);
            wind_rgba(key[2], tgt[3], tbl_a[tbl_p], &mut sw, &mut tgt[2]);
        }
    } else {
        for zz in 0..channels as usize {
            wind_rgb(key[zz], tbl_a[tbl_p], &mut sw, &mut tgt[zz]);
        }
    }
}

fn invert_pixel(channels: i32, pixel: &mut [f64]) {
    if channels == RGBA_CHANNELS {
        let alp = pixel[3];
        pixel[0] = (1.0 - pixel[0]) * alp;
        pixel[1] = (1.0 - pixel[1]) * alp;
        pixel[2] = (1.0 - pixel[2]) * alp;
    } else if channels == RGB_CHANNELS {
        pixel[0] = 1.0 - pixel[0];
        pixel[1] = 1.0 - pixel[1];
        pixel[2] = 1.0 - pixel[2];
    } else {
        for val in pixel.iter_mut().take(channels as usize) {
            *val = 1.0 - *val;
        }
    }
}

#[inline]
fn rgb_to_lightness(re: f64, gr: f64, bl: f64) -> f64 {
    // Faithful translation of `rgb_to_lightness_` in igs_motion_wind_pixel.cpp.
    // It is NOT (max + min) / 2: the `re < gr` branch divides only max(gr, bl) by
    // two, so the result depends on channel order. Kept verbatim for parity; the
    // differential golden pins this behaviour.
    if re < gr {
        (if gr < bl { bl } else { gr }) / 2.0
    } else {
        let a = if re < bl { bl } else { re };
        let b = if gr < re {
            if bl < gr {
                bl
            } else {
                gr
            }
        } else if bl < re {
            bl
        } else {
            re
        };
        (a + b) / 2.0
    }
}

fn get_lightness(channels: i32, pixel: &[f64]) -> f64 {
    if channels == RGBA_CHANNELS || channels == RGB_CHANNELS {
        rgb_to_lightness(pixel[RED], pixel[GRE], pixel[BLU])
    } else {
        pixel[0]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lightness_grayscale() {
        let pixel = [0.5];
        assert!((get_lightness(1, &pixel) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn lightness_rgb() {
        // Toonz order: index 0 is blue, 2 is red.
        let pixel = [1.0, 0.0, 0.0]; // pure blue
        assert!((get_lightness(3, &pixel) - 0.5).abs() < 1e-9); // (1+0)/2
    }

    #[test]
    fn invert_rgb() {
        let mut pixel = [0.2, 0.5, 0.8];
        invert_pixel(3, &mut pixel);
        assert!((pixel[0] - 0.8).abs() < 1e-9);
        assert!((pixel[1] - 0.5).abs() < 1e-9);
        assert!((pixel[2] - 0.2).abs() < 1e-9);
    }

    #[test]
    fn processor_resets_key() {
        let mut proc = MotionWindPixel::new(
            false, false, 1, 5.0, 10.0, 1.0, false, 2, 1.0, 1.0, 1.0, false, 3, 1.0, 1.0, 1.0,
            false,
        );
        let mut pixel = [0.8, 0.7, 0.6];
        proc.change(true, -1.0, 3, &mut pixel);
        // b,g,r = 0.8,0.7,0.6 → re=0.6 < gr=0.7 → max(gr,bl)/2 = 0.8/2
        assert!((proc.key_lightness - 0.4).abs() < 1e-9);
    }

    /// Platform-independent invariants for the pixel arithmetic: no golden, no recorded
    /// RNG stream, no libm comparison. This runs on Windows and Linux as well as macOS,
    /// which is what "covered on three platforms" has to mean for the port itself.
    #[test]
    fn invariants_hold_for_every_channel_count() {
        for dark in [false, true] {
            for channels in 1..=4i32 {
                let mut proc = MotionWindPixel::new(
                    dark, true, 1, 2.0, 6.0, 1.0, false, 1, 1.0, 1.0, 1.0, false, 1, 1.0, 1.0,
                    1.0, false,
                );
                for step in 0..200usize {
                    let mut px = vec![0.0f64; channels as usize];
                    for (c, v) in px.iter_mut().enumerate() {
                        *v = ((step * 37 + c * 11) % 101) as f64 / 100.0;
                    }
                    proc.change(step % 16 == 0, -1.0, channels, &mut px);
                    for (c, v) in px.iter().enumerate() {
                        assert!(
                            v.is_finite() && (0.0..=1.0).contains(v),
                            "dark={dark} channels={channels} step={step} ch={c}: {v}"
                        );
                    }
                    if proc.table_len > 0 {
                        assert!(
                            proc.table_pos <= proc.table_len,
                            "dark={dark} channels={channels} step={step}: pos {} > len {}",
                            proc.table_pos,
                            proc.table_len
                        );
                    }
                }
                proc.clear();
                assert_eq!(proc.table_len, 0);
                assert_eq!(proc.table_pos, 0);
            }
        }
    }

    /// Golden from the original C++ (`tests/native/gen_igs_motion_wind_cases.py`).
    ///
    /// Three fixed configurations driven over a deterministic 400-pixel sequence, in both
    /// `unsigned long` widths the original was ever built with. `inside` must match
    /// exactly; channels must match exactly for the arithmetic-only configs and within
    /// 1e-12 for the gamma (`pow`) config.
    ///
    /// The original LCG derives *both* its modulus and its `next_d` denominator from
    /// `unsigned long`, so the stream is 64-bit on LP64 (macOS, Linux) and 32-bit on LLP64
    /// (Windows). Two goldens, both recorded on POSIX by
    /// `gen_igs_motion_wind_cases.py --rng-bits {64,32}`, which rebuilds the reference
    /// against an RNG stub of the requested width. Selecting on `size_of::<c_ulong>()`
    /// rather than `cfg(target_os)` also covers i686 Linux, where `c_ulong` is 32-bit too.
    ///
    /// Do not widen the RNG to `u64` to make one golden replay everywhere: the Windows
    /// original used a 32-bit `unsigned long`, and reproducing it is the point.
    #[test]
    fn matches_the_cpp_reference() {
        let golden: &str = if std::mem::size_of::<std::os::raw::c_ulong>() == 8 {
            include_str!("igs_motion_wind_cases.txt")
        } else {
            include_str!("igs_motion_wind_cases_32.txt")
        };

        // cfg index -> (blow_dark, blow_alpha, length seed/min/max/bias/ref,
        //              force seed/min/max/bias/ref, density seed/min/max/bias/ref,
        //              exact_bits)
        #[allow(clippy::type_complexity)]
        const CFGS: [(
            bool, bool,
            u64, f64, f64, f64, bool,
            u64, f64, f64, f64, bool,
            u64, f64, f64, f64, bool,
            bool,
        ); 3] = [
            (false, true, 1, 2.0, 6.0, 1.0, false, 1, 1.0, 1.0, 1.0, false, 1, 1.0, 1.0, 1.0, false, true),
            (true, true, 7, 1.0, 5.0, 0.5, false, 7, 0.5, 1.5, 1.0, false, 7, 0.8, 1.0, 1.0, false, false),
            (false, false, 3, 3.0, 9.0, 1.0, true, 3, 1.0, 1.0, 1.0, false, 3, 1.0, 1.0, 1.0, false, true),
        ];

        let mut blocks = golden.split("\n\n").filter(|b| !b.trim().is_empty());
        for (ci, cfg) in CFGS.iter().enumerate() {
            let block = blocks.next().unwrap_or_else(|| panic!("missing block {ci}"));
            let mut lines = block.lines();
            assert_eq!(
                lines.next().unwrap().trim(),
                format!("cfg {ci}"),
                "block order"
            );

            let mut proc = MotionWindPixel::new(
                cfg.0, cfg.1, cfg.2, cfg.3, cfg.4, cfg.5, cfg.6, cfg.7, cfg.8, cfg.9, cfg.10,
                cfg.11, cfg.12, cfg.13, cfg.14, cfg.15, cfg.16,
            );

            for step in 0..400usize {
                let line = lines.next().unwrap_or_else(|| panic!("cfg {ci}: missing step {step}"));
                let t: Vec<&str> = line.split_whitespace().collect();
                let want_step: usize = t[1].parse().unwrap();
                let want_inside: i32 = t[2].parse().unwrap();
                assert_eq!(want_step, step);

                let mut px = [0.0f64; 4];
                for (ch, slot) in px.iter_mut().enumerate() {
                    *slot = ((step * 37 + ch * 11) % 101) as f64 / 100.0;
                }
                let got_inside = proc.change(step % 16 == 0, -1.0, 4, &mut px) as i32;
                assert_eq!(got_inside, want_inside, "cfg {ci} step {step}: inside");

                for ch in 0..4 {
                    let want = f64::from_bits(u64::from_str_radix(t[3 + ch], 16).unwrap());
                    let got = px[ch];
                    if cfg.17 {
                        assert_eq!(
                            got.to_bits(),
                            want.to_bits(),
                            "cfg {ci} step {step} ch {ch}: got {got}, want {want}"
                        );
                    } else {
                        assert!(
                            (got - want).abs() <= 1e-12,
                            "cfg {ci} step {step} ch {ch}: got {got}, want {want}"
                        );
                    }
                }
            }
        }
    }
}
