//! Port of `common/flash/FDTFonts.cpp`: the SWF font tags (`DefineFont`, `DefineFont2`,
//! `DefineFontInfo`) and the kerning record.
//!
//! The classes (`FDTDefineFont`, `FDTDefineFont2`, `FDTDefineFontInfo`, `FKerningRec`) stay in
//! `FDTFonts.h`, now with inline members: they own `FShape` glyphs, `FRect` bounds, an
//! `FString` name and the character-ID counter, and they call the *virtual*
//! `FShape::WriteToSWFStream` / `FKerningRec::WriteToSWFStream`, which are still C++. What is
//! pure byte serialisation moved here and takes the destination `FSWFStream` handle
//! ([`crate::fswfstream`]):
//!
//! * [`flare_fdt_define_font_write`]: the whole `DefineFont` tag from the character id, the
//!   glyph offsets and the shape buffer the header rendered;
//! * [`flare_fdt_define_font2_body`]: the `DefineFont2` body up to and including the kerning
//!   count (flags, name, offset table, shape buffer, codes, layout, advances, bounds); the
//!   header then lets each (virtual) kerning record write itself and calls
//!   [`crate::fdt::flare_fdt_finish_tag`];
//! * [`flare_fdt_define_font_info_write`]: the whole `DefineFontInfo` tag (it decides between
//!   8 and 16 bit codes: any code above 65530 makes the table wide);
//! * [`flare_fkerning_write`]: `FKerningRec::WriteToSWFStream`.
//!
//! Quirks of the original that are kept: the `ShiftJIS` flag bit order differs between
//! `DefineFont2` (`1,0,0`) and `DefineFontInfo` (`0,1,0`); an unknown encoding writes no
//! encoding bits at all (the flags byte then shifts); `DefineFont2` truncates each glyph
//! offset to a *signed 8-bit* value (`(S8)shapeBuffer.Size()`) before storing it in the `S16`
//! table, so any shape offset above 127 is wrong in the file; `FDTDefineFont::ID()` and
//! `FDTDefineFontInfo::ID()` return only the low byte of the character id (header);
//! `DefineFontInfo` empties its code table while writing (header).
//!
//! Deviations (undefined behaviour in the C++): the `DefineFont2` offset table was handed to
//! the stream with `WriteLargeData` and freed (`delete[] offsetTable`) before the stream was
//! ever written, so the file read freed memory; the offsets are copied into the body here
//! (little-endian, which is what the x86 original wrote). `DefineFont` with no glyphs
//! dereferenced the list's end sentinel; it now writes the id and the single offset `0`. A
//! `DefineFont2` glyph without bounds dereferenced a null `FRect*`; the header substitutes an
//! empty rectangle.

use crate::fprimitive::rect_write;
use crate::fswfstream::Stream;
use std::os::raw::c_void;

const STAG_DEFINE_FONT: u16 = 10;
const STAG_DEFINE_FONT_INFO: u16 = 13;

const SHIFT_JIS: u16 = 1;
const UNICODE: u16 = 2;
const ANSI: u16 = 3;

type H = *mut Stream;

/// `ptr[..n]`, or `None` when `ptr` is NULL with `n > 0` (a NULL/0 pair is the empty slice).
unsafe fn arr<'a, T>(ptr: *const T, n: usize) -> Option<&'a [T]> {
    if n == 0 {
        Some(&[])
    } else if ptr.is_null() {
        None
    } else {
        Some(std::slice::from_raw_parts(ptr, n))
    }
}

/// `FString::Length()` is a `U16` and `WriteByte` keeps the low 8 bits.
fn name_len_byte(len: usize) -> u32 {
    (len as u16 as u32) & 0xff
}

/// `FDTDefineFont::WriteToSWFStream` (after the header rendered the glyphs into `shapes`).
///
/// `offsets` are the `n` words written after the id: `glyphs * 2` first, then
/// `glyphs * 2 + size of the shape buffer` after every glyph but the last. `shapes` is
/// appended (its large blocks move over, as `FSWFStream::Append` does).
///
/// # Safety
/// `out` and `shapes` NULL or distinct live handles; `offsets` valid for `n` words.
#[no_mangle]
pub unsafe extern "C" fn flare_fdt_define_font_write(out: *mut c_void, character_id: u32, offsets: *const u32, n: usize, shapes: *mut c_void) -> i32 {
    if out.is_null() || shapes.is_null() || out == shapes {
        return 1;
    }
    let offsets = match arr(offsets, n) {
        Some(o) => o,
        None => return 1,
    };
    crate::guard(2, || {
        let mut body = Stream::new();
        body.write_word(character_id);
        for &o in offsets {
            body.write_word(o);
        }
        body.append(&mut *(shapes as H));
        let size = body.size();
        (*(out as H)).append_tag(STAG_DEFINE_FONT, size, Some(&mut body));
        0
    })
}

