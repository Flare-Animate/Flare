//! Port of `common/flash/FDTSprite.cpp`: the `DefineSprite` tag.
//!
//! `FDTSprite` stays in `FDTSprite.h` with inline members: it owns a `std::list<FObj *>` of
//! virtual objects (each writes itself through its own `WriteToSWFStream`), allocates its
//! character ID from `FObjCollection` and counts `IsShowFrame()` objects. What moved here is the
//! byte layout of the tag, behind `flare_fdt_sprite_*` on the `FSWFStream` handles
//! ([`crate::fswfstream`]): the head (character ID, frame count) of the temporary body, and the
//! finish (the `End` tag, then `AppendTag(DefineSprite, body.Size(), body)`).

use crate::fswfstream::Stream;
use std::os::raw::c_void;

const STAG_END: u16 = 0;
const STAG_DEFINE_SPRITE: u16 = 39;

/// Head of the temporary body: `WriteWord(characterID)`, `WriteWord(numOfFrames)`.
///
/// # Safety
/// `body` NULL or a live `FSWFStream` handle.
#[no_mangle]
pub unsafe extern "C" fn flare_fdt_sprite_head(body: *mut c_void, character_id: u16, num_of_frames: u32) -> i32 {
    if body.is_null() {
        return 1;
    }
    crate::guard(2, || {
        let b = &mut *(body as *mut Stream);
        b.write_word(character_id as u32);
        b.write_word(num_of_frames);
        0
    })
}

/// Tail: `body.AppendTag(stagEnd, 0, 0)`, then `out.AppendTag(stagDefineSprite, body.Size(),
/// &body)`.
///
/// # Safety
/// `out`/`body` NULL or live, distinct `FSWFStream` handles.
#[no_mangle]
pub unsafe extern "C" fn flare_fdt_sprite_finish(out: *mut c_void, body: *mut c_void) -> i32 {
    if out.is_null() || body.is_null() || out == body {
        return 1;
    }
    crate::guard(2, || {
        let b = &mut *(body as *mut Stream);
        // Put an end tag on the end of the temporary buffer
        b.append_tag(STAG_END, 0, None);
        let n = b.size();
        (*(out as *mut Stream)).append_tag(STAG_DEFINE_SPRITE, n, Some(b));
        0
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

    #[test]
    fn empty_sprite() {
        let mut out = Stream::new();
        let mut body = Stream::new();
        unsafe {
            assert_eq!(flare_fdt_sprite_head(p(&mut body), 7, 0), 0);
            assert_eq!(flare_fdt_sprite_finish(p(&mut out), p(&mut body)), 0);
        }
        // tag 39, length 6 (id, frames, end tag word)
        assert_eq!(bytes(&mut out), vec![(39u16 << 6 | 6) as u8, (39u16 << 6 >> 8) as u8, 7, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn refuses_bad_handles() {
        let mut a = Stream::new();
        unsafe {
            assert_eq!(flare_fdt_sprite_head(std::ptr::null_mut(), 1, 1), 1);
            assert_eq!(flare_fdt_sprite_finish(p(&mut a), p(&mut a)), 1);
            assert_eq!(flare_fdt_sprite_finish(std::ptr::null_mut(), p(&mut a)), 1);
        }
    }

    // The recorded whole sprites (`sprite|...` lines of swfshapes_cases.txt) need the C++
    // objects; `tests/native/gen_swfshapes_cases.py --verify-chain` replays them through the
    // new header. Here: the tag wrapper around a recorded body.
    #[test]
    fn replay_recorded_sprite_bodies() {
        let mut n = 0;
        for line in include_str!("swfshapes_cases.txt").lines().filter(|l| l.starts_with("spritebody|")) {
            let f: Vec<&str> = line.split('|').collect();
            let hex = |s: &str| -> Vec<u8> {
                if s == "-" {
                    return vec![];
                }
                (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap()).collect()
            };
            let (id, frames): (u16, u32) = (f[1].parse().unwrap(), f[2].parse().unwrap());
            let mut body = Stream::new();
            unsafe { assert_eq!(flare_fdt_sprite_head(p(&mut body), id, frames), 0) };
            for b in hex(f[3]) {
                body.write_byte(b as u32);
            }
            let mut out = Stream::new();
            unsafe { assert_eq!(flare_fdt_sprite_finish(p(&mut out), p(&mut body)), 0) };
            assert_eq!(bytes(&mut out), hex(f[4]), "{line}");
            n += 1;
        }
        assert!(n >= 5);
    }
}
