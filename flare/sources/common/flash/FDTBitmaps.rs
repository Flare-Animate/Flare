//! Port of `common/flash/FDTBitmaps.cpp`: the SWF bitmap tags (`DefineBits`,
//! `DefineBitsJPEG2/3`, `DefineBitsLossless/2`, `JPEGTables`).
//!
//! The `FDT*` classes stay in `FDTBitmaps.h` (they are `FObj`s with virtual
//! `WriteToSWFStream` and own/borrow the pixel buffers); their constructors, character-ID
//! allocation and buffer ownership are inline there. The tag bodies are written here, into the
//! caller's `FSWFStream` handle ([`crate::fswfstream`]), through `flare_fdt_*`:
//!
//! * `DefineBits`, `JPEG2` (the empty encoding table `FFD9 FFD8`, the stream, `FFD9`),
//!   `JPEG3` (adds the offset dword and the alpha block) and `JPEGTables` register their pixel
//!   data as *large data*: the block is spliced in only when the stream is written, so the
//!   caller must keep it alive until then, exactly as before;
//! * `DefineBitsLossless/2`: [`flare_fdt_lossless_capacity`] / [`flare_fdt_lossless_compress`]
//!   do the constructor's work (colour table + image into one buffer, zlib level 9 into a buffer
//!   of `total + total/100 + 12` bytes) and [`flare_fdt_define_bits_lossless`] writes the tag.
//!
//! Deviations: the deflate output comes from `miniz_oxide`, not zlib (valid zlib stream, same
//! inflated bytes, not byte-identical; see `FSWFStream.rs`); a stream that miniz would write
//! larger than zlib's worst-case bound falls back to stored blocks (same bound as zlib's
//! `compressBound` for the sizes involved); sizes that overflow the C++ `int` arithmetic
//! (`width * height * bits`) or `format` > 30 are refused with an error status instead of
//! being undefined behaviour.

use crate::fswfstream::{zlib_compress, Stream};
use std::os::raw::c_void;

const STAG_DEFINE_BITS: u16 = 6;
const STAG_JPEG_TABLES: u16 = 8;
const STAG_DEFINE_BITS_LOSSLESS: u16 = 20;
const STAG_DEFINE_BITS_JPEG2: u16 = 21;
const STAG_DEFINE_BITS_JPEG3: u16 = 35;
const STAG_DEFINE_BITS_LOSSLESS2: u16 = 36;

/// `bm8Bit`: formats up to this one carry a colour table (and a count byte).
const BM_8BIT: u8 = 3;

unsafe fn tag(out: *mut c_void, id: u16, build: impl FnOnce(&mut Stream)) -> i32 {
    if out.is_null() {
        return 1;
    }
    crate::guard(2, || {
        let mut body = Stream::new();
        build(&mut body);
        let n = body.size();
        (*(out as *mut Stream)).append_tag(id, n, Some(&mut body));
        0
    })
}

/// `FDTDefineBits::WriteToSWFStream`.
///
/// # Safety
/// `out` NULL or a live `FSWFStream` handle; `image` must stay valid for `size` bytes until
/// the stream is written.
#[no_mangle]
pub unsafe extern "C" fn flare_fdt_define_bits(out: *mut c_void, character_id: u16, image: *const u8, size: u32) -> i32 {
    tag(out, STAG_DEFINE_BITS, |b| {
        b.write_word(character_id as u32);
        b.write_large_data(image, size);
    })
}

/// `FDTDefineBitsJPEG2::WriteToSWFStream`.
///
/// # Safety
/// As [`flare_fdt_define_bits`].
#[no_mangle]
pub unsafe extern "C" fn flare_fdt_define_bits_jpeg2(out: *mut c_void, character_id: u16, jpeg: *const u8, jpeg_size: u32) -> i32 {
    tag(out, STAG_DEFINE_BITS_JPEG2, |b| {
        b.write_word(character_id as u32);

        // 2 bytes indicating end of encoding stream
        // no encoding data is written here because it is an empty stream
        b.write_byte(0xff);
        b.write_byte(0xd9);

        // 2 bytes indicating beginning of JPEG stream
        b.write_byte(0xff);
        b.write_byte(0xd8);

        // the entire JPEG stream
        b.write_large_data(jpeg, jpeg_size);

        // 2 bytes indicating end of JPEG stream
        b.write_byte(0xff);
        b.write_byte(0xd9);
    })
}

