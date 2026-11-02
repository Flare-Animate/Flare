//! Image file encoders the exporters need beyond tiny-skia's PNG: baseline JPEG (ITU T.81) and
//! animated GIF (GIF89a), each from straight RGBA pixels.

use std::collections::HashMap;
use std::f32::consts::{FRAC_1_SQRT_2, PI};

/// A baseline JPEG: YCbCr without subsampling, the standard tables scaled to `quality` (1..=100,
/// as Animate's JPEG Quality runs). Alpha is ignored.
pub fn jpeg(rgba: &[u8], width: u16, height: u16, quality: u8) -> Vec<u8> {
    let q = quality.clamp(1, 100) as u32;
    let scale = if q < 50 { 5000 / q } else { 200 - 2 * q };
    let tables = [LUMA_Q, CHROMA_Q].map(|base| base.map(|v| ((v as u32 * scale + 50) / 100).clamp(1, 255) as u8));
    let mut out = vec![0xFF, 0xD8];
    segment(&mut out, 0xE0, b"JFIF\0\x01\x01\0\0\x01\0\x01\0\0");
    let mut dqt = Vec::new();
    for (id, table) in tables.iter().enumerate() {
        dqt.push(id as u8);
        dqt.extend(ZIGZAG.map(|i| table[i]));
    }
    segment(&mut out, 0xDB, &dqt);
    let ([h1, h0], [w1, w0]) = (height.to_be_bytes(), width.to_be_bytes());
    segment(&mut out, 0xC0, &[8, h1, h0, w1, w0, 3, 1, 0x11, 0, 2, 0x11, 1, 3, 0x11, 1]);
    let huffman = [(0x00, &DC_LUMA_BITS, &DC_VALS[..]), (0x10, &AC_LUMA_BITS, &AC_LUMA_VALS[..]), (0x01, &DC_CHROMA_BITS, &DC_VALS[..]), (0x11, &AC_CHROMA_BITS, &AC_CHROMA_VALS[..])];
    let mut dht = Vec::new();
    for (class, bits, vals) in huffman {
        dht.push(class);
        dht.extend(bits);
        dht.extend(vals);
    }
    segment(&mut out, 0xC4, &dht);
    segment(&mut out, 0xDA, &[3, 1, 0x00, 2, 0x11, 3, 0x11, 0, 63, 0]);

    let huff = huffman.map(|(_, bits, vals)| codes(bits, vals));
    let cos: [[f32; 8]; 8] = std::array::from_fn(|a| std::array::from_fn(|b| ((2 * a + 1) as f32 * b as f32 * PI / 16.0).cos()));
    let (w, h) = (width as usize, height as usize);
    let mut bits = Bits { out: &mut out, acc: 0, n: 0 };
    let mut last_dc = [0i32; 3];
    for by in (0..h).step_by(8) {
        for bx in (0..w).step_by(8) {
            // The block in YCbCr, level-shifted; edge pixels repeat past the image.
            let mut blocks = [[0.0f32; 64]; 3];
            for i in 0..64 {
                let (x, y) = ((bx + i % 8).min(w - 1), (by + i / 8).min(h - 1));
                let p = &rgba[(y * w + x) * 4..];
                let (r, g, b) = (p[0] as f32, p[1] as f32, p[2] as f32);
                blocks[0][i] = 0.299 * r + 0.587 * g + 0.114 * b - 128.0;
                blocks[1][i] = -0.168_736 * r - 0.331_264 * g + 0.5 * b;
                blocks[2][i] = 0.5 * r - 0.418_688 * g - 0.081_312 * b;
            }
            for (c, block) in blocks.iter().enumerate() {
                let (table, dc, ac) = if c == 0 { (&tables[0], &huff[0], &huff[1]) } else { (&tables[1], &huff[2], &huff[3]) };
                let f = fdct(block, &cos);
                // The AC tables code values up to 10 bits wide, which quality 100 can pass.
                let coeffs: [i32; 64] = std::array::from_fn(|k| (f[ZIGZAG[k]] / table[ZIGZAG[k]] as f32).round().clamp(-1023.0, 1023.0) as i32);
                let (size, value) = magnitude(coeffs[0] - last_dc[c]);
                last_dc[c] = coeffs[0];
                bits.code(dc[size as usize]);
                bits.put(value, size);
                let mut run = 0;
                for &v in &coeffs[1..] {
                    if v == 0 {
                        run += 1;
                        continue;
                    }
                    while run > 15 {
                        bits.code(ac[0xF0]);
                        run -= 16;
                    }
                    let (size, value) = magnitude(v);
                    bits.code(ac[(run << 4) | size as usize]);
                    bits.put(value, size);
                    run = 0;
                }
                if run > 0 {
                    bits.code(ac[0x00]);
                }
            }
        }
    }
    // The last byte fills up with 1 bits.
    let pad = (8 - bits.n) % 8;
    bits.put((1 << pad) - 1, pad);
    out.extend([0xFF, 0xD9]);
    out
}

