//! Port of `common/flash/FDTButtons.cpp`: the SWF button tags (`DefineButton`,
//! `DefineButton2`, `DefineButtonCxform`, `DefineButtonSound`) and their button records.
//!
//! The classes (`FButtonRecord1/2`, `FButtonRecordList`, `FDTDefineButton`, `FDTDefineButton2`,
//! `FDTDefineButtonCXForm`, `FDTDefineButtonSound`) stay in `FDTButtons.h` with inline members:
//! they own `FMatrix`, `FACXForm`, `FActionRecord`, `FActionCondition`, `FCXForm` and
//! `FSoundInfo` objects and call their (virtual, still C++) `WriteToSWFStream`, and they use
//! the character-ID counter. The fixed parts of the byte layout moved here, all on the
//! destination `FSWFStream` handle ([`crate::fswfstream`]):
//!
//! * [`flare_fdt_button_record1_head`] / [`flare_fdt_button_record2_head`]: the four reserved
//!   bits, the four state flags, character id and layer of a button record (record 2 starts
//!   with a `FlushBits`; the matrix, and for record 2 the colour transform, follow from C++);
//! * [`flare_fdt_button_head`]: character id (and the menu flag byte of `DefineButton2`);
//! * [`flare_fdt_button2_records`]: the `DefineButton2` record block: terminating zero byte,
//!   the offset to the action conditions (0 without conditions, else record bytes + 2), then
//!   the records;
//! * [`flare_fdt_button_finish`]: wrap the body as the right tag.
//!
//! Quirk kept: `FDTDefineButton` (flash 1.0 button) emits tag 34, `stagDefineButton2`, not
//! `stagDefineButton` (7), with the flash 1.0 body layout; its character id is allocated from
//! the shared counter by the *record* constructor too (`FButtonRecord1`), as before (header).

use crate::fswfstream::Stream;
use std::os::raw::c_void;

const STAG_DEFINE_BUTTON_SOUND: u16 = 17;
const STAG_DEFINE_BUTTON_CXFORM: u16 = 23;
const STAG_DEFINE_BUTTON2: u16 = 34;

/// `flare_fdt_button_finish` kinds.
pub const KIND_BUTTON: i32 = 1;
pub const KIND_BUTTON2: i32 = 2;
pub const KIND_CXFORM: i32 = 3;
pub const KIND_SOUND: i32 = 4;

type H = *mut Stream;

unsafe fn on(h: *mut c_void, f: impl FnOnce(&mut Stream)) -> i32 {
    if h.is_null() {
        return 1;
    }
    crate::guard(2, || {
        f(&mut *(h as H));
        0
    })
}

fn record_head(s: &mut Stream, hit: u32, down: u32, over: u32, up: u32, character_id: u32, layer: u32) {
    s.write_bits(0, 4);
    s.write_bits(hit, 1);
    s.write_bits(down, 1);
    s.write_bits(over, 1);
    s.write_bits(up, 1);
    s.write_word(character_id);
    s.write_word(layer);
}

/// `FButtonRecord1::WriteToSWFStream` up to (not including) the matrix.
///
/// # Safety
/// `h` NULL or a live `FSWFStream` handle.
#[no_mangle]
pub unsafe extern "C" fn flare_fdt_button_record1_head(h: *mut c_void, hit: u32, down: u32, over: u32, up: u32, character_id: u32, layer: u32) -> i32 {
    on(h, |s| record_head(s, hit, down, over, up, character_id, layer))
}

/// `FButtonRecord2::WriteToSWFStream` up to (not including) the matrix: like record 1 but
/// the partial byte left by the previous record is flushed first.
///
/// # Safety
/// `h` NULL or a live `FSWFStream` handle.
#[no_mangle]
pub unsafe extern "C" fn flare_fdt_button_record2_head(h: *mut c_void, hit: u32, down: u32, over: u32, up: u32, character_id: u32, layer: u32) -> i32 {
    on(h, |s| {
        s.flush_bits();
        record_head(s, hit, down, over, up, character_id, layer)
    })
}

