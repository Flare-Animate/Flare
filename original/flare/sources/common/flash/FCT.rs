//! Port of `common/flash/FCT.cpp`: the SWF control-tag writers.
//!
//! The `FCT*` classes stay in `FCT.h` (owning pointers to `FMatrix`, `FString`, `FCXForm`,
//! `FClipAction`, `FSoundInfo`, ... with virtual `WriteToSWFStream`), now with inline
//! constructors/destructors/accessors. The parts that are pure byte serialisation moved here
//! behind `flare_fct_*`, all taking the destination `FSWFStream` handle
//! ([`crate::fswfstream`]):
//!
//! * whole tags whose body is plain data: `Protect`, `ShowFrame`, `RemoveObject`,
//!   `RemoveObject2`, `FrameLabel`, `SetBackgroundColor`, and `FCTUnparsedTag`'s write;
//! * the fixed head of the tags whose body is completed by polymorphic C++ sub-objects:
//!   `PlaceObject` (character id, depth), `PlaceObject2` (the eight flag bits and the depth)
//!   and `StartSound` (sound id);
//! * [`flare_fct_finish_tag`]: wrap a body built in C++ with `AppendTag` (used by
//!   `DoAction`, which also adds the terminating zero byte, `PlaceObject`, `PlaceObject2` and
//!   `StartSound`).
//!
//! What stays in the header is the interleaving with `FMatrix`/`FCXForm`/`FClipAction`/
//! `FSoundInfo`/`FActionRecord`, which are virtual C++ objects.
//!
//! Deviation: `~FCTUnparsedTag` used `delete data` on a `new[]` block (undefined behaviour);
//! the header now uses `delete[]`.

use crate::fswfstream::Stream;
use std::os::raw::c_void;

const STAG_SHOW_FRAME: u16 = 1;
const STAG_REMOVE_OBJECT: u16 = 5;
const STAG_SET_BACKGROUND_COLOR: u16 = 9;
const STAG_PROTECT: u16 = 24;
const STAG_REMOVE_OBJECT2: u16 = 28;
const STAG_FRAME_LABEL: u16 = 43;

type H = *mut Stream;

unsafe fn tag(out: *mut c_void, id: u16, build: impl FnOnce(&mut Stream)) -> i32 {
    if out.is_null() {
        return 1;
    }
    crate::guard(2, || {
        let mut body = Stream::new();
        build(&mut body);
        let n = body.size();
        (*(out as H)).append_tag(id, n, Some(&mut body));
        0
    })
}

unsafe fn on(h: *mut c_void, f: impl FnOnce(&mut Stream)) -> i32 {
    if h.is_null() {
        return 1;
    }
    crate::guard(2, || {
        f(&mut *(h as H));
        0
    })
}

/// `FCTProtect::WriteToSWFStream` (an empty tag).
///
/// # Safety
/// `out` must be NULL or a live `FSWFStream` handle.
#[no_mangle]
pub unsafe extern "C" fn flare_fct_protect(out: *mut c_void) -> i32 {
    on(out, |s| s.append_tag(STAG_PROTECT, 0, None))
}

/// `FCTShowFrame::WriteToSWFStream` (an empty tag).
///
/// # Safety
/// As [`flare_fct_protect`].
#[no_mangle]
pub unsafe extern "C" fn flare_fct_show_frame(out: *mut c_void) -> i32 {
    on(out, |s| s.append_tag(STAG_SHOW_FRAME, 0, None))
}

/// `FCTRemoveObject::WriteToSWFStream`.
///
/// # Safety
/// As [`flare_fct_protect`].
#[no_mangle]
pub unsafe extern "C" fn flare_fct_remove_object(out: *mut c_void, character_id: u16, depth: u16) -> i32 {
    tag(out, STAG_REMOVE_OBJECT, |b| {
        b.write_word(character_id as u32);
        b.write_word(depth as u32);
    })
}

/// `FCTRemoveObject2::WriteToSWFStream`.
///
/// # Safety
/// As [`flare_fct_protect`].
#[no_mangle]
pub unsafe extern "C" fn flare_fct_remove_object2(out: *mut c_void, depth: u16) -> i32 {
    tag(out, STAG_REMOVE_OBJECT2, |b| b.write_word(depth as u32))
}