/// `FDTDefineFont2::WriteToSWFStream` up to and including the kerning count; the caller
/// then writes the kerning records and wraps the body as tag 48.
///
/// Per glyph: `offsets[i]` (already `(S8)` truncated by the caller), `codes[i]`, and, when
/// `has_layout != 0`, `advances[i]` and `bounds[4*i..4*i+4]` = xmin, xmax, ymin, ymax.
/// `shapes` is the rendered glyph buffer (appended). Returns 0, 1 for a NULL/aliased handle or
/// a NULL array that is needed, 2 on a caught panic.
///
/// # Safety
/// `body` and `shapes` NULL or distinct live handles; every array valid for `glyph_count`
/// entries (`bounds`: four times that); `name` valid for `name_len` bytes.
#[no_mangle]
pub unsafe extern "C" fn flare_fdt_define_font2_body(
    body: *mut c_void,
    font_id: u32,
    has_layout: i32,
    encode_type: u16,
    italic: i32,
    bold: i32,
    name: *const u8,
    name_len: usize,
    glyph_count: usize,
    offsets: *const i16,
    codes: *const u16,
    advances: *const i16,
    bounds: *const i32,
    shapes: *mut c_void,
    ascender: i16,
    descender: i16,
    leading: i16,
    kerning_count: usize,
) -> i32 {
    if body.is_null() || shapes.is_null() || body == shapes || glyph_count > (usize::MAX >> 4) {
        return 1;
    }
    let (name, offsets, codes) = match (arr(name, name_len), arr(offsets, glyph_count), arr(codes, glyph_count)) {
        (Some(a), Some(b), Some(c)) => (a, b, c),
        _ => return 1,
    };
    let layout = has_layout != 0;
    let (advances, bounds) = if layout {
        match (arr(advances, glyph_count), arr(bounds, glyph_count * 4)) {
            (Some(a), Some(b)) => (a, b),
            _ => return 1,
        }
    } else {
        (&[][..], &[][..])
    };
    crate::guard(2, || {
        let b = &mut *(body as H);
        b.write_word(font_id);
        // write flags
        b.write_bits(has_layout as u32, 1);
        match encode_type {
            SHIFT_JIS => {
                b.write_bits(1, 1);
                b.write_bits(0, 1);
                b.write_bits(0, 1);
            }
            UNICODE => {
                b.write_bits(0, 1);
                b.write_bits(1, 1);
                b.write_bits(0, 1);
            }
            ANSI => {
                b.write_bits(0, 1);
                b.write_bits(0, 1);
                b.write_bits(1, 1);
            }
            _ => {}
        }
        b.write_bits(0, 1); // 0 for narrowOffsetFlag
        b.write_bits(0, 1); // 0 for narrowOffsetCode
        b.write_bits(italic as u32, 1);
        b.write_bits(bold as u32, 1);
        b.write_byte(0); // FontFlagsReserved UB[8]

        b.write_byte(name_len_byte(name_len));
        for &c in name {
            b.write_byte(c as u32); // the name, without the terminating zero
        }

        b.write_word(glyph_count as u32);

        // the offset table (copied; the original passed a pointer to a freed block)
        for &o in offsets {
            b.write_byte(o as u16 as u32 & 0xff);
            b.write_byte((o as u16 as u32) >> 8);
        }

        // the shape glyph buffer
        b.append(&mut *(shapes as H));

        for &c in codes {
            b.write_word(c as u32);
        }

        // layout information
        if layout {
            b.write_word(ascender as u16 as u32);
            b.write_word(descender as u16 as u32);
            b.write_word(leading as u16 as u32);

            for &a in advances {
                b.write_word(a as u16 as u32);
            }
            for r in bounds.chunks_exact(4) {
                rect_write(b, r[0], r[1], r[2], r[3]);
            }
            b.write_word(kerning_count as u32);
        }
        0
    })
}