/// The start of every button tag body: the character id word, then, when `menu_flag >= 0`,
/// the `DefineButton2` menu flag byte.
///
/// # Safety
/// `body` NULL or a live handle.
#[no_mangle]
pub unsafe extern "C" fn flare_fdt_button_head(body: *mut c_void, id: u32, menu_flag: i32) -> i32 {
    on(body, |s| {
        s.write_word(id);
        if menu_flag >= 0 {
            s.write_byte(menu_flag as u32);
        }
    })
}

/// The `DefineButton2` record block: writes the terminating zero byte into `records`, then
/// into `body` the offset word (`records.Size() + 2` when `has_conditions != 0`, else 0) and
/// the whole `records` stream (appended).
///
/// # Safety
/// `body` and `records` NULL or distinct live handles.
#[no_mangle]
pub unsafe extern "C" fn flare_fdt_button2_records(body: *mut c_void, records: *mut c_void, has_conditions: i32) -> i32 {
    if body.is_null() || records.is_null() || body == records {
        return 1;
    }
    crate::guard(2, || {
        let r = &mut *(records as H);
        r.write_byte(0);
        let offset = if has_conditions != 0 { r.size().wrapping_add(2) } else { 0 };
        let b = &mut *(body as H);
        b.write_word(offset);
        b.append(r);
        0
    })
}