/// `FCTFrameLabel::WriteToSWFStream`: the name plus its NUL terminator.
///
/// # Safety
/// `out` NULL or live; `text` valid for `len` bytes (or NULL with `len == 0`).
#[no_mangle]
pub unsafe extern "C" fn flare_fct_frame_label(out: *mut c_void, text: *const u8, len: usize) -> i32 {
    if out.is_null() || (text.is_null() && len > 0) {
        return 1;
    }
    let t: &[u8] = if len == 0 { &[] } else { std::slice::from_raw_parts(text, len) };
    tag(out, STAG_FRAME_LABEL, |b| {
        for &c in t {
            b.write_byte(c as u32);
        }
        b.write_byte(0);
    })
}

/// `FCTSetBackgroundColor::WriteToSWFStream` (`FColor::WriteToSWFStream` of the owned colour:
/// the alpha byte only when `alpha_t != 0`).
///
/// # Safety
/// As [`flare_fct_protect`].
#[no_mangle]
pub unsafe extern "C" fn flare_fct_set_background_color(out: *mut c_void, red: u32, green: u32, blue: u32, alpha: u32, alpha_t: i32) -> i32 {
    tag(out, STAG_SET_BACKGROUND_COLOR, |b| {
        b.write_byte(red);
        b.write_byte(green);
        b.write_byte(blue);
        if alpha_t != 0 {
            b.write_byte(alpha);
        }
    })
}

/// `FCTUnparsedTag::WriteToSWFStream`: the payload registered as large data (kept by the
/// caller until the stream is written).
///
/// # Safety
/// `out` NULL or live; `data` valid for `length` bytes until the stream is written.
#[no_mangle]
pub unsafe extern "C" fn flare_fct_unparsed_tag(out: *mut c_void, tag_id: u16, data: *const u8, length: u32) -> i32 {
    tag(out, tag_id, |b| {
        if length > 0 {
            b.write_large_data(data, length);
        }
    })
}

/// Head of `FCTPlaceObject`'s body: character id, depth.
///
/// # Safety
/// `body` must be NULL or a live `FSWFStream` handle.
#[no_mangle]
pub unsafe extern "C" fn flare_fct_place_object_head(body: *mut c_void, character_id: u16, depth: u16) -> i32 {
    on(body, |b| {
        b.write_word(character_id as u32);
        b.write_word(depth as u32);
    })
}

/// Head of `FCTPlaceObject2`'s body: the eight one-bit flags (clip action, clip depth, has
/// name, ratio, colour transform, matrix, character id, move) and the depth word. Each flag is
/// written from its low bit, like `WriteBits(flag, 1)`.
///
/// # Safety
/// As [`flare_fct_place_object_head`].
#[no_mangle]
pub unsafe extern "C" fn flare_fct_place_object2_head(
    body: *mut c_void,
    has_clip_action: u32,
    has_clip_depth: u32,
    has_name: u32,
    has_ratio: u32,
    has_color_transform: u32,
    has_matrix: u32,
    has_char_id: u32,
    has_move: u32,
    depth: u32,
) -> i32 {
    on(body, |b| {
        b.write_bits(has_clip_action, 1);
        b.write_bits(has_clip_depth, 1);
        b.write_bits(has_name, 1);
        b.write_bits(has_ratio, 1);
        b.write_bits(has_color_transform, 1);
        b.write_bits(has_matrix, 1);
        b.write_bits(has_char_id, 1);
        b.write_bits(has_move, 1);
        b.write_word(depth);
    })
}

/// Head of `FCTStartSound`'s body: the sound id.
///
/// # Safety
/// As [`flare_fct_place_object_head`].
#[no_mangle]
pub unsafe extern "C" fn flare_fct_start_sound_head(body: *mut c_void, sound_id: u16) -> i32 {
    on(body, |b| b.write_word(sound_id as u32))
}

