#![allow(clippy::expect_used)]

use std::io::Write;

use super::*;

/// A payload as Animate writes it: header, then the rows raw or as length-prefixed zlib chunks.
fn payload(w: u16, h: u16, row: u16, alpha: bool, body: &[u8], chunk: Option<usize>) -> Vec<u8> {
    let mut out = vec![0x03, 0x05];
    for v in [row, w, h] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    for v in [0, i32::from(w) * 20, 0, i32::from(h) * 20] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.push(u8::from(alpha));
    out.push(u8::from(chunk.is_some()));
    match chunk {
        None => out.extend_from_slice(body),
        Some(size) => {
            let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
            z.write_all(body).expect("write");
            for piece in z.finish().expect("finish").chunks(size) {
                out.extend_from_slice(&(piece.len() as u16).to_le_bytes());
                out.extend_from_slice(piece);
            }
            out.extend_from_slice(&[0, 0]);
        }
    }
    out
}

/// 2x1 pixels as ARGB: half-transparent premultiplied red, and opaque green.
const ARGB: [u8; 8] = [128, 128, 0, 0, 255, 0, 255, 0];

#[test]
fn raw_with_alpha_is_unpremultiplied() {
    let p = decode(&payload(2, 1, 8, true, &ARGB, None)).expect("decode");
    assert_eq!((p.width, p.height), (2, 1));
    assert_eq!(p.rgba, [255, 0, 0, 128, 0, 255, 0, 255]);
}

#[test]
fn compressed_chunks_of_any_size_join() {
    for size in [1, 3, 5000] {
        let p = decode(&payload(2, 1, 8, true, &ARGB, Some(size))).expect("decode");
        assert_eq!(p.rgba, [255, 0, 0, 128, 0, 255, 0, 255], "chunk {size}");
    }
}

#[test]
fn without_alpha_the_pixels_are_opaque_whatever_the_first_byte() {
    let body = [0, 10, 20, 30, 7, 40, 50, 60];
    for chunk in [None, Some(4)] {
        let p = decode(&payload(2, 1, 8, false, &body, chunk)).expect("decode");
        assert_eq!(p.rgba, [10, 20, 30, 255, 40, 50, 60, 255]);
    }
}

#[test]
fn fully_transparent_pixels_become_clear_black() {
    let p = decode(&payload(1, 1, 4, true, &[0, 9, 9, 9], None)).expect("decode");
    assert_eq!(p.rgba, [0, 0, 0, 0]);
}

#[test]
fn failures_are_typed() {
    let ok = payload(2, 1, 8, true, &ARGB, Some(4));
    assert!(matches!(
        decode(&ok[..10]),
        Err(FlashBitmapError::TruncatedHeader)
    ));
    let mut bad = ok.clone();
    bad[1] = 4;
    assert!(matches!(decode(&bad), Err(FlashBitmapError::BadSignature)));
    // Cut inside a chunk, and before the terminating chunk.
    assert!(matches!(
        decode(&ok[..ok.len() - 5]),
        Err(FlashBitmapError::Truncated)
    ));
    assert!(matches!(
        decode(&ok[..ok.len() - 2]),
        Err(FlashBitmapError::Truncated)
    ));
    // Row length that is not four bytes a pixel (an 8-bit bitmap).
    assert!(matches!(
        decode(&payload(2, 1, 2, true, &ARGB, None)),
        Err(FlashBitmapError::UnsupportedRows(2, 2))
    ));
    assert!(matches!(
        decode(&payload(2, 1, 8, true, &ARGB[..7], None)),
        Err(FlashBitmapError::WrongLength(7, 8))
    ));
    assert!(matches!(
        decode(&payload(2, 1, 8, true, &[0; 9], None)),
        Err(FlashBitmapError::WrongLength(9, 8))
    ));
}

#[test]
fn zero_and_oversize_dimensions_are_refused() {
    assert!(matches!(
        decode(&payload(0, 1, 0, true, &[], None)),
        Err(FlashBitmapError::BadSize(0, 1))
    ));
    assert!(matches!(
        decode(&payload(1, 0, 4, true, &[], None)),
        Err(FlashBitmapError::BadSize(1, 0))
    ));
    // Over the side limit, and over the pixel limit with each side allowed (no pixel data is read).
    let wide = u16::try_from(MAX_BITMAP_SIDE + 1).expect("fits");
    assert!(matches!(
        decode(&payload(wide, 1, wide.wrapping_mul(4), true, &[], None)),
        Err(FlashBitmapError::BadSize(..))
    ));
    assert!(matches!(
        decode(&payload(8000, 8000, 0, true, &[], None)),
        Err(FlashBitmapError::BadSize(8000, 8000))
    ));
}

#[test]
fn bad_zlib_and_wrong_inflated_sizes_are_refused() {
    let mut junk = payload(2, 1, 8, true, &[], None);
    junk[25] = 1;
    junk.extend_from_slice(&[4, 0, 1, 2, 3, 4, 0, 0]);
    assert!(matches!(decode(&junk), Err(FlashBitmapError::BadZlib)));
    // A valid stream holding less, or more, than the header says.
    assert!(matches!(
        decode(&payload(2, 1, 8, true, &ARGB[..4], Some(100))),
        Err(FlashBitmapError::Truncated)
    ));
    assert!(matches!(
        decode(&payload(2, 1, 8, true, &[0; 64], Some(100))),
        Err(FlashBitmapError::WrongLength(9, 8))
    ));
}

#[test]
fn a_zlib_bomb_stops_at_the_declared_size() {
    // Half a MiB of zeros compresses to under a KiB; the header claims 8 bytes.
    let big = vec![0u8; 512 << 10];
    let result = decode(&payload(2, 1, 8, true, &big, Some(65535)));
    assert!(matches!(result, Err(FlashBitmapError::WrongLength(9, 8))));
}

#[test]
fn more_chunks_than_the_size_can_need_are_refused() {
    let mut data = payload(1, 1, 4, true, &[], None);
    data[25] = 1;
    for _ in 0..3000 {
        data.extend_from_slice(&[1, 0, 0]);
    }
    assert!(matches!(decode(&data), Err(FlashBitmapError::TooMuchData)));
}
