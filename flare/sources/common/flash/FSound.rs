//! Port of `common/flash/FSound.cpp`: the SWF ADPCM sound compressor (`FSoundComp`).
//!
//! `FSound` (format flags, rate tables, `Set(WaveFormat*)`) and the `FSoundComp` class keep
//! their layout in `FSound.h`; the compressor state is a plain `repr(C)` struct
//! ([`SoundCompState`]) owned by the C++ object and passed to the kernels by pointer:
//!
//! * [`flare_soundcomp_compress`] = `FSoundComp::Compress` (8-bit input is widened through a
//!   4096-sample buffer exactly like the C++, then `Compress16`);
//! * [`flare_soundcomp_flush`] = `FSoundComp::Flush`;
//! * [`flare_soundcomp_max_out`] = an upper bound for the bytes one `Compress`/`Flush` call can
//!   append, so the header can size a scratch buffer and append it to the caller's
//!   `std::vector<U8>` (Rust cannot push into a `std::vector`).
//!
//! The "just counting" mode of the C++ (`stream == NULL`) is kept (`out == NULL`): it only
//! advances `len`.
//!
//! Deviations (the C++ was undefined behaviour or read uninitialised memory):
//! * `Compress16` looked at `src[sn]` (the next frame) when starting a 4096-sample block, which
//!   runs one frame past the data when the block starts on the last frame. Here every read past
//!   the end of the source reads 0. The 8-bit path reads past its 4096-entry `buf` the same way
//!   (also 0); `buf` itself is zero-initialised once per call and keeps stale data between the
//!   chunks of one call, as the C++ stack array did.
//! * 8-bit **stereo** input (`n` is passed to `Filter8to16` as a byte count but `Compress16`
//!   consumes `2n` entries) reads those missing entries as 0 instead of stack garbage.
//! * `nBits` outside 2..=5 (an out-of-range `indexTables[]` access) is refused (status 1).
//! * `valpred`/`index` start at 0 (the constructor left them uninitialised; the first block
//!   header always sets them before any read).

const INDEX_TABLE2: [i32; 2] = [-1, 2];
const INDEX_TABLE3: [i32; 4] = [-1, -1, 2, 4];
const INDEX_TABLE4: [i32; 8] = [-1, -1, -1, -1, 2, 4, 6, 8];
const INDEX_TABLE5: [i32; 16] = [-1, -1, -1, -1, -1, -1, -1, -1, 1, 2, 4, 6, 8, 10, 13, 16];

const STEPSIZE_TABLE: [i32; 89] = [
    7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 21, 23, 25, 28, 31, 34, 37, 41, 45, 50, 55, 60, 66, 73, 80, 88, 97, 107, 118, 130, 143, 157, 173,
    190, 209, 230, 253, 279, 307, 337, 371, 408, 449, 494, 544, 598, 658, 724, 796, 876, 963, 1060, 1166, 1282, 1411, 1552, 1707, 1878, 2066,
    2272, 2499, 2749, 3024, 3327, 3660, 4026, 4428, 4871, 5358, 5894, 6484, 7132, 7845, 8630, 9493, 10442, 11487, 12635, 13899, 15289, 16818,
    18500, 20350, 22385, 24623, 27086, 29794, 32767,
];

fn index_table(n_bits: i32) -> &'static [i32] {
    match n_bits {
        2 => &INDEX_TABLE2,
        3 => &INDEX_TABLE3,
        4 => &INDEX_TABLE4,
        _ => &INDEX_TABLE5,
    }
}

/// Layout of the private state of `FSoundComp` (`FSound.h`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SoundCompState {
    pub is_stereo: i32,
    pub is_8bit: i32,
    /// number of bits in each sample (2..=5)
    pub n_bits: i32,
    /// samples compressed so far
    pub n_samples: i32,
    /// ADPCM state
    pub valpred: [i32; 2],
    pub index: [i32; 2],
    /// bytes produced so far
    pub len: i32,
    pub bit_buf: u32,
    pub bit_pos: i32,
}

/// Where the bytes go: a vector, or nowhere (counting only).
struct Sink<'a>(Option<&'a mut Vec<u8>>);

impl SoundCompState {
    fn write_bits(&mut self, sink: &mut Sink) {
        match sink.0.as_deref_mut() {
            Some(v) => {
                while self.bit_pos >= 8 {
                    v.push((self.bit_buf >> (self.bit_pos - 8)) as u8);
                    self.bit_pos -= 8;
                    self.len = self.len.wrapping_add(1);
                }
            }
            None => {
                self.len = self.len.wrapping_add(self.bit_pos / 8);
                self.bit_pos &= 0x7;
            }
        }
    }