fn segment(out: &mut Vec<u8>, marker: u8, data: &[u8]) {
    out.extend([0xFF, marker]);
    out.extend(((data.len() + 2) as u16).to_be_bytes());
    out.extend(data);
}

/// The 8×8 forward DCT of a block, row by row, as T.81 A.3.3 defines it.
fn fdct(block: &[f32; 64], cos: &[[f32; 8]; 8]) -> [f32; 64] {
    let c = |u: usize| if u == 0 { FRAC_1_SQRT_2 } else { 1.0 };
    let rows: [f32; 64] = std::array::from_fn(|i| (0..8).map(|x| block[i / 8 * 8 + x] * cos[x][i % 8]).sum());
    std::array::from_fn(|i| {
        let (v, u) = (i / 8, i % 8);
        0.25 * c(u) * c(v) * (0..8).map(|y| rows[y * 8 + u] * cos[y][v]).sum::<f32>()
    })
}

/// A coefficient's size category and its bits: negative values as their one's complement.
fn magnitude(v: i32) -> (u32, u32) {
    let size = 32 - v.unsigned_abs().leading_zeros();
    let bits = if v < 0 { (v - 1) as u32 & ((1 << size) - 1) } else { v as u32 };
    (size, bits)
}

/// Each symbol's Huffman code and length, from the counts of codes per length (T.81 Annex C).
fn codes(bits: &[u8; 16], vals: &[u8]) -> [(u32, u32); 256] {
    let mut table = [(0, 0); 256];
    let (mut code, mut k) = (0, 0);
    for len in 1..=16 {
        for _ in 0..bits[len - 1] {
            table[vals[k] as usize] = (code, len as u32);
            code += 1;
            k += 1;
        }
        code <<= 1;
    }
    table
}

/// The entropy-coded segment's bit writer: a 0xFF byte is followed by 0x00.
struct Bits<'a> {
    out: &'a mut Vec<u8>,
    acc: u32,
    n: u32,
}

impl Bits<'_> {
    fn code(&mut self, (code, len): (u32, u32)) {
        self.put(code, len);
    }

    fn put(&mut self, value: u32, len: u32) {
        if len == 0 {
            return;
        }
        self.acc = (self.acc << len) | (value & ((1 << len) - 1));
        self.n += len;
        while self.n >= 8 {
            self.n -= 8;
            let byte = (self.acc >> self.n) as u8;
            self.out.push(byte);
            if byte == 0xFF {
                self.out.push(0);
            }
        }
        self.acc &= (1 << self.n) - 1;
    }
}

/// An animated GIF, frame by frame: every frame covers the whole image, with a palette of its own.
pub struct Gif {
    out: Vec<u8>,
    width: u16,
    height: u16,
}

impl Gif {
    /// A `width` × `height` GIF that loops forever, or plays `repeat` more times after the first.
    pub fn new(width: u16, height: u16, repeat: Option<u16>) -> Gif {
        let mut out = b"GIF89a".to_vec();
        out.extend(width.to_le_bytes());
        out.extend(height.to_le_bytes());
        // No global colour table; background index 0; square pixels.
        out.extend([0, 0, 0]);
        // NETSCAPE2.0's loop count, 0 for ever. Played once, it's left out, as some viewers read 0 there as "for ever".
        if repeat != Some(0) {
            out.extend(b"\x21\xFF\x0BNETSCAPE2.0\x03\x01");
            out.extend(repeat.unwrap_or(0).to_le_bytes());
            out.push(0);
        }
        Gif { out, width, height }
    }

