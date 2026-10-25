//! Port of `flarelib/namebuilder.cpp`: the arithmetic of `NameCreator::getNext` (the level
//! name sequence A, B, ... Z, AA, AB, ...) and the name-suffix parse of the `NameModifier`
//! constructor (`name_3` -> base `name`, index 3).
//!
//! The classes stay in `namebuilder.h` (virtual dispatch, `std::wstring` ownership,
//! `FlexibleNameCreator` derives from `NameCreator`); only the digit arithmetic moved.
//! Names cross the C ABI as UTF-32 code units (`uint32_t`, widened from `wchar_t` by the
//! header) or, for the generated letters, as ASCII bytes.

/// One step of `NameCreator::getNext`.
///
/// `state` holds the base-26 digits, least significant first (`m_s`). The caller passes a
/// buffer of `cap >= n + 1` ints with the old state in the first `n`; on success the new
/// state is in `state[..new_n]`, the letters (most significant first) in `name[..name_len]`
/// (needs `name_cap >= n + 1`) and the function returns 0. `skip_invalid != 0` is the
/// `_WIN32` branch: device names (AUX, COM, CON, LPT, NUL, PRN) are skipped.
/// Returns 1 for a bad argument, nothing is written.
///
/// # Safety
/// Pointers must be valid for the stated lengths (checked for null / length only).
#[no_mangle]
pub unsafe extern "C" fn flare_namecreator_next(
    state: *mut i32,
    n: usize,
    cap: usize,
    new_n: *mut usize,
    name: *mut u8,
    name_cap: usize,
    name_len: *mut usize,
    skip_invalid: i32,
) -> i32 {
    std::panic::catch_unwind(|| {
        if state.is_null() || new_n.is_null() || name.is_null() || name_len.is_null() {
            return 1;
        }
        if n > (1 << 24) || cap < n + 1 || name_cap < n + 1 {
            return 1;
        }
        let st = std::slice::from_raw_parts_mut(state, cap);
        let out = std::slice::from_raw_parts_mut(name, name_cap);
        let mut len = n;
        loop {
            if len == 0 {
                st[0] = 0;
                len = 1;
                out[0] = b'A';
                *new_n = len;
                *name_len = 1;
                return 0;
            }
            let mut i = 0usize;
            while i < len {
                st[i] = st[i].wrapping_add(1);
                if st[i] <= 25 {
                    break;
                }
                st[i] = 0;
                i += 1;
            }
            if i >= len {
                if len + 1 > cap || len + 1 > name_cap {
                    return 1;
                }
                st[len] = 0;
                len += 1;
            }
            // letters, most significant first; out of range digits wrap like (wchar_t)
            let mut k = 0usize;
            for d in (0..len).rev() {
                out[k] = (b'A' as i32).wrapping_add(st[d]) as u8;
                k += 1;
            }
            let s = &out[..k];
            if skip_invalid != 0
                && [&b"AUX"[..], b"COM", b"CON", b"LPT", b"NUL", b"PRN"].contains(&s)
            {
                continue;
            }
            *new_n = len;
            *name_len = k;
            return 0;
        }
    })
    .unwrap_or(1)
}

