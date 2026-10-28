//! Port of `common/flash/FAction.cpp`: the SWF action-record writers.
//!
//! The action classes stay in `FAction.h` with inline member definitions: `FActionCondition`,
//! `FClipAction`, `FClipActionRecord` and `FActionConstantPool` own lists of virtual
//! `FActionRecord`/`FString` objects, and the records own their `FString` operands. What moved
//! here is the byte layout, behind `flare_faction_*` / `flare_fclip_*`, each taking the
//! destination `FSWFStream` handle ([`crate::fswfstream`]) and null-checked/`catch_unwind`ed:
//!
//! * the fixed-size records (`GotoFrame`, `GotoFrame2`, `GetURL2`, `If`, `Jump`,
//!   `WaitForFrame`, `WaitForFrame2`), the `FActionUnparsed` copy, and the heads of the
//!   string-carrying records (`GetURL` wrap-up, `GotoLabel`, `SetTarget`);
//! * `FActionPush` whole (type switch, sizes, float/`double` operands, string operand);
//! * `FActionCondition`: condition-flag bit table and both write forms; the constant pool
//!   wrapper; `FClipAction` and `FClipActionRecord` wrappers (event flags, key code, size).
//!
//! Quirks kept on purpose:
//! * `FActionPush` type 1/7 writes `(U32)number` of a `float`: converted like x86-64 does
//!   (through a 64-bit integer, keeping the low 32 bits), so negative values wrap;
//! * `FActionPush` type 0 computes the operand size as `1 + Length() + 1` in 16 bits, with
//!   `Length()` truncated to 16 bits, while the string itself is written in full;
//! * `FActionPush` type 6 (`double`): the original wrote the *address bytes* of the operand
//!   (`WriteByte((U32)(bPtr + n))`) instead of its bytes -- an obvious bug that makes the output
//!   depend on where the object lives. The header still computes those eight values the same way
//!   and hands them to [`flare_faction_push`] as `double_bytes`; the golden masks them;
//! * unknown push types write `size == 0`, the type byte and nothing else;
//! * `FActionCondition::AddKeyCode` shifts the key into the 16-bit flag word (`keyCode << 9`),
//!   dropping the high bits; unknown conditions leave the flags unchanged.
//!
//! Deviation: a type 0 push without a string (a NULL dereference in C++) is refused (status 1).

use crate::fswfstream::Stream;
use std::os::raw::c_void;

const SACTION_NONE: u32 = 0x00;
const SACTION_GOTO_FRAME: u32 = 0x81;
const SACTION_GET_URL: u32 = 0x83;
const SACTION_CONSTANT_POOL: u32 = 0x88;
const SACTION_WAIT_FOR_FRAME: u32 = 0x8A;
const SACTION_SET_TARGET: u32 = 0x8B;
const SACTION_GOTO_LABEL: u32 = 0x8C;
const SACTION_WAIT_FOR_FRAME2: u32 = 0x8D;
const SACTION_PUSH: u32 = 0x96;
const SACTION_JUMP: u32 = 0x99;
const SACTION_GET_URL2: u32 = 0x9A;
const SACTION_IF: u32 = 0x9D;
const SACTION_GOTO_FRAME2: u32 = 0x9F;

const CLIP_EVENT_KEY_PRESS: u32 = 0x0002_0000;

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

unsafe fn two(a: *mut c_void, b: *mut c_void, f: impl FnOnce(&mut Stream, &mut Stream)) -> i32 {
    if a.is_null() || b.is_null() || a == b {
        return 1;
    }
    crate::guard(2, || {
        f(&mut *(a as H), &mut *(b as H));
        0
    })
}

/// `FActionUnparsed::WriteToSWFStream`: the code, then (code >= 0x80) length and data.
///
/// # Safety
/// `out` NULL or a live `FSWFStream` handle; `data` valid for `length` bytes when `code >= 0x80`
/// and `length > 0`.
#[no_mangle]
pub unsafe extern "C" fn flare_faction_unparsed(out: *mut c_void, code: u8, length: u16, data: *const u8) -> i32 {
    if code >= 0x80 && length > 0 && data.is_null() {
        return 1;
    }
    on(out, |s| {
        s.write_byte(code as u32);
        if code >= 0x80 {
            s.write_word(length as u32);
            for i in 0..length as usize {
                s.write_byte(*data.add(i) as u32);
            }
        }
    })
}