/// `FDTDefineFontInfo::WriteToSWFStream`: the whole tag.
///
/// # Safety
/// `out` NULL or a live handle; `name` valid for `name_len` bytes, `codes` for `n` entries.
#[no_mangle]
pub unsafe extern "C" fn flare_fdt_define_font_info_write(
    out: *mut c_void,
    font_id: u32,
    name: *const u8,
    name_len: usize,
    encode_type: u16,
    italic: u32,
    bold: u32,
    codes: *const u16,
    n: usize,
) -> i32 {
    if out.is_null() {
        return 1;
    }
    let (name, codes) = match (arr(name, name_len), arr(codes, n)) {
        (Some(a), Some(b)) => (a, b),
        _ => return 1,
    };
    crate::guard(2, || {
        // determine whether 8 or 16 bit code fields are needed
        let wide = codes.iter().any(|&c| c > 65530);
        let mut body = Stream::new();
        body.write_word(font_id);
        body.write_byte(name_len_byte(name_len));
        for &c in name {
            body.write_byte(c as u32);
        }
        body.write_bits(0, 2); // reserved flags
        match encode_type {
            SHIFT_JIS => {
                body.write_bits(0, 1);
                body.write_bits(1, 1);
                body.write_bits(0, 1);
            }
            UNICODE => {
                body.write_bits(1, 1);
                body.write_bits(0, 1);
                body.write_bits(0, 1);
            }
            ANSI => {
                body.write_bits(0, 1);
                body.write_bits(0, 1);
                body.write_bits(1, 1);
            }
            _ => {}
        }
        body.write_bits(italic, 1);
        body.write_bits(bold, 1);
        body.write_bits(wide as u32, 1);
        for &c in codes {
            if wide {
                body.write_word(c as u32);
            } else {
                body.write_byte(c as u32);
            }
        }
        let size = body.size();
        (*(out as H)).append_tag(STAG_DEFINE_FONT_INFO, size, Some(&mut body));
        0
    })
}