    fn put_bits(&mut self, v: i32, n: i32, sink: &mut Sink) {
        debug_assert!(n <= 16);
        if self.bit_pos + n > 32 {
            self.write_bits(sink);
        }
        self.bit_buf = (self.bit_buf << n) | ((v as u32) & ((1u32 << n) - 1));
        self.bit_pos += n;
    }

    fn flush(&mut self, sink: &mut Sink) {
        self.write_bits(sink);
        if self.bit_pos > 0 {
            self.put_bits(0, 8 - self.bit_pos, sink);
            self.write_bits(sink);
        }
    }

    /// `Compress16`. `src(i)` is the i-th 16-bit sample (0 past the end of the data).
    fn compress16(&mut self, src: &dyn Fn(usize) -> i32, mut n: i32, sink: &mut Sink) {
        if self.n_samples == 0 {
            // Emit the compression settings
            self.put_bits(self.n_bits - 2, 2, sink);
        }

        let sn: usize = if self.is_stereo != 0 { 2 } else { 1 };
        let index_table = index_table(self.n_bits);
        let mut pos: usize = 0;
        while n > 0 {
            n -= 1;
            self.n_samples = self.n_samples.wrapping_add(1);
            if (self.n_samples & 0xfff) == 1 {
                // We emit a header every 4096 samples so we can seek quickly
                for i in 0..sn {
                    // Pick an initial index value
                    let d = (src(pos + sn) - src(pos)).abs();
                    let mut k = 0;
                    while k < 63 && STEPSIZE_TABLE[k] < d {
                        k += 1;
                    }
                    let s = src(pos);
                    pos += 1;
                    self.valpred[i] = s;
                    self.put_bits(s, 16, sink);
                    self.index[i] = k as i32;
                    self.put_bits(k as i32, 6, sink);
                }
            } else {
                // Generate a delta value
                for i in 0..sn {
                    /* Step 1 - compute difference with previous value */
                    let mut diff = src(pos) - self.valpred[i];
                    pos += 1;
                    let sign;
                    if diff < 0 {
                        sign = 1 << (self.n_bits - 1);
                        diff = -diff;
                    } else {
                        sign = 0;
                    }

                    /* Step 2 - Divide and clamp */
                    let mut step = STEPSIZE_TABLE[self.index[i] as usize];
                    let mut delta: i32 = 0;
                    let mut vpdiff: i32 = 0;

                    let mut k = 1 << (self.n_bits - 2);
                    loop {
                        if diff >= step {
                            delta |= k;
                            diff -= step;
                            vpdiff += step;
                        }
                        step >>= 1;
                        k >>= 1;
                        if k == 0 {
                            break;
                        }
                    }
                    vpdiff += step; // add the 0.5

                    /* Step 3 - Update previous value */
                    if sign != 0 {
                        self.valpred[i] -= vpdiff;
                    } else {
                        self.valpred[i] += vpdiff;
                    }

                    /* Step 4 - Clamp previous value to 16 bits */
                    if self.valpred[i] != (self.valpred[i] as i16) as i32 {
                        self.valpred[i] = if self.valpred[i] < 0 { -32768 } else { 32767 };
                    }

                    /* Step 5 - Assemble value, update index and step values */
                    self.index[i] += index_table[delta as usize];
                    if self.index[i] < 0 {
                        self.index[i] = 0;
                    } else if self.index[i] > 88 {
                        self.index[i] = 88;
                    }

                    delta |= sign;

                    /* Step 6 - Output value */
                    self.put_bits(delta, self.n_bits, sink);
                }
            }
        }
    }

    /// `Compress`: `src` is the raw byte buffer (8-bit samples or native-endian 16-bit).
    fn compress(&mut self, src: &[u8], mut n: i32, sink: &mut Sink) {
        if self.is_8bit != 0 {
            let mut buf = vec![0i32; 8192 + 4];
            let mut off = 0usize;
            while n > 0 {
                // Expand to 16 bit and compress
                let nb = n.min(4096);
                for i in 0..nb as usize {
                    // Filter8to16: ((S16)src - 128) << 8, as S16
                    let v = src.get(off + i).copied().unwrap_or(0) as i32;
                    buf[i] = (((v - 128) << 8) as i16) as i32;
                }
                let b = &buf;
                self.compress16(&|i| b.get(i).copied().unwrap_or(0), nb, sink);
                n -= nb;
                off += nb as usize;
            }
        } else {
            let at = |i: usize| -> i32 {
                match (src.get(2 * i), src.get(2 * i + 1)) {
                    (Some(&a), Some(&b)) => i16::from_ne_bytes([a, b]) as i32,
                    _ => 0,
                }
            };
            self.compress16(&at, n, sink);
        }
    }
}