/// `FDTDefineBitsJPEG3::WriteToSWFStream`.
///
/// # Safety
/// As [`flare_fdt_define_bits`], for both blocks.
#[no_mangle]
pub unsafe extern "C" fn flare_fdt_define_bits_jpeg3(
    out: *mut c_void,
    character_id: u16,
    jpeg: *const u8,
    jpeg_size: u32,
    alpha: *const u8,
    alpha_size: u32,
) -> i32 {
    tag(out, STAG_DEFINE_BITS_JPEG3, |b| {
        b.write_word(character_id as u32);

        // offset includes the 2 end of stream tags and the 1 beginning stream tag
        let offset = jpeg_size.wrapping_add(6);
        b.write_dword(offset);

        b.write_byte(0xff);
        b.write_byte(0xd9);
        b.write_byte(0xff);
        b.write_byte(0xd8);

        b.write_large_data(jpeg, jpeg_size);

        b.write_byte(0xff);
        b.write_byte(0xd9);

        // alpha data
        b.write_large_data(alpha, alpha_size);
    })
}

/// `FDTJPEGTables::WriteToSWFStream`.
///
/// # Safety
/// As [`flare_fdt_define_bits`].
#[no_mangle]
pub unsafe extern "C" fn flare_fdt_jpeg_tables(out: *mut c_void, data: *const u8, size: u32) -> i32 {
    tag(out, STAG_JPEG_TABLES, |b| {
        b.write_large_data(data, size);
    })
}

/// Sizes the `FDTDefineBitsLosslessBase` constructor computed with `int`/`U32` arithmetic:
/// `(table_bytes, image_bytes, total, capacity)`; `None` where the C++ overflowed or was UB.
fn lossless_sizes(format: u8, width: u16, height: u16, count: i32, alpha: bool) -> Option<(usize, usize, u32, u32)> {
    if format > 30 || count < 0 {
        return None;
    }
    let rgb_bytes: i64 = if alpha { 4 } else { 3 };
    let table_bytes = count as i64 * rgb_bytes;
    let bits = 1i64 << format; // how many bits does this format have?
    let image_bytes = (width as i64 * height as i64 * bits + 7) / 8;
    if table_bytes > i32::MAX as i64 || image_bytes > i32::MAX as i64 {
        return None;
    }
    let total = table_bytes + image_bytes;
    if total > u32::MAX as i64 {
        return None;
    }
    let total = total as u32;
    let cap = total.wrapping_add(total / 100).wrapping_add(12);
    Some((table_bytes as usize, image_bytes as usize, total, cap))
}

/// Capacity of the compressed buffer (`total + total/100 + 12`), or -1 when the sizes are
/// refused (see the module note).
#[no_mangle]
pub extern "C" fn flare_fdt_lossless_capacity(format: u8, width: u16, height: u16, color_table_count: i32, alpha: i32) -> i64 {
    crate::guard(-1, || match lossless_sizes(format, width, height, color_table_count, alpha != 0) {
        Some((_, _, _, cap)) => cap as i64,
        None => -1,
    })
}

