//! Port of flarelib/sandor_fxs/CIL.cpp: parse a colour-index list string such as
//! "0,3-5,12" / "all" / "-1" into a sorted, de-duplicated index array.
//!
//! `CCIL` keeps its public `m_nb`/`m_ci[]` layout; `CCIL::set` forwards here.
//! Deviations from the C++:
//! - The C++ copied the whole remaining tail of the list (not just the current
//!   token) into a 100-byte stack buffer with `strcpy`, so any list longer than
//!   100 chars could overflow it (UB); Rust has no such limit.
//! - `atoi` clamps to the `i32` range (see [`atoi`]) instead of truncating.

pub const MAXNBCI: usize = 4096;

/// C `atoi`: optional whitespace/sign, digits. Out-of-range values are clamped to
/// `i32::MIN..=i32::MAX` (platform independent). The C++ result was `(int)` of a
/// saturated `long`, i.e. implementation-defined truncation (e.g. "4294967297"
/// gave 1 and "-4294967296" gave 0); clamping makes such huge tokens invalid or
/// clamped by the caller rather than aliasing to small indices.
fn atoi(s: &[u8]) -> i32 {
    let mut i = 0;
    while i < s.len() && matches!(s[i], b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r') {
        i += 1;
    }
    let mut neg = false;
    if i < s.len() && (s[i] == b'+' || s[i] == b'-') {
        neg = s[i] == b'-';
        i += 1;
    }
    let mut v: i64 = 0;
    while i < s.len() && s[i].is_ascii_digit() {
        v = v.saturating_mul(10).saturating_add((s[i] - b'0') as i64);
        i += 1;
    }
    let v = if neg { -v } else { v };
    v.clamp(i32::MIN as i64, i32::MAX as i64) as i32
}

fn range_begin(s: &[u8]) -> i32 {
    let end = s.iter().position(|c| *c == b'-').unwrap_or(s.len());
    if end == 0 {
        return -1;
    }
    atoi(&s[..end])
}

fn range_end(s: &[u8]) -> i32 {
    let start = s.iter().rposition(|c| *c == b'-').map_or(0, |p| p + 1);
    if start >= s.len() {
        return -1;
    }
    atoi(&s[start..])
}

fn token(s: &[u8], out: &mut Vec<i32>, max_index: i32) {
    if s == b"-1" {
        let mut i = 0;
        while i <= max_index && out.len() < MAXNBCI {
            out.push(i);
            // max_index == i32::MAX would overflow `i += 1`; stop instead
            let Some(n) = i.checked_add(1) else { break };
            i = n;
        }
        return;
    }
    if s.contains(&b'-') {
        let (b, e) = (range_begin(s), range_end(s));
        if b >= 0 && e >= 0 {
            let (b, e) = (b.min(max_index), e.min(max_index));
            let (lo, hi) = (b.min(e), b.max(e));
            let mut i = lo;
            while i <= hi && out.len() < MAXNBCI {
                out.push(i);
                let Some(n) = i.checked_add(1) else { break };
                i = n;
            }
        }
    } else if out.len() < MAXNBCI {
        let q = atoi(s);
        if q >= 0 && q <= max_index {
            out.push(q);
        }
    }
}

fn contains(h: &[u8], n: &[u8]) -> bool {
    h.windows(n.len()).any(|w| w == n)
}

/// Sorted unique list, at most MAXNBCI entries.
pub fn parse(s: &[u8], max_index: i32) -> Vec<i32> {
    let mut tmp = Vec::new();
    if s.is_empty() || contains(s, b"all") || contains(s, b"ALL") {
        token(b"-1", &mut tmp, max_index);
    } else {
        for t in s.split(|c| *c == b',') {
            if !t.is_empty() {
                token(t, &mut tmp, max_index);
            }
        }
    }
    tmp.sort_unstable();
    tmp.dedup();
    tmp
}

