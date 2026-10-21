//! Port of `common/flash/FDTText.cpp`: the SWF text tags (`DefineEditText`, `DefineText`,
//! `DefineText2`) and their text records.
//!
//! The classes (`FDTDefineEditText`, `FDTDefineText`, `FDTDefineText2`, `FTextChangeRecord`,
//! `FTextGlyphRecord`) stay in `FDTText.h` with inline members: they own `FRect`/`FMatrix`/
//! `FColor`/`FString` objects, store the records behind the *virtual* `FTextRecord`
//! interface and keep the character-ID counter. The byte serialisation moved here, taking the
//! destination `FSWFStream` handle ([`crate::fswfstream`]):
//!
//! * [`flare_fdt_edit_text_write`]: the whole `DefineEditText` tag (the many flag bits, the
//!   optional font/colour/length/layout fields, the zero terminated variable name and
//!   initial text), from a `repr(C)` argument block;
//! * [`flare_fdt_text_change_write`]: `FTextChangeRecord::WriteToSWFStream`;
//! * [`flare_fdt_text_glyph_bits`] / [`flare_fdt_text_glyph_write`]: `FTextGlyphRecord`'s
//!   `MinAdvanceBits` / `MinCodeBits` and `WriteToSWFStream`.
//!
//! `DefineText`/`DefineText2` themselves are driven from the header (id, bounds, matrix, the
//! virtual `MinAdvanceBits`/`MinCodeBits` maxima, the two bit-width bytes, the virtual record
//! writes) and finish with [`crate::fdt::flare_fdt_finish_tag`] (flush the bit writer, the
//! terminating zero byte, `AppendTag`).
//!
//! Quirks kept: the flags are written with `WriteBits(x, 1)`, i.e. only the low bit of each
//! `U8` flag is stored while the `if (flag)` tests use the whole value; the glyph record stores
//! the glyph count in 7 bits (more than 127 glyphs per record wrap); the colours of
//! `DefineEditText` and the text change record are always written with an alpha byte.
//!
//! Deviations (null dereferences in the C++): a NULL `variableName` / `initialText` /
//! colour / `bounds` is treated by the header as an empty string / black / empty rectangle.

use crate::fprimitive::{color_write, rect_write};
use crate::fswfstream::{min_bits, Stream};
use std::os::raw::c_void;

const STAG_DEFINE_EDIT_TEXT: u16 = 37;

type H = *mut Stream;

/// Argument block of [`flare_fdt_edit_text_write`]; `FDTText.h` declares the same layout.
/// Every scalar is a 32 bit value (the `U8`/`U16` members are zero-extended).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct EditTextArgs {
    pub xmin: i32,
    pub xmax: i32,
    pub ymin: i32,
    pub ymax: i32,
    pub has_font: u32,
    pub has_max_length: u32,
    pub has_text_color: u32,
    pub read_only: u32,
    pub password: u32,
    pub multiline: u32,
    pub word_wrap: u32,
    pub has_text: u32,
    pub use_outlines: u32,
    pub border: u32,
    pub no_select: u32,
    pub has_layout: u32,
    pub font_id: u32,
    pub font_height: u32,
    pub max_length: u32,
    pub alignment: u32,
    pub left_margin: u32,
    pub right_margin: u32,
    pub indent: u32,
    pub leading: u32,
    pub color_red: u32,
    pub color_green: u32,
    pub color_blue: u32,
    pub color_alpha: u32,
    pub variable_name: *const u8,
    pub variable_name_len: usize,
    pub initial_text: *const u8,
    pub initial_text_len: usize,
}

unsafe fn bytes<'a>(p: *const u8, n: usize) -> Option<&'a [u8]> {
    if n == 0 {
        Some(&[])
    } else if p.is_null() {
        None
    } else {
        Some(std::slice::from_raw_parts(p, n))
    }
}

fn zstring(s: &mut Stream, text: &[u8]) {
    for &b in text {
        s.write_byte(b as u32);
    }
    s.write_byte(0);
}