/// Colour table + image into one buffer, deflated at level 9 into `out` (capacity `out_cap`,
/// which must be the value of [`flare_fdt_lossless_capacity`]); the size goes to `*out_len`.
/// Status 0 ok, 1 invalid argument, 2 panic, 3 buffer too small.
///
/// # Safety
/// `color_table` valid for `count * (3|4)` bytes, `image` for the image bytes, `out` for
/// `out_cap`, `out_len` for one `u32`.
#[no_mangle]
pub unsafe extern "C" fn flare_fdt_lossless_compress(
    format: u8,
    width: u16,
    height: u16,
    color_table_count: i32,
    alpha: i32,
    color_table: *const u8,
    image: *const u8,
    out: *mut u8,
    out_cap: u32,
    out_len: *mut u32,
) -> i32 {
    let Some((table_bytes, image_bytes, total, cap)) = lossless_sizes(format, width, height, color_table_count, alpha != 0) else {
        return 1;
    };
    if out.is_null()
        || out_len.is_null()
        || out_cap < cap
        || (table_bytes > 0 && color_table.is_null())
        || (image_bytes > 0 && image.is_null())
    {
        return 1;
    }
    crate::guard(2, || {
        let mut raw = Vec::with_capacity(total as usize);
        if table_bytes > 0 {
            raw.extend_from_slice(std::slice::from_raw_parts(color_table, table_bytes));
        }
        if image_bytes > 0 {
            raw.extend_from_slice(std::slice::from_raw_parts(image, image_bytes));
        }
        let mut z = zlib_compress(&raw, 9);
        if z.len() > cap as usize {
            z = zlib_compress(&raw, 0); // stored blocks: zlib's own worst case
        }
        if z.len() > cap as usize {
            return 3;
        }
        std::slice::from_raw_parts_mut(out, z.len()).copy_from_slice(&z);
        *out_len = z.len() as u32;
        0
    })
}