/// `FActionGetURL::WriteToSWFStream` once `url` and `window` are written into `body`:
/// the code, `body.Size()`, then `body`.
///
/// # Safety
/// `out`/`body` NULL or live, distinct handles.
#[no_mangle]
pub unsafe extern "C" fn flare_faction_get_url_finish(out: *mut c_void, body: *mut c_void) -> i32 {
    two(out, body, |o, b| {
        o.write_byte(SACTION_GET_URL);
        o.write_word(b.size());
        o.append(b);
    })
}

/// `FActionGetURL2`.
///
/// # Safety
/// As [`flare_faction_unparsed`].
#[no_mangle]
pub unsafe extern "C" fn flare_faction_get_url2(out: *mut c_void, method: u8) -> i32 {
    on(out, |s| {
        s.write_byte(SACTION_GET_URL2);
        s.write_word(1);
        s.write_byte(method as u32);
    })
}

/// `FActionGotoFrame`.
///
/// # Safety
/// As [`flare_faction_unparsed`].
#[no_mangle]
pub unsafe extern "C" fn flare_faction_goto_frame(out: *mut c_void, frame_index: u16) -> i32 {
    on(out, |s| {
        s.write_byte(SACTION_GOTO_FRAME);
        s.write_word(2);
        s.write_word(frame_index as u32);
    })
}

/// `FActionGotoFrame2`.
///
/// # Safety
/// As [`flare_faction_unparsed`].
#[no_mangle]
pub unsafe extern "C" fn flare_faction_goto_frame2(out: *mut c_void, play: u8) -> i32 {
    on(out, |s| {
        s.write_byte(SACTION_GOTO_FRAME2);
        s.write_word(1);
        s.write_byte(play as u32);
    })
}

/// `FActionGotoLabel` head: the code byte; the label follows as a null-terminated `FString`.
///
/// # Safety
/// As [`flare_faction_unparsed`].
#[no_mangle]
pub unsafe extern "C" fn flare_faction_goto_label_head(out: *mut c_void) -> i32 {
    on(out, |s| s.write_byte(SACTION_GOTO_LABEL))
}

/// `FActionIf`.
///
/// # Safety
/// As [`flare_faction_unparsed`].
#[no_mangle]
pub unsafe extern "C" fn flare_faction_if(out: *mut c_void, branch_offset: i16) -> i32 {
    on(out, |s| {
        s.write_byte(SACTION_IF);
        s.write_word(2);
        s.write_word(branch_offset as u32);
    })
}

/// `FActionJump`.
///
/// # Safety
/// As [`flare_faction_unparsed`].
#[no_mangle]
pub unsafe extern "C" fn flare_faction_jump(out: *mut c_void, branch_offset: i16) -> i32 {
    on(out, |s| {
        s.write_byte(SACTION_JUMP);
        s.write_word(2);
        s.write_word(branch_offset as u32);
    })
}

/// `FActionSetTarget` head: the code byte and `Length() + 1`; the name follows as a
/// null-terminated `FString`.
///
/// # Safety
/// As [`flare_faction_unparsed`].
#[no_mangle]
pub unsafe extern "C" fn flare_faction_set_target_head(out: *mut c_void, length_plus_one: u32) -> i32 {
    on(out, |s| {
        s.write_byte(SACTION_SET_TARGET);
        s.write_word(length_plus_one);
    })
}

/// `FActionWaitForFrame`.
///
/// # Safety
/// As [`flare_faction_unparsed`].
#[no_mangle]
pub unsafe extern "C" fn flare_faction_wait_for_frame(out: *mut c_void, frame_index: u16, skip_count: u16) -> i32 {
    on(out, |s| {
        s.write_byte(SACTION_WAIT_FOR_FRAME);
        s.write_word(3);
        s.write_word(frame_index as u32);
        s.write_byte(skip_count as u32);
    })
}