/// `FDTDefineEditText::WriteToSWFStream`: the whole tag 37.
///
/// # Safety
/// `out` NULL or a live handle; `args` NULL or valid, its string pointers valid for their
/// lengths.
#[no_mangle]
pub unsafe extern "C" fn flare_fdt_edit_text_write(out: *mut c_void, character_id: u32, args: *const EditTextArgs) -> i32 {
    if out.is_null() || args.is_null() {
        return 1;
    }
    let a = *args;
    let (name, init) = match (bytes(a.variable_name, a.variable_name_len), bytes(a.initial_text, a.initial_text_len)) {
        (Some(n), Some(i)) => (n, i),
        _ => return 1,
    };
    crate::guard(2, || {
        let mut t = Stream::new();
        t.write_word(character_id);
        rect_write(&mut t, a.xmin, a.xmax, a.ymin, a.ymax);

        t.write_bits(a.has_text, 1);
        t.write_bits(a.word_wrap, 1);
        t.write_bits(a.multiline, 1);
        t.write_bits(a.password, 1);
        t.write_bits(a.read_only, 1);
        t.write_bits(a.has_text_color, 1);
        t.write_bits(a.has_max_length, 1);
        t.write_bits(a.has_font, 1);

        t.write_bits(0, 2); // unknown flags
        t.write_bits(a.has_layout, 1);
        t.write_bits(a.no_select, 1);
        t.write_bits(a.border, 1);
        t.write_bits(0, 2); // unknown flags
        t.write_bits(a.use_outlines, 1);

        if a.has_font as u8 != 0 {
            t.write_word(a.font_id);
            t.write_word(a.font_height);
        }
        if a.has_text_color as u8 != 0 {
            color_write(&mut t, a.color_red, a.color_green, a.color_blue, a.color_alpha, true);
        }
        if a.has_max_length as u8 != 0 {
            t.write_word(a.max_length);
        }
        if a.has_layout as u8 != 0 {
            t.write_byte(a.alignment);
            t.write_word(a.left_margin);
            t.write_word(a.right_margin);
            t.write_word(a.indent);
            t.write_word(a.leading);
        }
        zstring(&mut t, name);
        if a.has_text as u8 != 0 {
            zstring(&mut t, init);
        }
        let n = t.size();
        (*(out as H)).append_tag(STAG_DEFINE_EDIT_TEXT, n, Some(&mut t));
        0
    })
}

/// `FTextChangeRecord::WriteToSWFStream`. Flags are the `U16` members (low bit stored, any
/// non-zero value taken as set by the `if`s); the colour is written with its alpha byte.
///
/// # Safety
/// `h` NULL or a live handle.
#[no_mangle]
pub unsafe extern "C" fn flare_fdt_text_change_write(
    h: *mut c_void,
    has_font: u16,
    has_color: u16,
    has_x_offset: u16,
    has_y_offset: u16,
    font_id: u16,
    font_height: u16,
    red: u32,
    green: u32,
    blue: u32,
    alpha: u32,
    x_offset: i16,
    y_offset: i16,
) -> i32 {
    if h.is_null() {
        return 1;
    }
    crate::guard(2, || {
        let s = &mut *(h as H);
        s.write_bits(1, 1);
        s.write_bits(0, 3); // reserved
        s.write_bits(has_font as u32, 1);
        s.write_bits(has_color as u32, 1);
        s.write_bits(has_y_offset as u32, 1);
        s.write_bits(has_x_offset as u32, 1);
        if has_font != 0 {
            s.write_word(font_id as u32);
        }
        if has_color != 0 {
            color_write(s, red, green, blue, alpha, true);
        }
        if has_x_offset != 0 {
            s.write_word(x_offset as u16 as u32);
        }
        if has_y_offset != 0 {
            s.write_word(y_offset as u16 as u32);
        }
        if has_font != 0 {
            s.write_word(font_height as u32);
        }
        0
    })
}

