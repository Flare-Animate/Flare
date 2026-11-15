//! The energy in a few frequency bands of a short stretch of sound: a Hann window and an
//! in-place radix-2 FFT, enough for one frame of a voice.

use std::f32::consts::TAU;

/// The FFT sizes used: big enough for the low band's resolution, small enough to stay quick.
const MIN_SIZE: usize = 256;
const MAX_SIZE: usize = 4096;

/// The energy between each pair of neighbouring `edges` (in Hz) of `samples` at `rate`.
pub(super) fn band_energy(samples: &[f32], rate: f32, edges: &[f32]) -> Vec<f32> {
    let n = samples.len().next_power_of_two().clamp(MIN_SIZE, MAX_SIZE);
    let used = samples.len().min(n);
    let mut re = vec![0.0_f32; n];
    let mut im = vec![0.0_f32; n];
    let width = used.max(2) as f32;
    for (i, (slot, sample)) in re.iter_mut().zip(samples).enumerate() {
        *slot = sample * (0.5 - 0.5 * (TAU * i as f32 / width).cos());
    }
    transform(&mut re, &mut im);
    let hz = rate / n as f32;
    edges
        .windows(2)
        .map(|pair| {
            let low = ((pair[0] / hz) as usize).min(n / 2);
            let high = ((pair[1] / hz) as usize).min(n / 2);
            (low..high.max(low))
                .map(|k| re[k] * re[k] + im[k] * im[k])
                .sum()
        })
        .collect()
}

/// Cooley-Tukey, in place; `re.len()` is a power of two.
fn transform(re: &mut [f32], im: &mut [f32]) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let angle = -TAU / len as f32;
        for start in (0..n).step_by(len) {
            for k in 0..len / 2 {
                let (wr, wi) = ((angle * k as f32).cos(), (angle * k as f32).sin());
                let (a, b) = (start + k, start + k + len / 2);
                let (xr, xi) = (re[b] * wr - im[b] * wi, re[b] * wi + im[b] * wr);
                re[b] = re[a] - xr;
                im[b] = im[a] - xi;
                re[a] += xr;
                im[a] += xi;
            }
        }
        len <<= 1;
    }
}