/// `FDTDefineBitsLosslessBase::WriteToSWFStream`.
///
/// # Safety
/// `out` NULL or a live `FSWFStream` handle; `compressed` valid for `compressed_size` bytes
/// until the stream is written.
#[no_mangle]
pub unsafe extern "C" fn flare_fdt_define_bits_lossless(
    out: *mut c_void,
    character_id: u16,
    format: u8,
    width: u16,
    height: u16,
    color_table_count: i32,
    compressed: *const u8,
    compressed_size: u32,
    alpha: i32,
) -> i32 {
    tag(out, if alpha != 0 { STAG_DEFINE_BITS_LOSSLESS2 } else { STAG_DEFINE_BITS_LOSSLESS }, |b| {
        b.write_word(character_id as u32);
        b.write_byte(format as u32);
        b.write_word(width as u32);
        b.write_word(height as u32);

        if format <= BM_8BIT {
            b.write_byte(color_table_count.wrapping_sub(1) as u32);
        }

        b.write_large_data(compressed, compressed_size);
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

    #[test]
    fn jpeg_tags() {
        let img = [1u8, 2, 3];
        let mut out = Stream::new();
        let p = &mut out as *mut Stream as *mut c_void;
        unsafe {
            assert_eq!(flare_fdt_define_bits(p, 5, img.as_ptr(), 3), 0);
            assert_eq!(flare_fdt_define_bits_jpeg2(p, 7, img.as_ptr(), 3), 0);
            assert_eq!(flare_fdt_jpeg_tables(p, img.as_ptr(), 3), 0);
            assert_eq!(flare_fdt_define_bits(std::ptr::null_mut(), 5, img.as_ptr(), 3), 1);
        }
        let b = bytes(&mut out);
        let mut want = vec![0x85, 0x01, 5, 0, 1, 2, 3]; // DefineBits: tag 6, length 5
        want.extend_from_slice(&[0x4b, 0x05, 7, 0, 0xff, 0xd9, 0xff, 0xd8, 1, 2, 3, 0xff, 0xd9]); // JPEG2: tag 21, length 11
        want.extend_from_slice(&[0x03, 0x02, 1, 2, 3]); // JPEGTables: tag 8, length 3
        assert_eq!(b, want);
    }

    #[test]
    fn jpeg3_offset_counts_the_jpeg_plus_six() {
        let (jpeg, alpha) = ([9u8; 4], [7u8; 2]);
        let mut out = Stream::new();
        unsafe {
            assert_eq!(flare_fdt_define_bits_jpeg3(&mut out as *mut Stream as *mut c_void, 1, jpeg.as_ptr(), 4, alpha.as_ptr(), 2), 0);
        }
        let b = bytes(&mut out);
        assert_eq!(u16::from_le_bytes([b[0], b[1]]) >> 6, 35);
        assert_eq!(&b[2..4], &[1, 0]);
        assert_eq!(&b[4..8], &10u32.to_le_bytes());
        assert_eq!(&b[b.len() - 2..], &[7, 7]);
    }

    #[test]
    fn lossless_sizes_follow_the_cpp_formula() {
        // 8-bit, 3x2 image, 4 palette entries RGB: 6 + 12 bytes
        assert_eq!(lossless_sizes(3, 3, 2, 4, false), Some((12, 6, 18, 18 + 0 + 12)));
        // 1-bit images round up to whole bytes
        assert_eq!(lossless_sizes(0, 9, 1, 2, true), Some((8, 2, 10, 10 + 12)));
        // 32-bit, no table
        assert_eq!(lossless_sizes(5, 10, 10, 0, true), Some((0, 400, 400, 400 + 4 + 12)));
        assert_eq!(lossless_sizes(31, 1, 1, 0, false), None);
        assert_eq!(lossless_sizes(3, 1, 1, -1, false), None);
        assert_eq!(lossless_sizes(30, 65535, 65535, 0, false), None);
        assert_eq!(flare_fdt_lossless_capacity(3, 3, 2, 4, 0), 30);
        assert_eq!(flare_fdt_lossless_capacity(3, 3, 2, -4, 0), -1);
    }

    #[test]
    fn lossless_compress_round_trips_and_checks_buffers() {
        let table = [1u8, 2, 3, 4, 5, 6];
        let image = [0u8, 1, 1, 0, 0, 1];
        let cap = flare_fdt_lossless_capacity(3, 3, 2, 2, 0) as u32;
        let mut out = vec![0u8; cap as usize];
        let mut n = 0u32;
        unsafe {
            assert_eq!(flare_fdt_lossless_compress(3, 3, 2, 2, 0, table.as_ptr(), image.as_ptr(), out.as_mut_ptr(), cap, &mut n), 0);
            let mut raw = table.to_vec();
            raw.extend_from_slice(&image);
            assert_eq!(miniz_oxide::inflate::decompress_to_vec_zlib(&out[..n as usize]).unwrap(), raw);
            assert_eq!(flare_fdt_lossless_compress(3, 3, 2, 2, 0, std::ptr::null(), image.as_ptr(), out.as_mut_ptr(), cap, &mut n), 1);
            assert_eq!(flare_fdt_lossless_compress(3, 3, 2, 2, 0, table.as_ptr(), image.as_ptr(), out.as_mut_ptr(), cap - 1, &mut n), 1);
            assert_eq!(flare_fdt_lossless_compress(3, 3, 2, 2, 0, table.as_ptr(), image.as_ptr(), std::ptr::null_mut(), cap, &mut n), 1);
        }
        // the incompressible worst case still fits the capacity
        let noise: Vec<u8> = (0..4000u32).map(|i| (i.wrapping_mul(2654435761) >> 13) as u8).collect();
        let cap = flare_fdt_lossless_capacity(5, 1000, 1, 0, 1) as u32;
        let mut out = vec![0u8; cap as usize];
        unsafe {
            assert_eq!(flare_fdt_lossless_compress(5, 1000, 1, 0, 1, std::ptr::null(), noise.as_ptr(), out.as_mut_ptr(), cap, &mut n), 0);
        }
        assert!(n <= cap);
        assert_eq!(miniz_oxide::inflate::decompress_to_vec_zlib(&out[..n as usize]).unwrap(), noise);
    }

    #[test]
    fn lossless_tag_layout() {
        let z = [0xAAu8; 5];
        let mut out = Stream::new();
        let p = &mut out as *mut Stream as *mut c_void;
        unsafe {
            assert_eq!(flare_fdt_define_bits_lossless(p, 3, 3, 4, 2, 17, z.as_ptr(), 5, 0), 0);
            assert_eq!(flare_fdt_define_bits_lossless(p, 3, 5, 4, 2, 0, z.as_ptr(), 5, 1), 0);
        }
        let b = bytes(&mut out);
        let c0 = u16::from_le_bytes([b[0], b[1]]);
        assert_eq!(c0 >> 6, 20);
        assert_eq!(&b[2..2 + (c0 & 0x3f) as usize], &[3, 0, 3, 4, 0, 2, 0, 16, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA][..]);
        let off = 2 + (c0 & 0x3f) as usize;
        let c1 = u16::from_le_bytes([b[off], b[off + 1]]);
        assert_eq!(c1 >> 6, 36); // alpha variant, no table count byte for format 5
        assert_eq!((c1 & 0x3f) as usize, 2 + 1 + 2 + 2 + 5);
    }

    // Replays the tags recorded from the original C++ (swfwriter_cases.txt). Zlib streams are
    // compared after inflating (see the module note).
    #[test]
    fn replay_recorded_cpp_cases() {
        let mut counts = std::collections::BTreeMap::<&str, usize>::new();
        for line in include_str!("swfwriter_cases.txt").lines() {
            let f: Vec<&str> = line.split('|').collect();
            let mut out = Stream::new();
            let p = &mut out as *mut Stream as *mut c_void;
            match f[0] {
                "defbits" => {
                    let img = unhex(f[1]);
                    unsafe { assert_eq!(flare_fdt_define_bits(p, f[2].parse().unwrap(), img.as_ptr(), img.len() as u32), 0) };
                    assert_eq!(bytes(&mut out), unhex(f[3]), "{line}");
                }
                "jpeg2" => {
                    let img = unhex(f[1]);
                    unsafe { assert_eq!(flare_fdt_define_bits_jpeg2(p, f[2].parse().unwrap(), img.as_ptr(), img.len() as u32), 0) };
                    assert_eq!(bytes(&mut out), unhex(f[3]), "{line}");
                }
                "jpeg3" => {
                    let (img, al) = (unhex(f[1]), unhex(f[2]));
                    unsafe {
                        assert_eq!(flare_fdt_define_bits_jpeg3(p, f[3].parse().unwrap(), img.as_ptr(), img.len() as u32, al.as_ptr(), al.len() as u32), 0)
                    };
                    assert_eq!(bytes(&mut out), unhex(f[4]), "{line}");
                }
                "jpegtables" => {
                    let enc = unhex(f[1]);
                    unsafe { assert_eq!(flare_fdt_jpeg_tables(p, enc.as_ptr(), enc.len() as u32), 0) };
                    assert_eq!(bytes(&mut out), unhex(f[2]), "{line}");
                }
                "lossless" => {
                    let alpha: i32 = f[1].parse().unwrap();
                    let (format, w, h, count): (u8, u16, u16, i32) = (f[2].parse().unwrap(), f[3].parse().unwrap(), f[4].parse().unwrap(), f[5].parse().unwrap());
                    let (table, img) = (unhex(f[6]), unhex(f[7]));
                    let cap = flare_fdt_lossless_capacity(format, w, h, count, alpha);
                    assert!(cap > 0);
                    let mut comp = vec![0u8; cap as usize];
                    let mut n = 0u32;
                    let tp = if table.is_empty() { std::ptr::null() } else { table.as_ptr() };
                    unsafe {
                        assert_eq!(flare_fdt_lossless_compress(format, w, h, count, alpha, tp, img.as_ptr(), comp.as_mut_ptr(), cap as u32, &mut n), 0);
                        assert_eq!(flare_fdt_define_bits_lossless(p, f[8].parse().unwrap(), format, w, h, count, comp.as_ptr(), n, alpha), 0);
                    }
                    let t = bytes(&mut out);
                    let code = u16::from_le_bytes([t[0], t[1]]);
                    assert_eq!((code >> 6) as u32, f[9].parse::<u32>().unwrap());
                    let (hdr, len) = if code & 0x3f == 0x3f { (6, u32::from_le_bytes([t[2], t[3], t[4], t[5]]) as usize) } else { (2, (code & 0x3f) as usize) };
                    assert_eq!(hdr + len, t.len(), "tag length field: {line}");
                    let prefix = f[12].len() / 2;
                    assert_eq!(&t[hdr..hdr + prefix], &unhex(f[12])[..], "{line}");
                    let inflated = miniz_oxide::inflate::decompress_to_vec_zlib(&t[hdr + prefix..]).unwrap();
                    assert_eq!(inflated, unhex(f[13]), "{line}");
                }
                _ => continue,
            }
            *counts.entry(f[0]).or_default() += 1;
        }
        assert_eq!(counts.get("defbits"), Some(&12));
        assert_eq!(counts.get("jpeg2"), Some(&12));
        assert_eq!(counts.get("jpeg3"), Some(&12));
        assert_eq!(counts.get("jpegtables"), Some(&6));
        assert_eq!(counts.get("lossless"), Some(&60));
    }
}