/// `FTextGlyphRecord::MinCodeBits` / `MinAdvanceBits` over `n` entries stored as consecutive
/// `(code, advance)` `u16` pairs: the largest `MinBits(value, 1)` of each column (0 when
/// empty).
///
/// # Safety
/// `entries` valid for `2 * n` values; the out pointers NULL or writable.
#[no_mangle]
pub unsafe extern "C" fn flare_fdt_text_glyph_bits(entries: *const u16, n: usize, code_bits: *mut u32, advance_bits: *mut u32) -> i32 {
    if n > (usize::MAX >> 2) || (n > 0 && entries.is_null()) {
        return 1;
    }
    crate::guard(2, || {
        let e: &[u16] = if n == 0 { &[] } else { std::slice::from_raw_parts(entries, 2 * n) };
        let (mut cb, mut ab) = (0u32, 0u32);
        for g in e.chunks_exact(2) {
            cb = cb.max(min_bits(g[0] as u32, 1));
            ab = ab.max(min_bits(g[1] as u32, 1));
        }
        if !code_bits.is_null() {
            *code_bits = cb;
        }
        if !advance_bits.is_null() {
            *advance_bits = ab;
        }
        0
    })
}

/// `FTextGlyphRecord::WriteToSWFStream`: a zero bit, the 7 bit glyph count, then each
/// `(code, advance)` in `code_bits` / `advance_bits` bits.
///
/// # Safety
/// `h` NULL or a live handle; `entries` valid for `2 * n` values.
#[no_mangle]
pub unsafe extern "C" fn flare_fdt_text_glyph_write(h: *mut c_void, entries: *const u16, n: usize, code_bits: i32, advance_bits: i32) -> i32 {
    if h.is_null() || n > (usize::MAX >> 2) || (n > 0 && entries.is_null()) {
        return 1;
    }
    crate::guard(2, || {
        let e: &[u16] = if n == 0 { &[] } else { std::slice::from_raw_parts(entries, 2 * n) };
        let s = &mut *(h as H);
        s.write_bits(0, 1);
        s.write_bits(n as u32, 7);
        for g in e.chunks_exact(2) {
            s.write_bits(g[0] as u32, code_bits as u32);
            s.write_bits(g[1] as u32, advance_bits as u32);
        }
        0
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bytes_of(s: &mut Stream) -> Vec<u8> {
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

    fn nums<T: std::str::FromStr>(s: &str) -> Vec<T>
    where
        T::Err: std::fmt::Debug,
    {
        if s == "-" {
            return vec![];
        }
        s.split(',').map(|x| x.parse().unwrap()).collect()
    }

    fn args(name: &[u8], init: &[u8]) -> EditTextArgs {
        EditTextArgs {
            xmin: 0,
            xmax: 0,
            ymin: 0,
            ymax: 0,
            has_font: 0,
            has_max_length: 0,
            has_text_color: 0,
            read_only: 0,
            password: 0,
            multiline: 0,
            word_wrap: 0,
            has_text: 0,
            use_outlines: 0,
            border: 0,
            no_select: 0,
            has_layout: 0,
            font_id: 0,
            font_height: 0,
            max_length: 0,
            alignment: 0,
            left_margin: 0,
            right_margin: 0,
            indent: 0,
            leading: 0,
            color_red: 0,
            color_green: 0,
            color_blue: 0,
            color_alpha: 0,
            variable_name: name.as_ptr(),
            variable_name_len: name.len(),
            initial_text: init.as_ptr(),
            initial_text_len: init.len(),
        }
    }

    #[test]
    fn edit_text_minimal_and_full() {
        let mut out = Stream::new();
        let a = args(b"v", b"hi");
        let mut full = args(b"v", b"hi");
        full.has_font = 1;
        full.has_text_color = 1;
        full.has_max_length = 1;
        full.has_layout = 1;
        full.has_text = 1;
        full.font_id = 3;
        full.font_height = 240;
        full.max_length = 9;
        full.alignment = 2;
        full.color_red = 1;
        full.color_green = 2;
        full.color_blue = 3;
        full.color_alpha = 4;
        unsafe {
            assert_eq!(flare_fdt_edit_text_write(p(&mut out), 1, &a), 0);
            assert_eq!(flare_fdt_edit_text_write(p(&mut out), 2, &full), 0);
            assert_eq!(flare_fdt_edit_text_write(p(&mut out), 2, std::ptr::null()), 1);
            let mut bad = a;
            bad.variable_name = std::ptr::null();
            assert_eq!(flare_fdt_edit_text_write(p(&mut out), 2, &bad), 1);
        }
        let b = bytes_of(&mut out);
        // first tag: id, the all-zero rectangle (5 + 4 bits, flushed), the two flag bytes, "v\0"
        assert_eq!(&b[..2], &[(37u16 << 6 | 8) as u8, (37u16 << 6 >> 8) as u8]);
        assert_eq!(&b[2..10], &[1, 0, 0b0000_1000, 0, 0, 0, b'v', 0]);
        // second tag: hasText 1, hasColor/hasMaxLength/hasFont 1 | hasLayout 1, then the optional
        // fields in order: font, colour with alpha, max length, layout, name, text
        let t2 = &b[10 + 2..];
        assert_eq!(&t2[..4], &[2, 0, 0b0000_1000, 0]);
        assert_eq!(t2[4], 0b1000_0111);
        assert_eq!(t2[5], 0b0010_0000);
        assert_eq!(&t2[6..], &[3, 0, 240, 0, 1, 2, 3, 4, 9, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, b'v', 0, b'h', b'i', 0]);
    }

    #[test]
    fn change_and_glyph_records() {
        let mut s = Stream::new();
        unsafe {
            assert_eq!(flare_fdt_text_change_write(p(&mut s), 1, 1, 1, 1, 7, 100, 9, 8, 7, 6, -1, 2), 0);
            assert_eq!(flare_fdt_text_change_write(std::ptr::null_mut(), 1, 1, 1, 1, 7, 100, 9, 8, 7, 6, -1, 2), 1);
        }
        assert_eq!(bytes_of(&mut s), vec![0b1000_1111, 7, 0, 9, 8, 7, 6, 0xFF, 0xFF, 2, 0, 100, 0]);

        let g = [5u16, 300, 0, 1];
        let (mut cb, mut ab) = (0u32, 0u32);
        unsafe {
            assert_eq!(flare_fdt_text_glyph_bits(g.as_ptr(), 2, &mut cb, &mut ab), 0);
            assert_eq!(flare_fdt_text_glyph_bits(std::ptr::null(), 2, &mut cb, &mut ab), 1);
            assert_eq!(flare_fdt_text_glyph_bits(std::ptr::null(), 0, &mut cb, &mut ab), 0);
            assert_eq!((cb, ab), (0, 0));
            assert_eq!(flare_fdt_text_glyph_bits(g.as_ptr(), 2, &mut cb, &mut ab), 0);
        }
        assert_eq!((cb, ab), (min_bits(5, 1), min_bits(300, 1)));
        let mut w = Stream::new();
        unsafe {
            assert_eq!(flare_fdt_text_glyph_write(p(&mut w), g.as_ptr(), 2, cb as i32, ab as i32), 0);
            assert_eq!(flare_fdt_text_glyph_write(p(&mut w), std::ptr::null(), 1, 4, 4), 1);
        }
        w.flush_bits();
        // 0 | 0000010 | 0101 -> hmm: count in 7 bits then codes: verified by the replay below
        assert_eq!(w.size(), (1 + 7 + 2 * (cb + ab) + 7) / 8);
    }

    // Replays the tags recorded from the original C++ (swfwriter_cases.txt).
    #[test]
    fn replay_recorded_cpp_cases() {
        let mut counts = std::collections::BTreeMap::<&str, usize>::new();
        for line in include_str!("swfwriter_cases.txt").lines() {
            let f: Vec<&str> = line.split('|').collect();
            let mut out = Stream::new();
            match f[0] {
                // edittext|id|bounds|flags(12)|fontid,height,maxlen,align,left,right,indent,leading|rgba|name|init|expected
                "edittext" => {
                    let b: Vec<i32> = nums(f[2]);
                    let fl: Vec<u32> = nums(f[3]);
                    let n: Vec<u32> = nums(f[4]);
                    let c: Vec<u32> = nums(f[5]);
                    let name = if f[6] == "-" { vec![] } else { unhex(f[6]) };
                    let init = if f[7] == "-" { vec![] } else { unhex(f[7]) };
                    let mut a = args(&name, &init);
                    (a.xmin, a.xmax, a.ymin, a.ymax) = (b[0], b[1], b[2], b[3]);
                    (a.has_font, a.has_max_length, a.has_text_color, a.read_only, a.password, a.multiline) = (fl[0], fl[1], fl[2], fl[3], fl[4], fl[5]);
                    (a.word_wrap, a.has_text, a.use_outlines, a.border, a.no_select, a.has_layout) = (fl[6], fl[7], fl[8], fl[9], fl[10], fl[11]);
                    (a.font_id, a.font_height, a.max_length, a.alignment) = (n[0], n[1], n[2], n[3]);
                    (a.left_margin, a.right_margin, a.indent, a.leading) = (n[4], n[5], n[6], n[7]);
                    (a.color_red, a.color_green, a.color_blue, a.color_alpha) = (c[0], c[1], c[2], c[3]);
                    unsafe { assert_eq!(flare_fdt_edit_text_write(p(&mut out), f[1].parse().unwrap(), &a), 0) };
                    assert_eq!(bytes_of(&mut out), unhex(f[8]), "{line}");
                }
                // text|tag|id|bounds|records|expected ; record = G:code.adv.code.adv.. | C:hf,hc,hx,hy,fid,fh,r,g,b,a,x,y
                "text" => {
                    let tag: u16 = f[1].parse().unwrap();
                    let b: Vec<i32> = nums(f[3]);
                    let recs: Vec<&str> = if f[4] == "-" { vec![] } else { f[4].split(';').collect() };
                    let mut body = Stream::new();
                    body.write_word(f[2].parse().unwrap());
                    crate::fprimitive::rect_write(&mut body, b[0], b[1], b[2], b[3]);
                    // matrix: identity (one zero byte), as the harness uses FMatrix()
                    body.write_byte(0);
                    let mut glyphs: Vec<Option<Vec<u16>>> = vec![];
                    let (mut cb, mut ab) = (0u32, 0u32);
                    for r in &recs {
                        if let Some(g) = r.strip_prefix("G:") {
                            let v: Vec<u16> = if g.is_empty() { vec![] } else { nums(&g.replace('.', ",")) };
                            let (mut c, mut a) = (0u32, 0u32);
                            unsafe { assert_eq!(flare_fdt_text_glyph_bits(v.as_ptr(), v.len() / 2, &mut c, &mut a), 0) };
                            cb = cb.max(c);
                            ab = ab.max(a);
                            glyphs.push(Some(v));
                        } else {
                            glyphs.push(None);
                        }
                    }
                    body.write_byte(cb);
                    body.write_byte(ab);
                    for (r, g) in recs.iter().zip(&glyphs) {
                        match g {
                            Some(v) => unsafe { assert_eq!(flare_fdt_text_glyph_write(p(&mut body), v.as_ptr(), v.len() / 2, cb as i32, ab as i32), 0) },
                            None => {
                                let v: Vec<i64> = nums(&r[2..]);
                                unsafe {
                                    assert_eq!(
                                        flare_fdt_text_change_write(p(&mut body), v[0] as u16, v[1] as u16, v[2] as u16, v[3] as u16, v[4] as u16, v[5] as u16, v[6] as u32, v[7] as u32, v[8] as u32, v[9] as u32, v[10] as i16, v[11] as i16),
                                        0
                                    )
                                };
                            }
                        }
                    }
                    unsafe { assert_eq!(crate::fdt::flare_fdt_finish_tag(p(&mut out), tag, p(&mut body), 1, 1), 0) };
                    assert_eq!(bytes_of(&mut out), unhex(f[5]), "{line}");
                }
                _ => continue,
            }
            *counts.entry(f[0]).or_default() += 1;
        }
        assert_eq!(counts.get("edittext"), Some(&EDIT_CASES));
        assert_eq!(counts.get("text"), Some(&TEXT_CASES));
    }

    const EDIT_CASES: usize = 48;
    const TEXT_CASES: usize = 40;
}