/// Wrap `body` as the button tag of `kind` ([`KIND_BUTTON`] adds the action list's terminating
/// zero byte first and is tag 34, see the module note).
///
/// # Safety
/// `out` and `body` NULL or distinct live handles.
#[no_mangle]
pub unsafe extern "C" fn flare_fdt_button_finish(out: *mut c_void, kind: i32, body: *mut c_void) -> i32 {
    let (tag, zero) = match kind {
        KIND_BUTTON => (STAG_DEFINE_BUTTON2, 1),
        KIND_BUTTON2 => (STAG_DEFINE_BUTTON2, 0),
        KIND_CXFORM => (STAG_DEFINE_BUTTON_CXFORM, 0),
        KIND_SOUND => (STAG_DEFINE_BUTTON_SOUND, 0),
        _ => return 1,
    };
    crate::fdt::flare_fdt_finish_tag(out, tag, body, 0, zero)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bytes(s: &mut Stream) -> Vec<u8> {
        let mut v = vec![0u8; s.full_size() as usize];
        assert!(s.write_to_memory(&mut v));
        v
    }

    fn p(s: &mut Stream) -> *mut c_void {
        s as *mut Stream as *mut c_void
    }

    fn unhex(s: &str) -> Vec<u8> {
        if s == "-" {
            return vec![];
        }
        (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap()).collect()
    }

    fn put(s: &mut Stream, data: &[u8]) {
        for &b in data {
            s.write_bits(b as u32, 8);
        }
    }

    /// A colour transform recorded as `<flushed hex>:<partial bits>`: the whole bytes, then the
    /// top `partial` bits of the last byte (the original leaves them pending in the stream).
    fn put_cx(s: &mut Stream, spec: &str) {
        let (hex, partial) = spec.split_once(':').unwrap();
        let b = unhex(hex);
        let partial: u32 = partial.parse().unwrap();
        if partial == 0 {
            put(s, &b);
        } else {
            put(s, &b[..b.len() - 1]);
            s.write_bits((b[b.len() - 1] >> (8 - partial)) as u32, partial);
        }
    }

    #[test]
    fn record_heads_differ_in_the_leading_flush() {
        let mut a = Stream::new();
        let mut b = Stream::new();
        a.write_bits(1, 3);
        b.write_bits(1, 3);
        unsafe {
            assert_eq!(flare_fdt_button_record1_head(p(&mut a), 1, 0, 1, 0, 0x0102, 3), 0);
            assert_eq!(flare_fdt_button_record2_head(p(&mut b), 1, 0, 1, 0, 0x0102, 3), 0);
            assert_eq!(flare_fdt_button_record1_head(std::ptr::null_mut(), 1, 0, 1, 0, 0, 0), 1);
        }
        // record 1 joins the pending 3 bits: 001 0000 1 | 010 (flushed by the word write)
        assert_eq!(bytes(&mut a), vec![0b0010_0001, 0b0100_0000, 2, 1, 3, 0]);
        // record 2 flushes first: 001 00000 | 0000 1010
        assert_eq!(bytes(&mut b), vec![0b0010_0000, 0b0000_1010, 2, 1, 3, 0]);
    }

    #[test]
    fn button2_record_block_and_finish() {
        let mut body = Stream::new();
        let mut recs = Stream::new();
        put(&mut recs, &[9, 8, 7]);
        let mut recs0 = Stream::new();
        unsafe {
            assert_eq!(flare_fdt_button_head(p(&mut body), 5, 1), 0);
            assert_eq!(flare_fdt_button2_records(p(&mut body), p(&mut recs), 1), 0);
            assert_eq!(flare_fdt_button2_records(p(&mut body), p(&mut recs0), 0), 0);
            assert_eq!(flare_fdt_button2_records(p(&mut body), p(&mut body), 0), 1);
        }
        let mut out = Stream::new();
        unsafe {
            assert_eq!(flare_fdt_button_finish(p(&mut out), KIND_BUTTON2, p(&mut body)), 0);
            assert_eq!(flare_fdt_button_finish(p(&mut out), 99, p(&mut body)), 1);
        }
        // id, menu flag, offset = 3 + 1 + 2, records + 0, offset 0, lone 0
        let want_body = vec![5, 0, 1, 6, 0, 9, 8, 7, 0, 0, 0, 0];
        let mut want = vec![(34u16 << 6 | want_body.len() as u16) as u8, (34u16 << 6 >> 8) as u8];
        want.extend(want_body);
        assert_eq!(bytes(&mut out), want);
    }

    #[test]
    fn finish_kinds_pick_the_tags() {
        for (kind, tag, extra) in [(KIND_BUTTON, 34u16, 1usize), (KIND_CXFORM, 23, 0), (KIND_SOUND, 17, 0)] {
            let mut out = Stream::new();
            let mut body = Stream::new();
            body.write_byte(0x55);
            unsafe { assert_eq!(flare_fdt_button_finish(p(&mut out), kind, p(&mut body)), 0) };
            let b = bytes(&mut out);
            assert_eq!(u16::from_le_bytes([b[0], b[1]]) >> 6, tag);
            assert_eq!(b.len(), 3 + extra);
        }
    }

    // Replays the tags recorded from the original C++ (swfwriter_cases.txt). The matrices of
    // the records are rebuilt from their constructor arguments; the colour transforms,
    // action records, conditions and sound infos (virtual C++ objects) are recorded as the
    // bytes they wrote.
    #[test]
    fn replay_recorded_cpp_cases() {
        fn matrix(h: *mut c_void, spec: &str) {
            use crate::fprimitive::{flare_fmatrix_write, matrix_min_bits};
            if spec == "id" {
                unsafe { assert_eq!(flare_fmatrix_write(h, 1, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0), 0) };
                return;
            }
            let v: Vec<i32> = spec.split(',').map(|x| x.parse().unwrap()).collect();
            let (nsb, nrb, ntb) = (matrix_min_bits(v[1], v[2]), matrix_min_bits(v[4], v[5]), matrix_min_bits(v[6], v[7]));
            unsafe { assert_eq!(flare_fmatrix_write(h, 0, v[0] as u32, nsb, v[1], v[2], v[3] as u32, nrb, v[4], v[5], ntb, v[6], v[7]), 0) };
        }
        let mut counts = std::collections::BTreeMap::<&str, usize>::new();
        for line in include_str!("swfwriter_cases.txt").lines() {
            let f: Vec<&str> = line.split('|').collect();
            let mut out = Stream::new();
            match f[0] {
                // button|id|records(hit,down,over,up,layer,charid,matrix spec ; ...)|actions(hex;...)|expected
                "button" => {
                    let id: u32 = f[1].parse().unwrap();
                    let mut body = Stream::new();
                    unsafe { assert_eq!(flare_fdt_button_head(p(&mut body), id, -1), 0) };
                    if f[2] != "-" {
                        for r in f[2].split(';') {
                            let (head, m) = r.split_once('/').unwrap();
                            let v: Vec<u32> = head.split(',').map(|x| x.parse().unwrap()).collect();
                            unsafe { assert_eq!(flare_fdt_button_record1_head(p(&mut body), v[0], v[1], v[2], v[3], v[5], v[4]), 0) };
                            matrix(p(&mut body), m);
                        }
                    }
                    body.write_byte(0);
                    if f[3] != "-" {
                        for a in f[3].split(';') {
                            put(&mut body, &unhex(a));
                        }
                    }
                    // the action end flag is written by the finish (KIND_BUTTON)
                    unsafe { assert_eq!(flare_fdt_button_finish(p(&mut out), KIND_BUTTON, p(&mut body)), 0) };
                    assert_eq!(bytes(&mut out), unhex(f[4]), "{line}");
                }
                // button2|id|menu|records(hit,down,over,up,layer,charid/matrix/cxformhex ; ...)|conditions(hex;...)|expected
                "button2" => {
                    let (id, menu): (u32, i32) = (f[1].parse().unwrap(), f[2].parse().unwrap());
                    let mut body = Stream::new();
                    let mut recs = Stream::new();
                    if f[3] != "-" {
                        for r in f[3].split(';') {
                            let parts: Vec<&str> = r.split('/').collect();
                            let v: Vec<u32> = parts[0].split(',').map(|x| x.parse().unwrap()).collect();
                            unsafe { assert_eq!(flare_fdt_button_record2_head(p(&mut recs), v[0], v[1], v[2], v[3], v[5], v[4]), 0) };
                            matrix(p(&mut recs), parts[1]);
                            recs.flush_bits();
                            put_cx(&mut recs, parts[2]);
                        }
                    }
                    let conds: Vec<&str> = if f[4] == "-" { vec![] } else { f[4].split(';').collect() };
                    unsafe {
                        assert_eq!(flare_fdt_button_head(p(&mut body), id, menu), 0);
                        assert_eq!(flare_fdt_button2_records(p(&mut body), p(&mut recs), !conds.is_empty() as i32), 0);
                    }
                    for c in &conds {
                        put(&mut body, &unhex(c));
                    }
                    unsafe { assert_eq!(flare_fdt_button_finish(p(&mut out), KIND_BUTTON2, p(&mut body)), 0) };
                    assert_eq!(bytes(&mut out), unhex(f[5]), "{line}");
                }
                // buttoncx|id|cxform(hex:partial bits)|expected
                "buttoncx" => {
                    let mut body = Stream::new();
                    unsafe { assert_eq!(flare_fdt_button_head(p(&mut body), f[1].parse().unwrap(), -1), 0) };
                    put_cx(&mut body, f[2]);
                    unsafe { assert_eq!(flare_fdt_button_finish(p(&mut out), KIND_CXFORM, p(&mut body)), 0) };
                    assert_eq!(bytes(&mut out), unhex(f[3]), "{line}");
                }
                // buttonsound|id|(soundid/infohex);x4|expected
                "buttonsound" => {
                    let mut body = Stream::new();
                    unsafe { assert_eq!(flare_fdt_button_head(p(&mut body), f[1].parse().unwrap(), -1), 0) };
                    for part in f[2].split(';') {
                        let (sid, info) = part.split_once('/').unwrap();
                        body.write_word(sid.parse().unwrap());
                        put(&mut body, &unhex(info));
                    }
                    unsafe { assert_eq!(flare_fdt_button_finish(p(&mut out), KIND_SOUND, p(&mut body)), 0) };
                    assert_eq!(bytes(&mut out), unhex(f[3]), "{line}");
                }
                _ => continue,
            }
            *counts.entry(f[0]).or_default() += 1;
        }
        assert_eq!(counts.get("button"), Some(&BUTTON_CASES));
        assert_eq!(counts.get("button2"), Some(&BUTTON2_CASES));
        assert_eq!(counts.get("buttoncx"), Some(&CX_CASES));
        assert_eq!(counts.get("buttonsound"), Some(&SOUND_CASES));
    }

    const BUTTON_CASES: usize = 24;
    const BUTTON2_CASES: usize = 32;
    const CX_CASES: usize = 12;
    const SOUND_CASES: usize = 12;
}