/// `FKerningRec::WriteToSWFStream`: the two codes (bytes, or words when `wide != 0`) and the
/// adjustment word.
///
/// # Safety
/// `h` NULL or a live handle.
#[no_mangle]
pub unsafe extern "C" fn flare_fkerning_write(h: *mut c_void, wide: u16, code1: u16, code2: u16, adjust: i16) -> i32 {
    if h.is_null() {
        return 1;
    }
    crate::guard(2, || {
        let b = &mut *(h as H);
        if wide != 0 {
            b.write_word(code1 as u32);
            b.write_word(code2 as u32);
        } else {
            b.write_byte(code1 as u32);
            b.write_byte(code2 as u32);
        }
        b.write_word(adjust as u16 as u32);
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

    fn p(s: &mut Stream) -> *mut c_void {
        s as *mut Stream as *mut c_void
    }

    fn unhex(s: &str) -> Vec<u8> {
        if s == "-" {
            return vec![];
        }
        (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap()).collect()
    }

    fn csv<T: std::str::FromStr>(s: &str) -> Vec<T>
    where
        T::Err: std::fmt::Debug,
    {
        if s == "-" {
            return vec![];
        }
        s.split(',').map(|x| x.parse().unwrap()).collect()
    }

    #[test]
    fn define_font_layout() {
        let mut shapes = Stream::new();
        shapes.write_byte(0xAA);
        shapes.write_byte(0xBB);
        let mut out = Stream::new();
        unsafe {
            let offs = [4u32, 5];
            assert_eq!(flare_fdt_define_font_write(p(&mut out), 0x0102, offs.as_ptr(), 2, p(&mut shapes)), 0);
            assert_eq!(flare_fdt_define_font_write(p(&mut out), 1, std::ptr::null(), 1, p(&mut shapes)), 1);
            assert_eq!(flare_fdt_define_font_write(p(&mut out), 1, offs.as_ptr(), 2, std::ptr::null_mut()), 1);
            assert_eq!(flare_fdt_define_font_write(std::ptr::null_mut(), 1, offs.as_ptr(), 2, p(&mut shapes)), 1);
        }
        // tag 10, body = id, two offsets, two shape bytes
        assert_eq!(bytes(&mut out), vec![(10u16 << 6 | 8) as u8, (10u16 << 6 >> 8) as u8, 2, 1, 4, 0, 5, 0, 0xAA, 0xBB]);
    }

    #[test]
    fn font_info_wide_and_destructive_flags() {
        let mut out = Stream::new();
        unsafe {
            assert_eq!(flare_fdt_define_font_info_write(p(&mut out), 7, b"Ab".as_ptr(), 2, UNICODE, 1, 0, [1u16, 65530].as_ptr(), 2), 0);
            assert_eq!(flare_fdt_define_font_info_write(p(&mut out), 7, b"Ab".as_ptr(), 2, ANSI, 0, 1, [1u16, 65531].as_ptr(), 2), 0);
            assert_eq!(flare_fdt_define_font_info_write(p(&mut out), 7, std::ptr::null(), 1, ANSI, 0, 1, std::ptr::null(), 0), 1);
        }
        let b = bytes(&mut out);
        // narrow: id, len, "Ab", flags 00_100_1_0_0, codes 1, 0xFA
        assert_eq!(&b[2..], &[7, 0, 2, b'A', b'b', 0b0010_0100, 1, 0xFA, (13u16 << 6 | 10) as u8, (13u16 << 6 >> 8) as u8, 7, 0, 2, b'A', b'b', 0b0000_1011, 1, 0, 0xFB, 0xFF][..]);
        assert_eq!(b[0], (13u16 << 6 | 8) as u8);
    }

    #[test]
    fn kerning_records() {
        let mut s = Stream::new();
        unsafe {
            assert_eq!(flare_fkerning_write(p(&mut s), 0, 0x141, 2, -2), 0);
            assert_eq!(flare_fkerning_write(p(&mut s), 1, 0x141, 2, 3), 0);
            assert_eq!(flare_fkerning_write(std::ptr::null_mut(), 1, 0, 0, 0), 1);
        }
        assert_eq!(bytes(&mut s), vec![0x41, 2, 0xFE, 0xFF, 0x41, 1, 2, 0, 3, 0]);
    }

    #[test]
    fn font2_body_errors_and_encodings() {
        let mut body = Stream::new();
        let mut shapes = Stream::new();
        unsafe {
            // NULL arrays that are needed
            assert_eq!(flare_fdt_define_font2_body(p(&mut body), 1, 1, UNICODE, 0, 0, std::ptr::null(), 0, 1, std::ptr::null(), std::ptr::null(), std::ptr::null(), std::ptr::null(), p(&mut shapes), 0, 0, 0, 0), 1);
            assert_eq!(flare_fdt_define_font2_body(p(&mut body), 1, 1, UNICODE, 0, 0, std::ptr::null(), 0, 0, std::ptr::null(), std::ptr::null(), std::ptr::null(), std::ptr::null(), p(&mut body), 0, 0, 0, 0), 1);
            // no glyphs: id, flags, reserved byte, name length, count, layout words, kerning count
            assert_eq!(flare_fdt_define_font2_body(p(&mut body), 5, 1, SHIFT_JIS, 1, 1, b"F".as_ptr(), 1, 0, std::ptr::null(), std::ptr::null(), std::ptr::null(), std::ptr::null(), p(&mut shapes), 1, 2, 3, 0), 0);
        }
        assert_eq!(bytes(&mut body), vec![5, 0, 0b1100_0011, 0, 1, b'F', 0, 0, 1, 0, 2, 0, 3, 0, 0, 0]);
    }

    // Replays the tags recorded from the original C++ (swfwriter_cases.txt): the same
    // orchestration as FDTFonts.h, driven from the recorded inputs.
    #[test]
    fn replay_recorded_cpp_cases() {
        let mut counts = std::collections::BTreeMap::<&str, usize>::new();
        for line in include_str!("swfwriter_cases.txt").lines() {
            let f: Vec<&str> = line.split('|').collect();
            let mut out = Stream::new();
            match f[0] {
                // font|id|shapes(hex per glyph, ';' separated)|ID()|expected
                "font" => {
                    let glyphs: Vec<Vec<u8>> = if f[2] == "-" { vec![] } else { f[2].split(';').map(unhex).collect() };
                    let n = glyphs.len();
                    let mut shapes = Stream::new();
                    let mut offsets = vec![(n * 2) as u32];
                    for (i, g) in glyphs.iter().enumerate() {
                        for &b in g {
                            shapes.write_byte(b as u32);
                        }
                        if i + 1 != n {
                            offsets.push((n * 2) as u32 + shapes.size());
                        }
                    }
                    let id: u32 = f[1].parse().unwrap();
                    unsafe { assert_eq!(flare_fdt_define_font_write(p(&mut out), id, offsets.as_ptr(), offsets.len(), p(&mut shapes)), 0) };
                    assert_eq!(bytes(&mut out), unhex(f[4]), "{line}");
                }
                // font2|id|encode|italic|bold|name|asc,desc,lead|glyphs(code,adv,xmin,xmax,ymin,ymax,shapehex;..)|kerning(c1,c2,adj;..)|expected
                "font2" => {
                    let (id, enc, it, bo): (u32, u16, i32, i32) = (f[1].parse().unwrap(), f[2].parse().unwrap(), f[3].parse().unwrap(), f[4].parse().unwrap());
                    let name = f[5].as_bytes();
                    let name = if f[5] == "-" { &b""[..] } else { name };
                    let lay: Vec<i16> = csv(f[6]);
                    let glyphs: Vec<&str> = if f[7] == "-" { vec![] } else { f[7].split(';').collect() };
                    let (mut offsets, mut codes, mut advs, mut bounds) = (vec![], vec![], vec![], vec![]);
                    let mut shapes = Stream::new();
                    for g in &glyphs {
                        let v: Vec<&str> = g.split(',').collect();
                        offsets.push(shapes.size() as u8 as i8 as i16);
                        for &b in &unhex(v[6]) {
                            shapes.write_byte(b as u32);
                        }
                        codes.push(v[0].parse::<u16>().unwrap());
                        advs.push(v[1].parse::<i16>().unwrap());
                        for x in &v[2..6] {
                            bounds.push(x.parse::<i32>().unwrap());
                        }
                    }
                    let kern: Vec<[i32; 3]> = if f[8] == "-" { vec![] } else { f[8].split(';').map(|k| { let v: Vec<i32> = csv(k); [v[0], v[1], v[2]] }).collect() };
                    let mut body = Stream::new();
                    let n = glyphs.len();
                    let nz = |v: &Vec<i16>| if n == 0 { std::ptr::null() } else { v.as_ptr() };
                    unsafe {
                        assert_eq!(
                            flare_fdt_define_font2_body(
                                p(&mut body), id, 1, enc, it, bo, name.as_ptr(), name.len(), n, nz(&offsets),
                                if n == 0 { std::ptr::null() } else { codes.as_ptr() }, nz(&advs),
                                if n == 0 { std::ptr::null() } else { bounds.as_ptr() }, p(&mut shapes),
                                lay[0], lay[1], lay[2], kern.len()
                            ),
                            0
                        );
                        for k in &kern {
                            assert_eq!(flare_fkerning_write(p(&mut body), 0, k[0] as u16, k[1] as u16, k[2] as i16), 0);
                        }
                        assert_eq!(crate::fdt::flare_fdt_finish_tag(p(&mut out), 48, p(&mut body), 0, 0), 0);
                    }
                    assert_eq!(bytes(&mut out), unhex(f[9]), "{line}");
                }
                // fontinfo|id|encode|italic|bold|name|codes|expected|expected after the code table was consumed|ID()
                "fontinfo" => {
                    let (id, enc, it, bo): (u32, u16, u32, u32) = (f[1].parse().unwrap(), f[2].parse().unwrap(), f[3].parse().unwrap(), f[4].parse().unwrap());
                    let name = if f[5] == "-" { &b""[..] } else { f[5].as_bytes() };
                    let codes: Vec<u16> = csv(f[6]);
                    let cp = if codes.is_empty() { std::ptr::null() } else { codes.as_ptr() };
                    unsafe { assert_eq!(flare_fdt_define_font_info_write(p(&mut out), id, name.as_ptr(), name.len(), enc, it, bo, cp, codes.len()), 0) };
                    assert_eq!(bytes(&mut out), unhex(f[7]), "{line}");
                    // the C++ class consumed its code table while writing: the second write has no codes
                    let mut out2 = Stream::new();
                    unsafe { assert_eq!(flare_fdt_define_font_info_write(p(&mut out2), id, name.as_ptr(), name.len(), enc, it, bo, std::ptr::null(), 0), 0) };
                    assert_eq!(bytes(&mut out2), unhex(f[8]), "{line}");
                    // ... and ID() is the low byte of the character id (the header's quirk)
                }
                _ => continue,
            }
            *counts.entry(f[0]).or_default() += 1;
        }
        assert_eq!(counts.get("font"), Some(&FONT_CASES));
        assert_eq!(counts.get("font2"), Some(&FONT2_CASES));
        assert_eq!(counts.get("fontinfo"), Some(&FONTINFO_CASES));
    }

    const FONT_CASES: usize = 16;
    const FONT2_CASES: usize = 48;
    const FONTINFO_CASES: usize = 24;
}