/// `CCIL::set`: writes up to `cap` indices to `out`, returns the count (0 on bad args).
#[no_mangle]
pub unsafe extern "C" fn flare_cil_set(s: *const std::os::raw::c_char, max_index: i32, out: *mut i32, cap: i32) -> i32 {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if s.is_null() || out.is_null() || cap <= 0 {
            return 0;
        }
        let bytes = std::ffi::CStr::from_ptr(s).to_bytes();
        let v = parse(bytes, max_index);
        let n = v.len().min(cap as usize);
        std::ptr::copy_nonoverlapping(v.as_ptr(), out, n);
        n as i32
    }))
    .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn basics() {
        assert_eq!(parse(b"0,3-5,12", 20), vec![0, 3, 4, 5, 12]);
        assert_eq!(parse(b"5-3", 20), vec![3, 4, 5]);
        assert_eq!(parse(b"1,1,1", 20), vec![1]);
        assert_eq!(parse(b"", 3), vec![0, 1, 2, 3]);
        assert_eq!(parse(b"xALLx", 2), vec![0, 1, 2]);
        assert_eq!(parse(b"-1", 2), vec![0, 1, 2]);
    }
    #[test]
    fn clamps_and_rejects() {
        assert_eq!(parse(b"2-100", 4), vec![2, 3, 4]);
        assert_eq!(parse(b"9", 4), Vec::<i32>::new());
        assert_eq!(parse(b"-5", 9), Vec::<i32>::new());
        assert_eq!(parse(b"3-", 9), Vec::<i32>::new());
        assert_eq!(parse(b",,2,,", 9), vec![2]);
    }
    #[test]
    fn cap_at_max() {
        assert_eq!(parse(b"0-99999", 99999).len(), MAXNBCI);
    }
    #[test]
    fn atoi_clamps_to_i32() {
        assert_eq!(atoi(b"2147483647"), i32::MAX);
        assert_eq!(atoi(b"2147483648"), i32::MAX);
        assert_eq!(atoi(b"4294967297"), i32::MAX); // was 1 with truncation
        assert_eq!(atoi(b"99999999999999999999999"), i32::MAX);
        assert_eq!(atoi(b"-2147483648"), i32::MIN);
        assert_eq!(atoi(b"-4294967296"), i32::MIN); // was 0 with truncation
        assert_eq!(atoi(b"  +12x"), 12);
        // huge single index is out of range, not aliased to a small one
        assert_eq!(parse(b"4294967297", 9), Vec::<i32>::new());
        assert_eq!(parse(b"3-4294967297", 9), vec![3, 4, 5, 6, 7, 8, 9]);
    }
    #[test]
    fn max_index_int_max_no_overflow() {
        // must terminate (MAXNBCI cap) without overflowing `i += 1`
        assert_eq!(parse(b"all", i32::MAX).len(), MAXNBCI);
        let v = parse(b"2147483640-2147483647", i32::MAX);
        assert_eq!(v, (2147483640..=i32::MAX).collect::<Vec<_>>());
        assert_eq!(parse(b"2147483647", i32::MAX), vec![i32::MAX]);
    }
    #[test]
    fn long_list_over_100_chars() {
        let s: Vec<String> = (0..60).map(|i| i.to_string()).collect();
        let joined = s.join(",");
        assert!(joined.len() > 100);
        assert_eq!(parse(joined.as_bytes(), 100), (0..60).collect::<Vec<_>>());
    }
    #[test]
    fn ffi() {
        unsafe {
            let mut o = [0i32; 8];
            let s = std::ffi::CString::new("1-3").unwrap();
            assert_eq!(flare_cil_set(s.as_ptr(), 9, o.as_mut_ptr(), 8), 3);
            assert_eq!(flare_cil_set(std::ptr::null(), 9, o.as_mut_ptr(), 8), 0);
            assert_eq!(flare_cil_set(s.as_ptr(), 9, std::ptr::null_mut(), 8), 0);
        }
    }
}
