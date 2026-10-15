//! Port of common/tcore/trandom.cpp: the additive lagged-Fibonacci generator
//! (Knuth's ran3 style, 55 words) behind `TRandom`.  `TRandom` below mirrors the
//! C++ object layout (UINT seed; int idx1, idx2; UINT ran[56]) so trandom.h can
//! pass `this` to the C ABI.  Only float layout type 1 (IEEE 754, 0x3f800000 ==
//! 1.0f) exists here; type 2 was an SGI/IRIX big-endian quirk.

pub const C_MAXUINT: u32 = u32::MAX;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct TRandom {
    pub seed: u32,
    pub idx1: i32,
    pub idx2: i32,
    pub ran: [u32; 56],
}

impl TRandom {
    pub fn new(seed: u32) -> TRandom {
        let mut r = TRandom { seed, idx1: 0, idx2: 0, ran: [0; 56] };
        r.reset();
        r
    }

    pub fn set_seed(&mut self, s: u32) {
        self.seed = s;
        self.reset();
    }

    pub fn reset(&mut self) {
        let mut uj: u32 = 161803398u32.wrapping_sub(self.seed);
        self.ran[55] = uj;
        let mut uk: u32 = 1;
        for i in 1..=54usize {
            let ii = (21 * i) % 55;
            self.ran[ii] = uk;
            uk = uj.wrapping_sub(uk);
            uj = self.ran[ii];
        }
        for _k in 1..=4 {
            for i in 1..=55usize {
                self.ran[i] = self.ran[i].wrapping_sub(self.ran[1 + (i + 30) % 55]);
            }
        }
        self.idx1 = 55;
        self.idx2 = 31;
    }

    fn get_next_uint(&mut self) -> u32 {
        self.idx1 += 1;
        if self.idx1 == 56 {
            self.idx1 = 1;
        }
        self.idx2 += 1;
        if self.idx2 == 56 {
            self.idx2 = 1;
        }
        let (a, b) = (self.idx1 as usize, self.idx2 as usize);
        self.ran[a] = self.ran[a].wrapping_sub(self.ran[b]);
        self.ran[a]
    }

    /// [0,end[
    pub fn get_uint(&mut self, end: u32) -> u32 {
        if end == 0 {
            return 0;
        }
        let u = self.get_next_uint();
        if end == C_MAXUINT {
            return u;
        }
        u % end
    }

    /// [begin, end[
    pub fn get_int(&mut self, begin: i32, end: i32) -> i32 {
        debug_assert!(end >= begin);
        let limit = end.wrapping_sub(begin);
        if limit == 0 {
            // end == begin
            return begin;
        }
        let u = self.get_next_uint();
        // `u % limit + begin` is evaluated in unsigned arithmetic in C++
        (u % (limit as u32)).wrapping_add(begin as u32) as i32
    }

    /// [0,1[
    pub fn get_float(&mut self) -> f32 {
        let mut u = self.get_next_uint();
        // RANDOM_FLOAT_TYPE_1
        u = ((u >> 5) & 0x007fffff) | 0x3f800000;
        f32::from_bits(u) - 1.0
    }

    /// [0, end[
    pub fn get_float_end(&mut self, end: f32) -> f32 {
        self.get_float() * end
    }

    /// [begin, end[
    pub fn get_float_range(&mut self, begin: f32, end: f32) -> f32 {
        debug_assert!(end >= begin);
        (self.get_float() * (end - begin)) + begin
    }

    pub fn get_bool(&mut self) -> bool {
        let u = self.get_next_uint();
        u & 1 != 0
    }

    /// [0,1[  (the original simply returns getFloat())
    pub fn get_double(&mut self) -> f64 {
        self.get_float() as f64
    }
}

//=============================================================================
// C ABI (trandom.h forwards to these with `this`)