/// Appends `body` to `out` as a `tag_id` tag (`AppendTag(tag, body.Size(), &body)`), after an
/// optional terminating zero byte (`FCTDoAction`), flushing a pending partial byte of the body
/// first when `flush_bits != 0` (`FCTPlaceObject2` ended its body with `FlushBits()`).
///
/// # Safety
/// `out` and `body` NULL or live handles, not the same.
#[no_mangle]
pub unsafe extern "C" fn flare_fct_finish_tag(out: *mut c_void, tag_id: u16, body: *mut c_void, trailing_zero_byte: i32, flush_bits: i32) -> i32 {
    if out.is_null() || body.is_null() || out == body {
        return 1;
    }
    crate::guard(2, || {
        let b = &mut *(body as H);
        if flush_bits != 0 {
            b.flush_bits();
        }
        if trailing_zero_byte != 0 {
            b.write_byte(0);
        }
        let n = b.size();
        (*(out as H)).append_tag(tag_id, n, Some(b));
        0
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn p(s: &mut Stream) -> *mut c_void {
        s as *mut Stream as *mut c_void
    }

    #[test]
    fn simple_tags() {
        let mut s = Stream::new();
        unsafe {
            assert_eq!(flare_fct_show_frame(p(&mut s)), 0);
            assert_eq!(flare_fct_protect(p(&mut s)), 0);
            assert_eq!(flare_fct_remove_object2(p(&mut s), 0x0102), 0);
            assert_eq!(flare_fct_remove_object(p(&mut s), 3, 4), 0);
            assert_eq!(flare_fct_frame_label(p(&mut s), b"ab".as_ptr(), 2), 0);
            assert_eq!(flare_fct_set_background_color(p(&mut s), 1, 2, 3, 9, 0), 0);
            assert_eq!(flare_fct_protect(std::ptr::null_mut()), 1);
            assert_eq!(flare_fct_frame_label(p(&mut s), std::ptr::null(), 1), 1);
        }
        let want: Vec<u8> = [
            &[(1 << 6) as u8, 0][..],                            // ShowFrame, length 0
            &[(24u16 << 6) as u8, (24u16 << 6 >> 8) as u8],      // Protect
            &[(28u16 << 6 | 2) as u8, (28u16 << 6 >> 8) as u8, 2, 1], // RemoveObject2
            &[(5u16 << 6 | 4) as u8, (5u16 << 6 >> 8) as u8, 3, 0, 4, 0],
            &[(43u16 << 6 | 3) as u8, (43u16 << 6 >> 8) as u8, b'a', b'b', 0],
            &[(9u16 << 6 | 3) as u8, (9u16 << 6 >> 8) as u8, 1, 2, 3],
        ]
        .concat();
        assert_eq!(bytes(&mut s), want);
    }

    #[test]
    fn finish_tag_modes() {
        let mut out = Stream::new();
        let mut body = Stream::new();
        unsafe {
            assert_eq!(flare_fct_place_object2_head(p(&mut body), 0, 1, 0, 0, 0, 0, 1, 1, 0x0304), 0);
            assert_eq!(flare_fct_finish_tag(p(&mut out), 26, p(&mut body), 0, 1), 0);
            assert_eq!(flare_fct_finish_tag(p(&mut out), 26, p(&mut out), 0, 0), 1);
            let mut b2 = Stream::new();
            assert_eq!(flare_fct_start_sound_head(p(&mut b2), 7), 0);
            assert_eq!(flare_fct_finish_tag(p(&mut out), 12, p(&mut b2), 1, 0), 0);
        }
        let b = bytes(&mut out);
        // tag 26 (PlaceObject2), body: flags 01000011, depth 0x0304
        assert_eq!(&b[..2], &[(26u16 << 6 | 3) as u8, (26u16 << 6 >> 8) as u8]);
        assert_eq!(&b[2..5], &[0b0100_0011, 0x04, 0x03]);
        // tag 12 (DoAction) with the terminating zero
        assert_eq!(&b[5..], &[(12u16 << 6 | 3) as u8, (12u16 << 6 >> 8) as u8, 7, 0, 0]);
    }

    #[test]
    fn unparsed_tag_keeps_the_payload_as_large_data() {
        let data = [1u8, 2, 3];
        let mut s = Stream::new();
        unsafe {
            assert_eq!(flare_fct_unparsed_tag(p(&mut s), 100, data.as_ptr(), 3), 0);
            assert_eq!(flare_fct_unparsed_tag(p(&mut s), 100, std::ptr::null(), 0), 0);
        }
        assert_eq!(s.size(), 2 + 3 + 2);
        assert_eq!(bytes(&mut s), vec![(100u16 << 6 | 3) as u8, (100u16 << 6 >> 8) as u8, 1, 2, 3, (100u16 << 6) as u8, (100u16 << 6 >> 8) as u8]);
    }

    // Replays the plain tags recorded from the original C++ (swfwriter_cases.txt).
    #[test]
    fn replay_recorded_cpp_cases() {
        let mut counts = std::collections::BTreeMap::<String, usize>::new();
        let mut last_bg = (0u32, 0u32, 0u32, 0u32);
        for line in include_str!("swfwriter_cases.txt").lines() {
            let f: Vec<&str> = line.split('|').collect();
            let mut s = Stream::new();
            let h = p(&mut s);
            let key = match f[0] {
                "fct" => match f[1] {
                    "protect" => unsafe {
                        assert_eq!(flare_fct_protect(h), 0);
                        "protect"
                    },
                    "showframe" => unsafe {
                        assert_eq!(flare_fct_show_frame(h), 0);
                        "showframe"
                    },
                    "remove" => unsafe {
                        let a: Vec<u16> = f[2].split(',').map(|x| x.parse().unwrap()).collect();
                        assert_eq!(flare_fct_remove_object(h, a[0], a[1]), 0);
                        "remove"
                    },
                    "remove2" => unsafe {
                        assert_eq!(flare_fct_remove_object2(h, f[2].parse().unwrap()), 0);
                        "remove2"
                    },
                    "framelabel" => unsafe {
                        let t = unhex(f[2]);
                        assert_eq!(flare_fct_frame_label(h, t.as_ptr(), t.len()), 0);
                        "framelabel"
                    },
                    "bgcolor" => unsafe {
                        let a: Vec<u32> = f[2].split(',').map(|x| x.parse().unwrap()).collect();
                        last_bg = (a[0], a[1], a[2], if a[4] == 1 { a[3] } else { 255 }); // FColor(r,g,b) has alpha 0xff
                        assert_eq!(flare_fct_set_background_color(h, a[0], a[1], a[2], a[3], 0), 0); // AlphaChannel(false) in the ctor
                        "bgcolor"
                    },
                    "bgcolor2" => unsafe {
                        let alpha: i32 = f[2].parse().unwrap();
                        assert_eq!(flare_fct_set_background_color(h, last_bg.0, last_bg.1, last_bg.2, last_bg.3, alpha), 0);
                        "bgcolor2"
                    },
                    _ => continue,
                },
                "unparsed" => unsafe {
                    let d = unhex(f[2]);
                    assert_eq!(flare_fct_unparsed_tag(h, f[1].parse().unwrap(), d.as_ptr(), d.len() as u32), 0);
                    // first write only (the second one follows a SetID in the header)
                    assert_eq!(bytes(&mut s), unhex(f[4]), "{line}");
                    *counts.entry("unparsed".into()).or_default() += 1;
                    continue;
                },
                "doaction" => unsafe {
                    let mut body = Stream::new();
                    for a in f[1].split(';').filter(|a| !a.is_empty()) {
                        for b in unhex(a) {
                            body.write_byte(b as u32);
                        }
                    }
                    assert_eq!(flare_fct_finish_tag(h, 12, p(&mut body), 1, 0), 0);
                    assert_eq!(bytes(&mut s), unhex(f[2]), "{line}");
                    *counts.entry("doaction".into()).or_default() += 1;
                    continue;
                },
                _ => continue,
            };
            let want = if f[0] == "fct" { unhex(f[3]) } else { unreachable!() };
            assert_eq!(bytes(&mut s), want, "{line}");
            *counts.entry(key.into()).or_default() += 1;
        }
        assert_eq!(counts["protect"], 1);
        assert_eq!(counts["showframe"], 1);
        assert_eq!(counts["remove"], 20);
        assert_eq!(counts["remove2"], 20);
        assert_eq!(counts["framelabel"], 10);
        assert_eq!(counts["bgcolor"], 12);
        assert_eq!(counts["bgcolor2"], 12);
        assert_eq!(counts["unparsed"], 24);
        assert_eq!(counts["doaction"], 10);
    }
}