/// Upper bound on the bytes appended by one `Compress(src, n)` or `Flush` call.
pub fn max_out(st: &SoundCompState, n: i32) -> usize {
    let n = n.max(0) as u64;
    let sn: u64 = if st.is_stereo != 0 { 2 } else { 1 };
    let bits = n * sn * 5 + (n / 4096 + 2) * sn * 22 + 2 + 32 + st.bit_pos.max(0) as u64;
    (bits / 8 + 8) as usize
}

fn valid(st: &SoundCompState) -> bool {
    (2..=5).contains(&st.n_bits)
}

/// Upper bound (bytes) for the output of `flare_soundcomp_compress(st, src, n, ..)` /
/// `flare_soundcomp_flush`; 0 for a null state.
///
/// # Safety
/// `st` must be NULL or point to a valid state.
#[no_mangle]
pub unsafe extern "C" fn flare_soundcomp_max_out(st: *const SoundCompState, n: i32) -> usize {
    if st.is_null() {
        return 0;
    }
    max_out(&*st, n)
}

/// `FSoundComp::Compress`. With `out == NULL` only the byte counter advances. Otherwise the
/// produced bytes go to `out[..*out_len]` (capacity `out_cap`; status 3 when too small).
/// Returns 0 ok, 1 invalid argument (null state/source, `nBits` not in 2..=5), 2 panic.
///
/// `src` is read for `n` bytes (8-bit input) or `n * channels` 16-bit samples (otherwise);
/// anything the algorithm looks at beyond that reads as 0.
///
/// # Safety
/// `st` valid state; `src` valid for the byte count above; `out` valid for `out_cap` bytes;
/// `out_len` valid for one `usize` (may be NULL when `out` is NULL).
#[no_mangle]
pub unsafe extern "C" fn flare_soundcomp_compress(
    st: *mut SoundCompState,
    src: *const u8,
    n: i32,
    out: *mut u8,
    out_cap: usize,
    out_len: *mut usize,
) -> i32 {
    if st.is_null() || !valid(&*st) || (n > 0 && src.is_null()) || (!out.is_null() && out_len.is_null()) {
        return 1;
    }
    crate::guard(2, || {
        let state = &mut *st;
        let bytes = if n <= 0 {
            0usize
        } else if state.is_8bit != 0 {
            n as usize
        } else {
            n as usize * if state.is_stereo != 0 { 4 } else { 2 }
        };
        let data: &[u8] = if bytes == 0 { &[] } else { std::slice::from_raw_parts(src, bytes) };
        finish(state, out, out_cap, out_len, |s, sink| s.compress(data, n, sink))
    })
}

/// `FSoundComp::Flush`; same output convention as [`flare_soundcomp_compress`].
///
/// # Safety
/// As [`flare_soundcomp_compress`].
#[no_mangle]
pub unsafe extern "C" fn flare_soundcomp_flush(st: *mut SoundCompState, out: *mut u8, out_cap: usize, out_len: *mut usize) -> i32 {
    if st.is_null() || !valid(&*st) || (!out.is_null() && out_len.is_null()) {
        return 1;
    }
    crate::guard(2, || finish(&mut *st, out, out_cap, out_len, |s, sink| s.flush(sink)))
}