/// `FActionWaitForFrame2`.
///
/// # Safety
/// As [`flare_faction_unparsed`].
#[no_mangle]
pub unsafe extern "C" fn flare_faction_wait_for_frame2(out: *mut c_void, skip_count: u8) -> i32 {
    on(out, |s| {
        s.write_byte(SACTION_WAIT_FOR_FRAME2);
        s.write_word(1);
        s.write_byte(skip_count as u32);
    })
}

/// `(U32)float` as x86-64 compiles it: through a 64-bit integer, low 32 bits kept (NaN and
/// out-of-range values become `0x8000_0000_0000_0000`, whose low half is 0).
fn float_to_u32(x: f32) -> u32 {
    if x.is_nan() || x >= 9.223_372e18 || x < -9.223_372e18 {
        0
    } else {
        (x as i64) as u32
    }
}

/// `FActionPush::WriteToSWFStream`.
///
/// `text`/`text_len` is the type 0 operand (`text_len16` is `FString::Length()`, the 16-bit
/// truncation of `text_len`); `number` the type 1/7 `float`; `value` the type 4/5/8 byte;
/// `double_bytes` the eight bytes of the type 6 operand in the order they are written.
///
/// # Safety
/// `out` NULL or a live `FSWFStream` handle; `text` valid for `text_len` bytes (NULL with
/// `text_len == 0` is an empty string, but a type 0 push needs `has_text != 0`);
/// `double_bytes` NULL or 8 readable bytes.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn flare_faction_push(
    out: *mut c_void,
    kind: u8,
    has_text: i32,
    text: *const u8,
    text_len: usize,
    number: f32,
    value: u8,
    double_bytes: *const u8,
) -> i32 {
    if (kind == 0 && has_text == 0) || (text.is_null() && text_len > 0) || (kind == 6 && double_bytes.is_null()) {
        return 1;
    }
    on(out, |s| {
        s.write_byte(SACTION_PUSH);
        let mut size: u16 = 0;
        let mut temp = Stream::new();
        match kind {
            0 => {
                // the operand is null terminated
                size = 1u16.wrapping_add(text_len as u16).wrapping_add(1);
                for i in 0..text_len {
                    temp.write_byte(*text.add(i) as u32);
                }
                temp.write_byte(0);
            }
            1 | 7 => {
                size = 5;
                temp.write_dword(float_to_u32(number));
            }
            4 | 5 | 8 => {
                size = 2;
                temp.write_byte(value as u32);
            }
            6 => {
                size = 9;
                for i in 0..8 {
                    temp.write_byte(*double_bytes.add(i) as u32);
                }
            }
            _ => {}
        }
        s.write_word(size as u32);
        s.write_byte(kind as u32);
        s.append(&mut temp);
    })
}

/// `FActionCondition::AddCondition`: the flag bit of an `OverDownToIdle`..`IdleToOverUp`
/// condition (1..=9); 0 for anything else.
#[no_mangle]
pub extern "C" fn flare_faction_condition_bit(condition: u16) -> u32 {
    match condition {
        1 => 256,
        2 => 128,
        3 => 64,
        4 => 32,
        5 => 16,
        6 => 8,
        7 => 4,
        8 => 2,
        9 => 1,
        _ => 0,
    }
}

/// `FActionCondition::AddKeyCode`: the 16-bit flag word with `key_code << 9` or-ed in.
#[no_mangle]
pub extern "C" fn flare_faction_condition_key(flags: u16, key_code: u32) -> u16 {
    flags | (key_code << 9) as u16
}

/// `FActionCondition::WriteToSWFStream(stream)` head, written into the temporary `body`.
///
/// # Safety
/// As [`flare_faction_unparsed`].
#[no_mangle]
pub unsafe extern "C" fn flare_faction_condition_head(body: *mut c_void, flags: u16) -> i32 {
    on(body, |s| s.write_word(flags as u32))
}

/// `FActionCondition::WriteToSWFStream(stream)` tail: `2 + body.Size()`, then `body`.
///
/// # Safety
/// `out`/`body` NULL or live, distinct handles.
#[no_mangle]
pub unsafe extern "C" fn flare_faction_condition_finish(out: *mut c_void, body: *mut c_void) -> i32 {
    two(out, body, |o, b| {
        o.write_word(2u32.wrapping_add(b.size()));
        o.append(b);
    })
}

