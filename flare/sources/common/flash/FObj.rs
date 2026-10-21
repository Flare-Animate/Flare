//! Port of `common/flash/FObj.cpp`: the numeric/serialising parts of `FObjCollection`.
//!
//! `FObj`/`FFragment`/`FObjCollection` are pointer-linked C++ object graphs with virtual
//! `WriteToSWFStream`, so the classes (list of `FObj*`, `AddFObj`, `InsertFObj`, the
//! `CreateMovie` orchestration) stay inline in `FObj.h`. What moved here:
//!
//! * `WriteFileHeader`: the 21 byte `FWS` header that `CreateMovie` patches over the 21 zero
//!   bytes it reserved at the front of the stream (the movie rectangle is always written with
//!   16-bit fields so the header has a fixed size) -> [`flare_fobj_file_header`];
//! * the static `characterID` counter behind `FObjCollection::Increment()` and the reset done
//!   at the end of `CreateMovie`/`CreateSprite` -> [`flare_fobj_next_character_id`] /
//!   [`flare_fobj_reset_character_id`].
//!
//! Deviation: the counter is an atomic (the C++ static was not thread safe); `Increment()`
//! wraps to 0 after 65535 like the C++ `U16` (its `assert(characterID != 0xffff)` only fired in
//! debug builds; the header keeps an equivalent `assert`).

use crate::fswfstream::Stream;
use std::sync::atomic::{AtomicU16, Ordering};

pub const HEADER_SIZE: usize = 21;

static CHARACTER_ID: AtomicU16 = AtomicU16::new(0);

/// The 21 byte header of `FObjCollection::WriteFileHeader`.
pub fn file_header(file_length_no_header: u32, camera: [i32; 4], frame_rate: u32, version: u8, num_frames: u32) -> [u8; HEADER_SIZE] {
    let mut header = Stream::new();
    header.write_byte(b'F' as u32);
    header.write_byte(b'W' as u32);
    header.write_byte(b'S' as u32);
    header.write_byte(version as u32);

    header.write_dword(file_length_no_header);

    // The movie dimensions "by hand" so they have the bit size that fills the 21 byte header.
    let n_bits = 16;
    header.write_bits(n_bits, 5);
    header.write_bits(camera[0] as u32, n_bits); // Xmin
    header.write_bits(camera[1] as u32, n_bits); // Xmax
    header.write_bits(camera[2] as u32, n_bits); // Ymin
    header.write_bits(camera[3] as u32, n_bits); // Ymax
    header.flush_bits();

    // The frame rate is 8.8 - we currently support 8.0
    header.write_byte(0);
    header.write_byte(frame_rate);

    header.write_word(num_frames);

    let mut out = [0u8; HEADER_SIZE];
    let mut full = vec![0u8; header.full_size() as usize];
    header.write_to_memory(&mut full);
    let n = full.len().min(HEADER_SIZE);
    out[..n].copy_from_slice(&full[..n]);
    out
}

/// Writes the header over the first 21 bytes of `target` (`camera` = xmin, xmax, ymin, ymax).
/// Returns 0, 1 for a null `target` or `target_len < 21`, 2 if a panic was caught.
///
/// # Safety
/// `target` must be valid for `target_len` bytes.
#[no_mangle]
pub unsafe extern "C" fn flare_fobj_file_header(
    target: *mut u8,
    target_len: usize,
    file_length_no_header: u32,
    xmin: i32,
    xmax: i32,
    ymin: i32,
    ymax: i32,
    frame_rate: u32,
    version: u8,
    num_frames: u32,
) -> i32 {
    if target.is_null() || target_len < HEADER_SIZE {
        return 1;
    }
    crate::guard(2, || {
        let h = file_header(file_length_no_header, [xmin, xmax, ymin, ymax], frame_rate, version, num_frames);
        std::slice::from_raw_parts_mut(target, HEADER_SIZE).copy_from_slice(&h);
        0
    })
}

