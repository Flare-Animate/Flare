//! Bitmap data in a package's `bin/*.dat` files. A .dat holds either the
//! original JPEG/PNG/GIF bytes or a lossless bitmap block:
//!
//! ```text
//! u8[2]  03 05            lossless ARGB
//! u16    row bytes (width * 4)
//! u16    width, u16 height
//! u32×4  frame left, right, top, bottom (twips)
//! u8     has alpha, u8 compressed (1)
//! then   chunks of (u16 length, bytes) ending with a 0 length;
//!        the joined chunks are a zlib stream of premultiplied A,R,G,B
//! ```
//! (Little-endian. Layout worked out from public notes and files written by
//! this program; treat as experimental.)

use std::io::{Read, Write};

/// Decode a .dat or image file to straight RGBA.
pub fn decode(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
    if bytes.len() > 2 && bytes[0] == 0x03 && bytes[1] == 0x05 {
        return decode_lossless(bytes);
    }
    let img = image::load_from_memory(bytes).map_err(|e| format!("unreadable bitmap: {e}"))?.to_rgba8();
    Ok((img.width(), img.height(), img.into_raw()))
}

fn u16le(b: &[u8], i: usize) -> Result<u16, String> {
    b.get(i..i + 2).map(|s| u16::from_le_bytes([s[0], s[1]])).ok_or_else(|| "truncated bitmap data".into())
}

fn decode_lossless(b: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
    let w = u16le(b, 4)? as u32;
    let h = u16le(b, 6)? as u32;
    let compressed = *b.get(25).ok_or("truncated bitmap data")? != 0;
    let mut i = 26;
    let mut z = vec![];
    if compressed {
        loop {
            let n = u16le(b, i)? as usize;
            i += 2;
            if n == 0 {
                break;
            }
            z.extend_from_slice(b.get(i..i + n).ok_or("truncated bitmap chunk")?);
            i += n;
        }
    } else {
        z = b[i..].to_vec();
    }
    let raw = if compressed {
        let mut out = vec![];
        flate2::read::ZlibDecoder::new(&z[..]).read_to_end(&mut out).map_err(|e| format!("bitmap data: {e}"))?;
        out
    } else {
        z
    };
    let n = (w * h) as usize;
    if raw.len() < n * 4 {
        return Err("bitmap data shorter than its size".into());
    }
    let mut rgba = Vec::with_capacity(n * 4);
    for px in raw[..n * 4].chunks_exact(4) {
        let (a, r, g, bl) = (px[0], px[1], px[2], px[3]);
        let un = |c: u8| if a == 0 { 0 } else { ((c as u32 * 255 + a as u32 / 2) / a as u32).min(255) as u8 };
        rgba.extend_from_slice(&[un(r), un(g), un(bl), a]);
    }
    Ok((w, h, rgba))
}

/// Encode straight RGBA as a lossless .dat block.
pub fn encode_lossless(w: u32, h: u32, rgba: &[u8]) -> Vec<u8> {
    let mut argb = Vec::with_capacity(rgba.len());
    for px in rgba.chunks_exact(4) {
        let a = px[3] as u32;
        let pm = |c: u8| ((c as u32 * a + 127) / 255) as u8;
        argb.extend_from_slice(&[px[3], pm(px[0]), pm(px[1]), pm(px[2])]);
    }
    let mut enc = flate2::write::ZlibEncoder::new(vec![], flate2::Compression::default());
    enc.write_all(&argb).unwrap();
    let z = enc.finish().unwrap();
    let mut out = vec![0x03, 0x05];
    out.extend_from_slice(&((w * 4) as u16).to_le_bytes());
    out.extend_from_slice(&(w as u16).to_le_bytes());
    out.extend_from_slice(&(h as u16).to_le_bytes());
    for v in [0u32, w * 20, 0, h * 20] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.push(1);
    out.push(1);
    for c in z.chunks(2048) {
        out.extend_from_slice(&(c.len() as u16).to_le_bytes());
        out.extend_from_slice(c);
    }
    out.extend_from_slice(&[0, 0]);
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn lossless_round_trip() {
        let rgba: Vec<u8> = (0..3 * 2).flat_map(|i| [i as u8 * 40, 200, 10, if i % 2 == 0 { 255 } else { 128 }]).collect();
        let dat = super::encode_lossless(3, 2, &rgba);
        let (w, h, back) = super::decode(&dat).unwrap();
        assert_eq!((w, h), (3, 2));
        for (a, b) in rgba.iter().zip(&back) {
            assert!((*a as i32 - *b as i32).abs() <= 2, "{rgba:?} vs {back:?}");
        }
    }
}