/// The `NameModifier` constructor: split `name` into base and numeric suffix.
///
/// If the part after the last non-digit is a non-empty digit run preceded by `_`, the base
/// is `name[..j]` (`*base_len = j`) and `*index` the number; otherwise the base is the whole
/// name and `*index` 0. Returns 0, 1 for a bad argument, 2 when the digits do not fit an
/// `int` (the original `std::stoi` threw `std::out_of_range`).
///
/// # Safety
/// `name` must be valid for `len` code units.
#[no_mangle]
pub unsafe extern "C" fn flare_namemodifier_parse(
    name: *const u32,
    len: usize,
    base_len: *mut usize,
    index: *mut i32,
) -> i32 {
    std::panic::catch_unwind(|| {
        if base_len.is_null() || index.is_null() || (name.is_null() && len != 0) {
            return 1;
        }
        let s: &[u32] = if len == 0 { &[] } else { std::slice::from_raw_parts(name, len) };
        *base_len = len;
        *index = 0;
        let is_digit = |c: u32| (b'0' as u32..=b'9' as u32).contains(&c);
        let Some(j) = s.iter().rposition(|&c| !is_digit(c)) else {
            return 0;
        };
        if j + 1 < len && s[j] == b'_' as u32 {
            let mut v: i64 = 0;
            for &c in &s[j + 1..] {
                v = v * 10 + (c - b'0' as u32) as i64;
                if v > i32::MAX as i64 {
                    return 2;
                }
            }
            *index = v as i32;
            *base_len = j;
        }
        0
    })
    .unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn next(st: &mut Vec<i32>, skip: i32) -> String {
        let n = st.len();
        st.resize(n + 1, 0);
        let mut nn = 0usize;
        let mut buf = vec![0u8; n + 2];
        let mut l = 0usize;
        let rc = unsafe {
            flare_namecreator_next(st.as_mut_ptr(), n, n + 1, &mut nn, buf.as_mut_ptr(), buf.len(), &mut l, skip)
        };
        assert_eq!(rc, 0);
        st.truncate(nn);
        String::from_utf8(buf[..l].to_vec()).unwrap()
    }

    fn parse(s: &str) -> (usize, i32, i32) {
        let u: Vec<u32> = s.chars().map(|c| c as u32).collect();
        let (mut b, mut i) = (0usize, 0i32);
        let rc = unsafe { flare_namemodifier_parse(u.as_ptr(), u.len(), &mut b, &mut i) };
        (b, i, rc)
    }

    #[test]
    fn sequence() {
        let mut st = vec![];
        let v: Vec<String> = (0..30).map(|_| next(&mut st, 0)).collect();
        assert_eq!(v[0], "A");
        assert_eq!(v[25], "Z");
        assert_eq!(v[26], "AA");
        assert_eq!(v[27], "AB");
        assert_eq!(v[29], "AD");
        let mut st = vec![];
        let mut last = String::new();
        for _ in 0..(26 + 26 * 26) {
            last = next(&mut st, 0);
        }
        assert_eq!(last, "ZZ");
        assert_eq!(next(&mut st, 0), "AAA");
    }

    #[test]
    fn windows_skip() {
        let mut st = vec![];
        let mut seen = vec![];
        for _ in 0..(26 + 26 * 26 + 26 * 26 * 26) {
            seen.push(next(&mut st, 1));
        }
        for bad in ["AUX", "COM", "CON", "LPT", "NUL", "PRN"] {
            assert!(!seen.iter().any(|s| s == bad), "{bad}");
        }
        assert!(seen.iter().any(|s| s == "AUY"));
    }

    #[test]
    fn modifier_parse() {
        assert_eq!(parse(""), (0, 0, 0));
        assert_eq!(parse("pippo"), (5, 0, 0));
        assert_eq!(parse("pippo_3"), (5, 3, 0));
        assert_eq!(parse("pippo_"), (6, 0, 0));
        assert_eq!(parse("pippo3"), (6, 0, 0));
        assert_eq!(parse("123"), (3, 0, 0));
        assert_eq!(parse("a_0012"), (1, 12, 0));
        assert_eq!(parse("a_2147483647"), (1, i32::MAX, 0));
        assert_eq!(parse("a_2147483648").2, 2);
        assert_eq!(unsafe { flare_namemodifier_parse(std::ptr::null(), 1, &mut 0, &mut 0) }, 1);
    }

    /// Replays `tests/native/gen_namebuilder_cases.py`: the original `namebuilder.cpp` built
    /// with g++ (non-Windows branch), 1500 creator names and the modifier parse/sequence.
    #[test]
    fn matches_recorded_cpp() {
        let golden = include_str!("namebuilder_cases.txt");
        let mut st = vec![];
        let (mut cn, mut md) = (0, 0);
        for line in golden.lines() {
            let f: Vec<&str> = line.split(' ').collect();
            match f[0] {
                "cn" => {
                    assert_eq!(next(&mut st, 0), f[2], "{line}");
                    cn += 1;
                }
                "mod" => {
                    let (b, i, rc) = parse(f[1]);
                    if f[2] == "throw" {
                        assert_eq!(rc, 2, "{line}");
                    } else {
                        assert_eq!(rc, 0, "{line}");
                        let base = &f[1][..b];
                        for k in 0..4 {
                            let idx = i.wrapping_add(k);
                            let want = if idx < 1 { base.to_string() } else { format!("{base}_{idx}") };
                            assert_eq!(f[2 + k as usize], want, "{line}");
                        }
                    }
                    md += 1;
                }
                _ => panic!("{line}"),
            }
        }
        assert_eq!((cn, md), (1500, 15));
    }
}