    /// Adds a frame of straight RGBA pixels, shown for `delay` hundredths of a second. Pixels under
    /// half alpha are transparent.
    // ponytail: no dithering, and no cropping to what changed since the last frame.
    pub fn frame(&mut self, rgba: &[u8], delay: u16) {
        let (palette, indices, clear) = quantize(rgba);
        // Colour tables hold a power of two entries, 2 at least; the transparent one is the last used.
        let used = palette.len() + clear as usize;
        let bits = (usize::BITS - (used.max(2) - 1).leading_zeros()).max(1);
        // Disposal: every frame clears to the background before the next is drawn, so none shows
        // through a later frame's transparent pixels.
        let packed = (2 << 2) | clear as u8;
        self.out.extend([0x21, 0xF9, 4, packed]);
        self.out.extend(delay.to_le_bytes());
        self.out.extend([palette.len() as u8 * clear as u8, 0]);
        self.out.push(0x2C);
        self.out.extend([0, 0, 0, 0]);
        self.out.extend(self.width.to_le_bytes());
        self.out.extend(self.height.to_le_bytes());
        self.out.push(0x80 | (bits - 1) as u8);
        for i in 0..1 << bits {
            self.out.extend(palette.get(i).copied().unwrap_or([0; 3]));
        }
        let min = bits.max(2) as u8;
        let lzw = weezl::encode::Encoder::new(weezl::BitOrder::Lsb, min).encode(&indices).unwrap_or_default();
        self.out.push(min);
        for chunk in lzw.chunks(255) {
            self.out.push(chunk.len() as u8);
            self.out.extend(chunk);
        }
        self.out.push(0);
    }

    pub fn finish(mut self) -> Vec<u8> {
        self.out.push(0x3B);
        self.out
    }
}

/// A palette for `rgba`'s opaque pixels, each pixel's index into it, and whether any pixel is
/// transparent (those get index `palette.len()`). Up to 256 colours (255 with transparency) are
/// kept exactly; more are cut down by median cut over 5-bit channels.
fn quantize(rgba: &[u8]) -> (Vec<[u8; 3]>, Vec<u8>, bool) {
    let pixels: Vec<Option<[u8; 3]>> = rgba.chunks_exact(4).map(|p| (p[3] >= 128).then(|| [p[0], p[1], p[2]])).collect();
    let clear = pixels.iter().any(Option::is_none);
    let max = if clear { 255 } else { 256 };
    let mut exact: HashMap<[u8; 3], u8> = HashMap::new();
    for c in pixels.iter().flatten() {
        if exact.len() > max {
            break;
        }
        let next = exact.len();
        exact.entry(*c).or_insert(next as u8);
    }
    if exact.len() <= max {
        let mut palette = vec![[0; 3]; exact.len()];
        for (&c, &i) in &exact {
            palette[i as usize] = c;
        }
        let indices = pixels.iter().map(|p| p.map_or(palette.len() as u8, |c| exact[&c])).collect();
        return (palette, indices, clear);
    }
    // Median cut: buckets of 5-bit colours with their pixel counts and channel sums.
    let key = |c: [u8; 3]| (c[0] as usize >> 3) << 10 | (c[1] as usize >> 3) << 5 | c[2] as usize >> 3;
    let mut sums = vec![[0u64; 4]; 1 << 15];
    for c in pixels.iter().flatten() {
        let s = &mut sums[key(*c)];
        s[0] += 1;
        (0..3).for_each(|i| s[i + 1] += c[i] as u64);
    }
    let channel = |k: usize, i: usize| (k >> (10 - 5 * i)) & 31;
    let mut boxes: Vec<Vec<usize>> = vec![(0..1 << 15).filter(|&k| sums[k][0] > 0).collect()];
    while boxes.len() < max {
        // The fullest box that can still split, along its widest channel, at its middle pixel.
        let count = |b: &Vec<usize>| b.iter().map(|&k| sums[k][0]).sum::<u64>();
        let Some(i) = (0..boxes.len()).filter(|&i| boxes[i].len() > 1).max_by_key(|&i| count(&boxes[i])) else { break };
        let mut b = std::mem::take(&mut boxes[i]);
        let range = |c: usize| b.iter().map(|&k| channel(k, c)).max().unwrap_or(0) - b.iter().map(|&k| channel(k, c)).min().unwrap_or(0);
        let wide = (0..3).max_by_key(|&c| range(c)).unwrap_or(0);
        b.sort_by_key(|&k| channel(k, wide));
        let (total, mut seen) = (count(&b), 0);
        let cut = b.iter().position(|&k| {
            seen += sums[k][0];
            seen * 2 >= total
        });
        let cut = cut.unwrap_or(0).clamp(0, b.len() - 2) + 1;
        boxes[i] = b.split_off(cut);
        boxes.push(b);
    }
    let mut index = vec![0u8; 1 << 15];
    let palette = boxes
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let mut s = [0u64; 4];
            for &k in b {
                index[k] = i as u8;
                (0..4).for_each(|c| s[c] += sums[k][c]);
            }
            [1, 2, 3].map(|c| (s[c] / s[0].max(1)) as u8)
        })
        .collect::<Vec<_>>();
    let indices = pixels.iter().map(|p| p.map_or(palette.len() as u8, |c| index[key(c)])).collect();
    (palette, indices, clear)
}

