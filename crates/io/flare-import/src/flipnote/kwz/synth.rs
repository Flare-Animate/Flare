//! A KWZ writer for the tests: the sections, the frame records and the LSB-first bit streams of
//! the layers, laid out as the format has them. Blocks are written in the order the decoder
//! reads them, so a test chooses each block's code.

/// The number of 8 by 8 blocks in a 320 by 240 layer.
pub(crate) const BLOCKS: usize = 40 * 30;

/// Writes bits LSB-first into little-endian 16-bit words.
#[derive(Default)]
pub(crate) struct BitWriter {
    out: Vec<u8>,
    acc: u32,
    n: u32,
}

impl BitWriter {
    pub(crate) fn put(&mut self, value: u32, bits: u32) {
        for i in 0..bits {
            self.acc |= ((value >> i) & 1) << self.n;
            self.n += 1;
            if self.n == 16 {
                self.out.extend_from_slice(&(self.acc as u16).to_le_bytes());
                self.acc = 0;
                self.n = 0;
            }
        }
    }

    pub(crate) fn finish(mut self) -> Vec<u8> {
        if self.n > 0 {
            self.out.extend_from_slice(&(self.acc as u16).to_le_bytes());
        }
        self.out
    }
}

/// The code of a line of eight values 0 to 2, as the table has it: digits a, b, c, d, e, f, g, h
/// with a the most significant, where the pixels are `[b, a, d, c, f, e, h, g]`.
pub(crate) fn line_code(pixels: [u8; 8]) -> u32 {
    let [p0, p1, p2, p3, p4, p5, p6, p7] = pixels;
    let digits = [p1, p0, p3, p2, p5, p4, p7, p6];
    digits.iter().fold(0_u32, |acc, &d| acc * 3 + u32::from(d))
}

/// Block codes for a layer of blank blocks: type 0 with common line 0 (all zero).
pub(crate) fn blank_blocks() -> Vec<Vec<(u32, u32)>> {
    vec![vec![(0, 3), (0, 5)]; BLOCKS]
}

/// A layer's bit stream from its block codes, in decode order.
pub(crate) fn layer(blocks: &[Vec<(u32, u32)>]) -> Vec<u8> {
    let mut w = BitWriter::default();
    for block in blocks {
        for &(value, bits) in block {
            w.put(value, bits);
        }
    }
    let mut out = w.finish();
    if out.len() % 2 == 1 {
        out.push(0);
    }
    out
}

/// One frame for the writer: the palette flag word, and the three layers' bytes. A layer of 38
/// bytes is the format's "unchanged" marker.
pub(crate) struct Frame {
    pub(crate) flags: u32,
    pub(crate) layers: [Vec<u8>; 3],
}

pub(crate) struct Options {
    pub(crate) speed: u8,
    pub(crate) layer_flags: u8,
    /// Frame count written in the header; `None` writes the number of frames.
    pub(crate) frame_count: Option<u16>,
    /// Sound: the BGM speed and the five track lengths. `None` writes no KSN section (a comment).
    pub(crate) sound: Option<(u32, [u32; 5])>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            speed: 2,
            layer_flags: 0,
            frame_count: None,
            sound: Some((0, [0; 5])),
        }
    }
}

fn section(magic: &[u8; 3], data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + data.len());
    out.extend_from_slice(magic);
    out.push(0x14);
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(data);
    out
}

/// Builds a whole KWZ file: KFH, KTN, KMC, KMI and (unless a comment) KSN, then the signature.
pub(crate) fn kwz(frames: &[Frame], options: &Options) -> Vec<u8> {
    // KFH: 0xD0 bytes after its 8-byte prefix, so the fields sit at section offsets 8 and up.
    let mut kfh = vec![0_u8; 0xD0];
    let count = options.frame_count.unwrap_or(frames.len() as u16);
    // The fields are at section offsets 0xCC (count), 0xD2 (speed) and 0xD3 (layer flags).
    kfh[0xCC - 8..0xCC - 6].copy_from_slice(&count.to_le_bytes());
    kfh[0xD2 - 8] = options.speed;
    kfh[0xD3 - 8] = options.layer_flags;
    let mut kmc_data = vec![0_u8; 4];
    let mut kmi_data = Vec::new();
    for frame in frames {
        kmi_data.extend_from_slice(&frame.flags.to_le_bytes());
        for layer in &frame.layers {
            kmi_data.extend_from_slice(&(layer.len() as u16).to_le_bytes());
        }
        kmi_data.extend_from_slice(&[0_u8; 28 - 4 - 6]);
        for layer in &frame.layers {
            kmc_data.extend_from_slice(layer);
        }
    }
    let mut out = section(b"KFH", &kfh);
    out.extend(section(b"KTN", &[]));
    out.extend(section(b"KMC", &kmc_data));
    out.extend(section(b"KMI", &kmi_data));
    if let Some((bgm_speed, tracks)) = options.sound {
        let mut ksn = Vec::new();
        ksn.extend_from_slice(&bgm_speed.to_le_bytes());
        for t in tracks {
            ksn.extend_from_slice(&t.to_le_bytes());
        }
        // Four bytes of padding put the first track at offset 28 of the section data.
        ksn.extend_from_slice(&[0_u8; 4]);
        let total: usize = tracks.iter().map(|&t| t as usize).sum();
        ksn.extend(std::iter::repeat_n(0_u8, total));
        out.extend(section(b"KSN", &ksn));
    }
    out.extend(std::iter::repeat_n(0_u8, 256));
    out
}