/// `FActionCondition::WriteToSWFStream(stream, lastFlag)` head: offset 0 and the flags.
///
/// # Safety
/// As [`flare_faction_unparsed`].
#[no_mangle]
pub unsafe extern "C" fn flare_faction_condition_last_head(out: *mut c_void, flags: u16) -> i32 {
    on(out, |s| {
        s.write_word(0);
        s.write_word(flags as u32);
    })
}

/// `FActionCondition::WriteToSWFStream(stream, lastFlag)` tail: the terminating zero byte.
///
/// # Safety
/// As [`flare_faction_unparsed`].
#[no_mangle]
pub unsafe extern "C" fn flare_faction_condition_last_tail(out: *mut c_void) -> i32 {
    on(out, |s| s.write_byte(0))
}

/// `FActionConstantPool::WriteToSWFStream` once the strings are in `body`: the code,
/// `body.FullSize() + 2`, the count, then `body`.
///
/// # Safety
/// `out`/`body` NULL or live, distinct handles.
#[no_mangle]
pub unsafe extern "C" fn flare_faction_constant_pool_finish(out: *mut c_void, body: *mut c_void, count: u32) -> i32 {
    two(out, body, |o, b| {
        o.write_byte(SACTION_CONSTANT_POOL);
        o.write_word(b.full_size().wrapping_add(2));
        o.write_word(count);
        o.append(b);
    })
}

/// `FClipAction::WriteToSWFStream` head, written into the temporary `body`: a zero word and the
/// event flags.
///
/// # Safety
/// As [`flare_faction_unparsed`].
#[no_mangle]
pub unsafe extern "C" fn flare_fclip_action_head(body: *mut c_void, event_flags: u32) -> i32 {
    on(body, |s| {
        s.write_word(0);
        s.write_dword(event_flags);
    })
}

/// `FClipAction::WriteToSWFStream` tail: the zero terminator, then `body` appended to `out`.
///
/// # Safety
/// `out`/`body` NULL or live, distinct handles.
#[no_mangle]
pub unsafe extern "C" fn flare_fclip_action_finish(out: *mut c_void, body: *mut c_void) -> i32 {
    two(out, body, |o, b| {
        b.write_dword(0);
        o.append(b);
    })
}