/// Natural (row by row) order of the zig-zag sequence's coefficients.
const ZIGZAG: [usize; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20, 13, 6, 7, 14, 21, 28, 35,
    42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59, 52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
];

// The example tables of T.81 Annex K.
const LUMA_Q: [u8; 64] = [
    16, 11, 10, 16, 24, 40, 51, 61, 12, 12, 14, 19, 26, 58, 60, 55, 14, 13, 16, 24, 40, 57, 69, 56, 14, 17, 22, 29, 51, 87, 80,
    62, 18, 22, 37, 56, 68, 109, 103, 77, 24, 35, 55, 64, 81, 104, 113, 92, 49, 64, 78, 87, 103, 121, 120, 101, 72, 92, 95, 98,
    112, 100, 103, 99,
];
const CHROMA_Q: [u8; 64] = [
    17, 18, 24, 47, 99, 99, 99, 99, 18, 21, 26, 66, 99, 99, 99, 99, 24, 26, 56, 99, 99, 99, 99, 99, 47, 66, 99, 99, 99, 99, 99,
    99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99,
    99, 99,
];
const DC_LUMA_BITS: [u8; 16] = [0, 1, 5, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0];
const DC_CHROMA_BITS: [u8; 16] = [0, 3, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0];
const DC_VALS: [u8; 12] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
const AC_LUMA_BITS: [u8; 16] = [0, 2, 1, 3, 3, 2, 4, 3, 5, 5, 4, 4, 0, 0, 1, 0x7D];
const AC_LUMA_VALS: [u8; 162] = [
    0x01, 0x02, 0x03, 0x00, 0x04, 0x11, 0x05, 0x12, 0x21, 0x31, 0x41, 0x06, 0x13, 0x51, 0x61, 0x07, 0x22, 0x71, 0x14, 0x32,
    0x81, 0x91, 0xA1, 0x08, 0x23, 0x42, 0xB1, 0xC1, 0x15, 0x52, 0xD1, 0xF0, 0x24, 0x33, 0x62, 0x72, 0x82, 0x09, 0x0A, 0x16,
    0x17, 0x18, 0x19, 0x1A, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2A, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3A, 0x43, 0x44, 0x45,
    0x46, 0x47, 0x48, 0x49, 0x4A, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5A, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69,
    0x6A, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7A, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89, 0x8A, 0x92, 0x93, 0x94,
    0x95, 0x96, 0x97, 0x98, 0x99, 0x9A, 0xA2, 0xA3, 0xA4, 0xA5, 0xA6, 0xA7, 0xA8, 0xA9, 0xAA, 0xB2, 0xB3, 0xB4, 0xB5, 0xB6,
    0xB7, 0xB8, 0xB9, 0xBA, 0xC2, 0xC3, 0xC4, 0xC5, 0xC6, 0xC7, 0xC8, 0xC9, 0xCA, 0xD2, 0xD3, 0xD4, 0xD5, 0xD6, 0xD7, 0xD8,
    0xD9, 0xDA, 0xE1, 0xE2, 0xE3, 0xE4, 0xE5, 0xE6, 0xE7, 0xE8, 0xE9, 0xEA, 0xF1, 0xF2, 0xF3, 0xF4, 0xF5, 0xF6, 0xF7, 0xF8,
    0xF9, 0xFA,
];
const AC_CHROMA_BITS: [u8; 16] = [0, 2, 1, 2, 4, 4, 3, 4, 7, 5, 4, 4, 0, 1, 2, 0x77];
const AC_CHROMA_VALS: [u8; 162] = [
    0x00, 0x01, 0x02, 0x03, 0x11, 0x04, 0x05, 0x21, 0x31, 0x06, 0x12, 0x41, 0x51, 0x07, 0x61, 0x71, 0x13, 0x22, 0x32, 0x81,
    0x08, 0x14, 0x42, 0x91, 0xA1, 0xB1, 0xC1, 0x09, 0x23, 0x33, 0x52, 0xF0, 0x15, 0x62, 0x72, 0xD1, 0x0A, 0x16, 0x24, 0x34,
    0xE1, 0x25, 0xF1, 0x17, 0x18, 0x19, 0x1A, 0x26, 0x27, 0x28, 0x29, 0x2A, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3A, 0x43, 0x44,
    0x45, 0x46, 0x47, 0x48, 0x49, 0x4A, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5A, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68,
    0x69, 0x6A, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7A, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89, 0x8A, 0x92,
    0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9A, 0xA2, 0xA3, 0xA4, 0xA5, 0xA6, 0xA7, 0xA8, 0xA9, 0xAA, 0xB2, 0xB3, 0xB4,
    0xB5, 0xB6, 0xB7, 0xB8, 0xB9, 0xBA, 0xC2, 0xC3, 0xC4, 0xC5, 0xC6, 0xC7, 0xC8, 0xC9, 0xCA, 0xD2, 0xD3, 0xD4, 0xD5, 0xD6,
    0xD7, 0xD8, 0xD9, 0xDA, 0xE2, 0xE3, 0xE4, 0xE5, 0xE6, 0xE7, 0xE8, 0xE9, 0xEA, 0xF2, 0xF3, 0xF4, 0xF5, 0xF6, 0xF7, 0xF8,
    0xF9, 0xFA,
];

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A `w` × `h` gradient: red across, green down, half the pixels transparent when `holes`.
    fn picture(w: usize, h: usize, holes: bool) -> Vec<u8> {
        let px = |x: usize, y: usize| [(x * 255 / w) as u8, (y * 255 / h) as u8, 90, if holes && (x + y) % 2 == 0 { 0 } else { 255 }];
        (0..w * h).flat_map(|i| px(i % w, i / w)).collect()
    }

    #[test]
    fn huffman_tables_hold_every_symbol() {
        let mut ac: Vec<u8> = vec![0x00, 0xF0];
        ac.extend((0..16).flat_map(|run| (1..=10).map(move |size| run << 4 | size)));
        ac.sort();
        for (bits, vals) in [(AC_LUMA_BITS, &AC_LUMA_VALS), (AC_CHROMA_BITS, &AC_CHROMA_VALS)] {
            let mut sorted = vals.to_vec();
            sorted.sort();
            assert_eq!((sorted, bits.iter().map(|&b| b as usize).sum::<usize>()), (ac.clone(), 162));
        }
        assert_eq!(magnitude(-3), (2, 0b00));
        assert_eq!(magnitude(5), (3, 0b101));
    }

    #[test]
    fn jpeg_decodes_to_the_picture() {
        let (w, h) = (37, 21);
        let rgba = picture(w, h, false);
        let bytes = jpeg(&rgba, w as u16, h as u16, 90);
        let mut decoder = zune_jpeg::JpegDecoder::new(zune_core::bytestream::ZCursor::new(&bytes));
        let rgb = decoder.decode().unwrap();
        assert_eq!(decoder.dimensions(), Some((w, h)));
        let worst = (0..w * h).flat_map(|i| (0..3).map(move |c| (i, c))).map(|(i, c)| (rgb[i * 3 + c] as i32 - rgba[i * 4 + c] as i32).abs()).max();
        assert!(worst.unwrap() < 24, "{worst:?}");
    }

    /// A GIF's frames as a viewer shows them, walking its blocks: each drawn over what the last one
    /// left (as its disposal says), in its palette's colours (None where nothing shows).
    pub(crate) fn gif_frames(bytes: &[u8]) -> Vec<Vec<Option<[u8; 3]>>> {
        assert_eq!(&bytes[..6], b"GIF89a");
        assert_eq!(bytes[10] & 0x80, 0, "no global colour table");
        let (mut at, mut frames, mut clear, mut dispose) = (13, Vec::new(), None, false);
        let mut canvas: Vec<Option<[u8; 3]>> = Vec::new();
        let blocks = |at: &mut usize| {
            let mut data = Vec::new();
            while bytes[*at] != 0 {
                data.extend(&bytes[*at + 1..*at + 1 + bytes[*at] as usize]);
                *at += 1 + bytes[*at] as usize;
            }
            *at += 1;
            data
        };
        loop {
            match bytes[at] {
                0x21 => {
                    if bytes[at + 1] == 0xF9 {
                        dispose = (bytes[at + 3] >> 2) & 7 == 2;
                        clear = (bytes[at + 3] & 1 == 1).then_some(bytes[at + 6]);
                    }
                    at += 2;
                    blocks(&mut at);
                }
                0x2C => {
                    let packed = bytes[at + 9];
                    let colours = 1 << ((packed & 7) + 1);
                    let palette: Vec<[u8; 3]> = bytes[at + 10..at + 10 + 3 * colours].chunks(3).map(|c| [c[0], c[1], c[2]]).collect();
                    at += 10 + 3 * colours;
                    let min = bytes[at];
                    at += 1;
                    let lzw = blocks(&mut at);
                    let indices = weezl::decode::Decoder::new(weezl::BitOrder::Lsb, min).decode(&lzw).unwrap();
                    canvas.resize(indices.len(), None);
                    for (px, &i) in canvas.iter_mut().zip(&indices) {
                        if Some(i) != clear {
                            *px = Some(palette[i as usize]);
                        }
                    }
                    frames.push(canvas.clone());
                    if dispose {
                        canvas.fill(None);
                    }
                    clear = None;
                }
                0x3B => return frames,
                other => panic!("unexpected block {other:#X} at {at}"),
            }
        }
    }

    #[test]
    fn gif_frames_decode_back() {
        let (w, h) = (40, 30);
        let mut gif = Gif::new(w as u16, h as u16, None);
        // Few colours stay exact; a gradient of 1200 is cut down to 256; holes are transparent, the
        // opaque frame before them not showing through.
        let flat: Vec<u8> = (0..w * h).flat_map(|i| if i % 3 == 0 { [255, 0, 0, 255] } else { [0, 0, 255, 255] }).collect();
        for frame in [&flat, &picture(w, h, false), &picture(w, h, true)] {
            gif.frame(frame, 4);
        }
        let frames = gif_frames(&gif.finish());
        assert_eq!(frames.len(), 3);
        assert_eq!((frames[0][0], frames[0][1]), (Some([255, 0, 0]), Some([0, 0, 255])));
        let near = |a: [u8; 3], b: &[u8]| (0..3).all(|c| (a[c] as i32 - b[c] as i32).abs() <= 16);
        let gradient = picture(w, h, false);
        assert!(frames[1].iter().enumerate().all(|(i, c)| c.is_some_and(|c| near(c, &gradient[i * 4..]))));
        assert!(frames[2][0].is_none() && frames[2][1].is_some_and(|c| near(c, &gradient[4..])));
    }
}