unsafe fn finish(
    state: &mut SoundCompState,
    out: *mut u8,
    out_cap: usize,
    out_len: *mut usize,
    f: impl FnOnce(&mut SoundCompState, &mut Sink),
) -> i32 {
    if out.is_null() {
        f(state, &mut Sink(None));
        return 0;
    }
    let mut v = Vec::new();
    // work on a copy so a too-small buffer leaves the state untouched
    let mut s = *state;
    f(&mut s, &mut Sink(Some(&mut v)));
    if v.len() > out_cap {
        return 3;
    }
    if !v.is_empty() {
        std::slice::from_raw_parts_mut(out, v.len()).copy_from_slice(&v);
    }
    *out_len = v.len();
    *state = s;
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(stereo: bool, is8: bool, nbits: i32) -> SoundCompState {
        SoundCompState { is_stereo: stereo as i32, is_8bit: is8 as i32, n_bits: nbits, ..Default::default() }
    }

    fn run(st: &mut SoundCompState, pieces: &[(&[u8], i32)]) -> Vec<u8> {
        let mut out = Vec::new();
        for &(src, n) in pieces {
            let mut buf = vec![0u8; max_out(st, n)];
            let mut len = 0usize;
            let rc = unsafe { flare_soundcomp_compress(st, src.as_ptr(), n, buf.as_mut_ptr(), buf.len(), &mut len) };
            assert_eq!(rc, 0);
            out.extend_from_slice(&buf[..len]);
        }
        let mut buf = vec![0u8; max_out(st, 0)];
        let mut len = 0usize;
        assert_eq!(unsafe { flare_soundcomp_flush(st, buf.as_mut_ptr(), buf.len(), &mut len) }, 0);
        out.extend_from_slice(&buf[..len]);
        out
    }

    #[test]
    fn first_sample_header_and_padding() {
        // one 16-bit mono sample, 2-bit codes: 2 bits (0) + 16 bits value + 6 bits index
        // value 0x1234, lookahead reads 0 -> d = 4660 -> index k: first step >= d
        let mut st = state(false, false, 2);
        let src = 0x1234i16.to_ne_bytes();
        let out = run(&mut st, &[(&src, 1)]);
        assert_eq!(out.len(), 3); // 24 bits exactly
        assert_eq!(out[0] >> 6, 0); // nBits - 2
        // |0 - 4660| is above every step up to index 62, so the index search stops at 63
        assert_eq!(out[2] & 0x3f, 63);
        assert_eq!(st.len, 3);
        assert_eq!(st.bit_pos, 0);
    }

    #[test]
    fn counting_mode_matches_the_byte_count() {
        let mut data = Vec::new();
        for i in 0..5000i32 {
            data.extend_from_slice(&(((i * 37) % 20000 - 10000) as i16).to_ne_bytes());
        }
        let mut a = state(false, false, 4);
        let produced = run(&mut a, &[(&data, 5000)]);
        let mut b = state(false, false, 4);
        unsafe {
            assert_eq!(flare_soundcomp_compress(&mut b, data.as_ptr(), 5000, std::ptr::null_mut(), 0, std::ptr::null_mut()), 0);
            assert_eq!(flare_soundcomp_flush(&mut b, std::ptr::null_mut(), 0, std::ptr::null_mut()), 0);
        }
        assert_eq!(b.len as usize, produced.len());
        assert_eq!(a, b);
    }

    #[test]
    fn rejects_bad_arguments_and_small_buffers() {
        let mut st = state(false, false, 6);
        let src = [0u8; 4000];
        let mut out = [0u8; 8];
        let mut len = 0;
        unsafe {
            assert_eq!(flare_soundcomp_compress(&mut st, src.as_ptr(), 1, out.as_mut_ptr(), 8, &mut len), 1);
            assert_eq!(flare_soundcomp_compress(std::ptr::null_mut(), src.as_ptr(), 1, out.as_mut_ptr(), 8, &mut len), 1);
            st.n_bits = 3;
            assert_eq!(flare_soundcomp_compress(&mut st, std::ptr::null(), 1, out.as_mut_ptr(), 8, &mut len), 1);
            assert_eq!(flare_soundcomp_compress(&mut st, src.as_ptr(), 2000, out.as_mut_ptr(), 1, &mut len), 3);
            assert_eq!(st.n_samples, 0); // state untouched by the refused call
            assert_eq!(flare_soundcomp_max_out(std::ptr::null(), 4), 0);
        }
    }

    #[test]
    fn eight_bit_widening() {
        // 8-bit 128 is silence: valpred 0, so the 2-bit codes are all zero deltas
        let mut st = state(false, true, 2);
        let src = [128u8; 6];
        let out = run(&mut st, &[(&src, 6)]);
        assert!(!out.is_empty());
        assert_eq!(st.n_samples, 6);
    }

    // Replays the recordings of the original C++ (swfwriter_cases.txt).
    #[test]
    fn replay_recorded_cpp_cases() {
        let un = |s: &str| -> Vec<u8> {
            if s == "-" {
                return vec![];
            }
            (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap()).collect()
        };
        let mut n = 0;
        for line in include_str!("swfwriter_cases.txt").lines().filter(|l| l.starts_with("sound|")) {
            let f: Vec<&str> = line.split('|').collect();
            let (stereo, is8, nbits): (bool, bool, i32) = (f[1] == "1", f[2] == "1", f[3].parse().unwrap());
            let pieces: Vec<i32> = f[4].split(',').filter(|s| !s.is_empty()).map(|s| s.parse().unwrap()).collect();
            let data = un(f[5]);
            let mut st = state(stereo, is8, nbits);
            let sn = if stereo { 2 } else { 1 };
            let mut off = 0;
            let mut parts: Vec<(&[u8], i32)> = Vec::new();
            for p in &pieces {
                let bytes = *p as usize * sn * if is8 { 1 } else { 2 };
                parts.push((&data[off..off + bytes], *p));
                off += bytes;
            }
            assert_eq!(run(&mut st, &parts), un(f[6]), "{line}");
            n += 1;
        }
        assert_eq!(n, 36);
    }
}