/// `FClipActionRecord::WriteToSWFStream` once the actions are in `actions`: the end byte, then
/// `eventFlags`, the size (`+ 1` and the key code for key-press events) and the actions.
///
/// # Safety
/// `out`/`actions` NULL or live, distinct handles.
#[no_mangle]
pub unsafe extern "C" fn flare_fclip_action_record_finish(out: *mut c_void, actions: *mut c_void, event_flags: u32, key_code: u8) -> i32 {
    two(out, actions, |o, a| {
        a.write_byte(SACTION_NONE);
        let size = a.full_size();
        let mut head = Stream::new();
        head.write_dword(event_flags);
        if event_flags & CLIP_EVENT_KEY_PRESS == CLIP_EVENT_KEY_PRESS {
            head.write_dword(size.wrapping_add(1));
            head.write_byte(key_code as u32);
        } else {
            head.write_dword(size);
        }
        head.append(a);
        o.append(&mut head);
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &mut Stream) -> *mut c_void {
        s as *mut Stream as *mut c_void
    }

    fn bytes(s: &mut Stream) -> Vec<u8> {
        let mut v = vec![0u8; s.full_size() as usize];
        assert!(s.write_to_memory(&mut v));
        v
    }

    fn unhex(s: &str) -> Vec<u8> {
        if s == "-" {
            return vec![];
        }
        (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap()).collect()
    }

    #[test]
    fn null_and_alias_are_refused() {
        unsafe {
            assert_eq!(flare_faction_goto_frame(std::ptr::null_mut(), 1), 1);
            let mut a = Stream::new();
            assert_eq!(flare_faction_get_url_finish(p(&mut a), p(&mut a)), 1);
            assert_eq!(flare_faction_get_url_finish(p(&mut a), std::ptr::null_mut()), 1);
            assert_eq!(flare_faction_unparsed(p(&mut a), 0x90, 3, std::ptr::null()), 1);
            assert_eq!(flare_faction_push(p(&mut a), 0, 0, std::ptr::null(), 0, 0.0, 0, std::ptr::null()), 1);
            assert_eq!(flare_faction_push(p(&mut a), 6, 0, std::ptr::null(), 0, 0.0, 0, std::ptr::null()), 1);
            assert_eq!(flare_faction_push(p(&mut a), 0, 1, std::ptr::null(), 4, 0.0, 0, std::ptr::null()), 1);
        }
    }

    #[test]
    fn fixed_records() {
        let mut s = Stream::new();
        unsafe {
            flare_faction_goto_frame(p(&mut s), 0x0102);
            flare_faction_if(p(&mut s), -2);
            flare_faction_wait_for_frame(p(&mut s), 5, 7);
            flare_faction_unparsed(p(&mut s), 0x07, 99, std::ptr::null());
        }
        assert_eq!(bytes(&mut s), vec![0x81, 2, 0, 2, 1, 0x9d, 2, 0, 0xfe, 0xff, 0x8a, 3, 0, 5, 0, 7, 0x07]);
    }

    #[test]
    fn push_types() {
        let mut s = Stream::new();
        unsafe {
            flare_faction_push(p(&mut s), 0, 1, b"ab".as_ptr(), 2, 0.0, 0, std::ptr::null());
            flare_faction_push(p(&mut s), 1, 0, std::ptr::null(), 0, -1.0, 0, std::ptr::null());
            flare_faction_push(p(&mut s), 5, 0, std::ptr::null(), 0, 0.0, 1, std::ptr::null());
            flare_faction_push(p(&mut s), 3, 0, std::ptr::null(), 0, 0.0, 1, std::ptr::null());
        }
        assert_eq!(
            bytes(&mut s),
            vec![0x96, 4, 0, 0, b'a', b'b', 0, 0x96, 5, 0, 1, 0xff, 0xff, 0xff, 0xff, 0x96, 2, 0, 5, 1, 0x96, 0, 0, 3]
        );
    }

    #[test]
    fn push_double_writes_given_bytes() {
        let mut s = Stream::new();
        let d = [1u8, 2, 3, 4, 5, 6, 7, 8];
        unsafe { flare_faction_push(p(&mut s), 6, 0, std::ptr::null(), 0, 0.0, 0, d.as_ptr()) };
        assert_eq!(bytes(&mut s), vec![0x96, 9, 0, 6, 1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn float_conversion_matches_x86() {
        assert_eq!(float_to_u32(1.5), 1);
        assert_eq!(float_to_u32(-1.0), 0xffff_ffff);
        assert_eq!(float_to_u32(4_294_967_296.0), 0);
        assert_eq!(float_to_u32(f32::NAN), 0);
    }

    #[test]
    fn condition_flags() {
        assert_eq!(flare_faction_condition_bit(1), 256);
        assert_eq!(flare_faction_condition_bit(9), 1);
        assert_eq!(flare_faction_condition_bit(0), 0);
        assert_eq!(flare_faction_condition_bit(10), 0);
        assert_eq!(flare_faction_condition_key(1, 0x41), 1 | (0x41 << 9) as u16);
        assert_eq!(flare_faction_condition_key(0, 0x1ff), (0x1ffu32 << 9) as u16);
    }

    #[test]
    fn clip_action_record_key_press() {
        let mut out = Stream::new();
        let mut act = Stream::new();
        act.write_byte(0xaa);
        unsafe { assert_eq!(flare_fclip_action_record_finish(p(&mut out), p(&mut act), 0x0002_0000, 0x41), 0) };
        // flags, size(2)+1, key, action, end
        assert_eq!(bytes(&mut out), vec![0, 0, 2, 0, 3, 0, 0, 0, 0x41, 0xaa, 0]);
        let mut out = Stream::new();
        let mut act = Stream::new();
        unsafe { assert_eq!(flare_fclip_action_record_finish(p(&mut out), p(&mut act), 0x1, 0x41), 0) };
        assert_eq!(bytes(&mut out), vec![1, 0, 0, 0, 1, 0, 0, 0, 0]);
    }

    // Replays the records recorded from the original C++ (swfshapes_cases.txt). `push|6` lines
    // mask the eight address-derived bytes.
    #[test]
    fn replay_recorded_cpp_cases() {
        let mut counts = std::collections::BTreeMap::<String, usize>::new();
        for line in include_str!("swfshapes_cases.txt").lines() {
            let f: Vec<&str> = line.split('|').collect();
            let mut s = Stream::new();
            let a = |i: usize| -> i64 { f[i].parse().unwrap_or_else(|_| panic!("{line}")) };
            let key = match f[0] {
                "unparsed" => unsafe {
                    let d = unhex(f[3]);
                    assert_eq!(flare_faction_unparsed(p(&mut s), a(1) as u8, a(2) as u16, d.as_ptr()), 0);
                    "unparsed"
                },
                "geturl2" => unsafe {
                    assert_eq!(flare_faction_get_url2(p(&mut s), a(1) as u8), 0);
                    "geturl2"
                },
                "gotoframe" => unsafe {
                    assert_eq!(flare_faction_goto_frame(p(&mut s), a(1) as u16), 0);
                    "gotoframe"
                },
                "gotoframe2" => unsafe {
                    assert_eq!(flare_faction_goto_frame2(p(&mut s), a(1) as u8), 0);
                    "gotoframe2"
                },
                "if" => unsafe {
                    assert_eq!(flare_faction_if(p(&mut s), a(1) as i16), 0);
                    "if"
                },
                "jump" => unsafe {
                    assert_eq!(flare_faction_jump(p(&mut s), a(1) as i16), 0);
                    "jump"
                },
                "waitframe" => unsafe {
                    let v: Vec<u16> = f[1].split(',').map(|x| x.parse().unwrap()).collect();
                    assert_eq!(flare_faction_wait_for_frame(p(&mut s), v[0], v[1]), 0);
                    "waitframe"
                },
                "waitframe2" => unsafe {
                    assert_eq!(flare_faction_wait_for_frame2(p(&mut s), a(1) as u8), 0);
                    "waitframe2"
                },
                "push" => unsafe {
                    // kind|number bits|value|text hex|dbl|bytes
                    let t = unhex(f[4]);
                    let dbl = [0u8; 8];
                    let kind = a(1) as u8;
                    let num = f32::from_bits(a(2) as u32);
                    assert_eq!(
                        flare_faction_push(p(&mut s), kind, (kind == 0) as i32, t.as_ptr(), t.len(), num, a(3) as u8, dbl.as_ptr()),
                        0
                    );
                    if kind == 6 {
                        // eight address-derived bytes: masked on both sides
                        let mut got = bytes(&mut s);
                        let n = got.len();
                        for b in &mut got[n - 8..] {
                            *b = 0;
                        }
                        assert_eq!(got, unhex(f[5]), "{line}");
                        *counts.entry("push".into()).or_default() += 1;
                        continue;
                    }
                    "push"
                },
                "condbit" => {
                    assert_eq!(flare_faction_condition_bit(a(1) as u16) as i64, a(2));
                    *counts.entry("condbit".into()).or_default() += 1;
                    continue;
                }
                "condkey" => {
                    assert_eq!(flare_faction_condition_key(a(1) as u16, a(2) as u32) as i64, a(3));
                    *counts.entry("condkey".into()).or_default() += 1;
                    continue;
                }
                "cliprec" => unsafe {
                    // actions hex|flags|key|bytes
                    let mut act = Stream::new();
                    for b in unhex(f[1]) {
                        act.write_byte(b as u32);
                    }
                    assert_eq!(flare_fclip_action_record_finish(p(&mut s), p(&mut act), a(2) as u32, a(3) as u8), 0);
                    assert_eq!(bytes(&mut s), unhex(f[4]), "{line}");
                    *counts.entry("cliprec".into()).or_default() += 1;
                    continue;
                },
                _ => continue,
            };
            let want = unhex(f[f.len() - 1]);
            assert_eq!(bytes(&mut s), want, "{line}");
            *counts.entry(key.into()).or_default() += 1;
        }
        for k in ["unparsed", "geturl2", "gotoframe", "gotoframe2", "if", "jump", "waitframe", "waitframe2", "push", "condbit", "condkey", "cliprec"] {
            assert!(counts.get(k).copied().unwrap_or(0) > 0, "no {k} cases");
        }
    }
}