/// # Safety
/// `r` must point to a TRandom-layout object.
#[no_mangle]
pub unsafe extern "C" fn flare_trandom_reset(r: *mut TRandom) {
    (*r).reset();
}
/// # Safety
/// `r` must point to a TRandom-layout object.
#[no_mangle]
pub unsafe extern "C" fn flare_trandom_set_seed(r: *mut TRandom, s: u32) {
    (*r).set_seed(s);
}
/// # Safety
/// `r` must point to a TRandom-layout object.
#[no_mangle]
pub unsafe extern "C" fn flare_trandom_get_uint(r: *mut TRandom, end: u32) -> u32 {
    (*r).get_uint(end)
}
/// # Safety
/// `r` must point to a TRandom-layout object.
#[no_mangle]
pub unsafe extern "C" fn flare_trandom_get_int(r: *mut TRandom, begin: i32, end: i32) -> i32 {
    (*r).get_int(begin, end)
}
/// # Safety
/// `r` must point to a TRandom-layout object.
#[no_mangle]
pub unsafe extern "C" fn flare_trandom_get_float(r: *mut TRandom) -> f32 {
    (*r).get_float()
}
/// # Safety
/// `r` must point to a TRandom-layout object.
#[no_mangle]
pub unsafe extern "C" fn flare_trandom_get_float_end(r: *mut TRandom, end: f32) -> f32 {
    (*r).get_float_end(end)
}
/// # Safety
/// `r` must point to a TRandom-layout object.
#[no_mangle]
pub unsafe extern "C" fn flare_trandom_get_float_range(r: *mut TRandom, begin: f32, end: f32) -> f32 {
    (*r).get_float_range(begin, end)
}
/// # Safety
/// `r` must point to a TRandom-layout object.
#[no_mangle]
pub unsafe extern "C" fn flare_trandom_get_bool(r: *mut TRandom) -> bool {
    (*r).get_bool()
}
/// # Safety
/// `r` must point to a TRandom-layout object.
#[no_mangle]
pub unsafe extern "C" fn flare_trandom_get_double(r: *mut TRandom) -> f64 {
    (*r).get_double()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_matches_cpp_object() {
        // UINT seed; int idx1, idx2; UINT ran[56]  => 4 + 4 + 4 + 224
        assert_eq!(std::mem::size_of::<TRandom>(), 236);
    }

    #[test]
    fn reset_state_after_construction() {
        let r = TRandom::new(0);
        assert_eq!((r.idx1, r.idx2), (55, 31));
        // same seed -> same sequence; different seed -> different
        let mut a = TRandom::new(7);
        let mut b = TRandom::new(7);
        let mut c = TRandom::new(8);
        let sa: Vec<u32> = (0..20).map(|_| a.get_uint(C_MAXUINT)).collect();
        let sb: Vec<u32> = (0..20).map(|_| b.get_uint(C_MAXUINT)).collect();
        let sc: Vec<u32> = (0..20).map(|_| c.get_uint(C_MAXUINT)).collect();
        assert_eq!(sa, sb);
        assert_ne!(sa, sc);
    }

    #[test]
    fn golden_sequence_seed0_matches_hand_port_of_reset() {
        // Independent re-derivation of the generator state straight from the C++ text.
        let mut ran = [0u32; 56];
        let mut uj: u32 = 161803398;
        ran[55] = uj;
        let mut uk: u32 = 1;
        for i in 1..=54usize {
            let ii = (21 * i) % 55;
            ran[ii] = uk;
            uk = uj.wrapping_sub(uk);
            uj = ran[ii];
        }
        for _ in 0..4 {
            for i in 1..=55usize {
                ran[i] = ran[i].wrapping_sub(ran[1 + (i + 30) % 55]);
            }
        }
        let (mut i1, mut i2) = (55usize, 31usize);
        let mut r = TRandom::new(0);
        for _ in 0..200 {
            i1 = if i1 + 1 == 56 { 1 } else { i1 + 1 };
            i2 = if i2 + 1 == 56 { 1 } else { i2 + 1 };
            ran[i1] = ran[i1].wrapping_sub(ran[i2]);
            assert_eq!(r.get_uint(C_MAXUINT), ran[i1]);
        }
    }

    #[test]
    fn ranges() {
        let mut r = TRandom::new(123);
        assert_eq!(r.get_uint(0), 0); // end == 0 does not advance
        assert_eq!(r.get_int(5, 5), 5);
        for _ in 0..500 {
            let u = r.get_uint(10);
            assert!(u < 10);
            let i = r.get_int(-3, 4);
            assert!((-3..4).contains(&i));
            let f = r.get_float();
            assert!((0.0..1.0).contains(&f));
            let g = r.get_float_end(5.0);
            assert!((0.0..5.0).contains(&g));
            let h = r.get_float_range(2.0, 3.0);
            assert!((2.0..3.0).contains(&h));
            let d = r.get_double();
            assert!((0.0..1.0).contains(&d));
        }
    }

    #[test]
    fn float_uses_bits_5_to_27() {
        // getFloat: ((u>>5)&0x7fffff)|0x3f800000, minus 1
        let mut a = TRandom::new(5);
        let mut b = TRandom::new(5);
        let u = a.get_uint(C_MAXUINT);
        let f = b.get_float();
        assert_eq!(f, f32::from_bits(((u >> 5) & 0x007fffff) | 0x3f800000) - 1.0);
    }

    #[test]
    fn bool_is_lsb_and_set_seed_resets() {
        let mut a = TRandom::new(9);
        let mut b = TRandom::new(9);
        let u = a.get_uint(C_MAXUINT);
        assert_eq!(b.get_bool(), u & 1 == 1);
        // setSeed(9) restarts the very same sequence
        a.set_seed(9);
        assert_eq!(a.get_uint(C_MAXUINT), u);
    }

    #[test]
    fn ffi_walks_same_sequence() {
        let mut a = TRandom::new(3);
        let mut b = TRandom::new(3);
        unsafe {
            flare_trandom_reset(&mut b);
            for _ in 0..10 {
                assert_eq!(flare_trandom_get_uint(&mut b, 100), a.get_uint(100));
            }
        }
    }
}