/// `++characterID` (the new value; wraps to 0 after 65535).
#[no_mangle]
pub extern "C" fn flare_fobj_next_character_id() -> u16 {
    CHARACTER_ID.fetch_add(1, Ordering::SeqCst).wrapping_add(1)
}

/// `characterID = 0`.
#[no_mangle]
pub extern "C" fn flare_fobj_reset_character_id() {
    CHARACTER_ID.store(0, Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_layout() {
        let h = file_header(0x0102_0304, [0, 12800, 0, 9600], 12, 6, 3);
        assert_eq!(&h[..4], b"FWS\x06");
        assert_eq!(&h[4..8], &[4, 3, 2, 1]);
        let mut bits = String::from("10000"); // nBits = 16, then Xmin, Xmax, Ymin, Ymax
        for v in [0u32, 12800, 0, 9600] {
            bits += &format!("{v:016b}");
        }
        while bits.len() % 8 != 0 {
            bits.push('0');
        }
        let exp: Vec<u8> = bits.as_bytes().chunks(8).map(|c| u8::from_str_radix(std::str::from_utf8(c).unwrap(), 2).unwrap()).collect();
        assert_eq!(&h[8..17], &exp[..]);
        assert_eq!(&h[17..], &[0, 12, 3, 0]);
    }

    #[test]
    fn negative_coordinates_keep_the_low_16_bits() {
        let h = file_header(0, [-1, 0, 0, 0], 0, 0, 0);
        assert_eq!(h[8] >> 3, 16);
        assert_eq!(h[8] & 7, 0b111); // top bits of 0xffff
    }

    #[test]
    fn ffi_checks_and_counter() {
        let mut buf = [0xAAu8; 25];
        unsafe {
            assert_eq!(flare_fobj_file_header(std::ptr::null_mut(), 25, 0, 0, 0, 0, 0, 0, 0, 0), 1);
            assert_eq!(flare_fobj_file_header(buf.as_mut_ptr(), 20, 0, 0, 0, 0, 0, 0, 0, 0), 1);
            assert_eq!(flare_fobj_file_header(buf.as_mut_ptr(), 25, 7, 0, 1, 0, 1, 12, 6, 1), 0);
        }
        assert_eq!(&buf[..4], b"FWS\x06");
        assert_eq!(&buf[21..], &[0xAA; 4]); // only 21 bytes touched
        flare_fobj_reset_character_id();
        assert_eq!(flare_fobj_next_character_id(), 1);
        assert_eq!(flare_fobj_next_character_id(), 2);
        flare_fobj_reset_character_id();
        assert_eq!(flare_fobj_next_character_id(), 1);
    }

    // Replays the movies recorded from the original C++: the 21 byte header patched over a
    // zero-reserved stream with the recorded body and end tag must give the recorded bytes.
    #[test]
    fn replay_recorded_cpp_movies() {
        let mut n = 0;
        for line in include_str!("swfwriter_cases.txt").lines().filter(|l| l.starts_with("fobjmovie|")) {
            let f: Vec<&str> = line.split('|').collect();
            let num = |i: usize| f[i].parse::<i64>().unwrap();
            let body = if f[8] == "-" { vec![] } else { unhex(f[8]) };
            let expect = unhex(f[9]);
            let mut s = Stream::new();
            for _ in 0..HEADER_SIZE {
                s.write_byte(0);
            }
            for &b in &body {
                s.write_byte(b as u32);
            }
            s.append_tag(0, 0, None); // WriteEndTag
            let total = s.size();
            let h = file_header(total, [num(1) as i32, num(2) as i32, num(3) as i32, num(4) as i32], num(5) as u32, num(6) as u8, num(7) as u32);
            let mut v = vec![0u8; s.full_size() as usize];
            assert!(s.write_to_memory(&mut v));
            v[..HEADER_SIZE].copy_from_slice(&h);
            assert_eq!(v, expect, "{line}");
            n += 1;
        }
        assert_eq!(n, 40);
    }

    fn unhex(s: &str) -> Vec<u8> {
        (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap()).collect()
    }
}
