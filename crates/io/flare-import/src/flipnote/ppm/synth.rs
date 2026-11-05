//! A PPM writer for the tests. It lays out the header, the frame table, the frames, the effect
//! flags and the sound header at the offsets the format uses. Frames are written with raw lines
//! and blank lines; the compressed line types are covered by hand-written bytes in the tests.

use super::{HEIGHT, WIDTH};

/// One frame to write: the picture it must decode to (per layer, 0 or 1 per pixel), its palette
/// and whether it is a keyframe. A delta frame is written as the XOR against `prev`.
pub(crate) struct SynthFrame {
    pub keyframe: bool,
    pub inverted: bool,
    /// Palette indices of the pens: layer 1, layer 2.
    pub pens: [u8; 2],
    /// Translation, when the frame has one.
    pub shift: Option<(i8, i8)>,
    pub layers: [Vec<u8>; 2],
}

/// The frame's bytes. `payload` replaces the XOR-encoded picture when a test wants exact bytes.
pub(crate) fn encode(
    frame: &SynthFrame,
    prev: &[Vec<u8>; 2],
    payload: Option<[Vec<u8>; 2]>,
) -> Vec<u8> {
    let (tx, ty) = frame
        .shift
        .map_or((0, 0), |(x, y)| (i64::from(x), i64::from(y)));
    let payload = payload.unwrap_or_else(|| {
        let mut out = frame.layers.clone();
        if !frame.keyframe {
            for y in ty.max(0)..(HEIGHT as i64 + ty).min(HEIGHT as i64) {
                for x in tx.max(0)..(WIDTH as i64 + tx).min(WIDTH as i64) {
                    let dest = (y * WIDTH as i64 + x) as usize;
                    let src = ((y - ty) * WIDTH as i64 + (x - tx)) as usize;
                    for k in 0..2 {
                        out[k][dest] ^= prev[k][src];
                    }
                }
            }
        }
        out
    });
    let mut header = 0_u8;
    if frame.keyframe {
        header |= 0x80;
    }
    if frame.shift.is_some() {
        header |= 0x20;
    }
    if !frame.inverted {
        header |= 0x01;
    }
    header |= pen_field(frame.pens[0]) << 1;
    header |= pen_field(frame.pens[1]) << 3;
    let mut out = vec![header];
    if let Some((x, y)) = frame.shift {
        out.push(x as u8);
        out.push(y as u8);
    }
    let mut kinds = [[0_u8; HEIGHT]; 2];
    for (k, layer) in payload.iter().enumerate() {
        for y in 0..HEIGHT {
            let row = &layer[y * WIDTH..(y + 1) * WIDTH];
            kinds[k][y] = if row.iter().any(|&p| p != 0) { 3 } else { 0 };
        }
    }
    for kind in kinds.iter() {
        for group in kind.chunks(4) {
            out.push(
                group
                    .iter()
                    .enumerate()
                    .fold(0, |b, (i, &t)| b | (t << (2 * i))),
            );
        }
    }
    for (k, layer) in payload.iter().enumerate() {
        for y in 0..HEIGHT {
            if kinds[k][y] == 0 {
                continue;
            }
            for chunk in layer[y * WIDTH..(y + 1) * WIDTH].chunks(8) {
                out.push(chunk.iter().enumerate().fold(0, |b, (i, &p)| b | (p << i)));
            }
        }
    }
    out
}

fn pen_field(index: u8) -> u8 {
    match index {
        2 => 2,
        3 => 3,
        _ => 0,
    }
}

/// Wraps encoded frames in a PPM file. `speed_byte` is the stored speed (in-app speed is
/// `8 - byte`); `bgm` is the BGM track's ADPCM bytes, header included, or empty.
pub(crate) fn wrap(
    frames: &[Vec<u8>],
    flags: &[u8],
    speed_byte: u8,
    bgm: &[u8],
    version: u16,
) -> Vec<u8> {
    let n = frames.len();
    let mut out = vec![0_u8; 0x06A0];
    out[0..4].copy_from_slice(b"PARA");
    let table_len = 4 * n;
    let frames_len: usize = frames.iter().map(Vec::len).sum();
    let frame_data_len = (0x06A8 + table_len + frames_len) - 0x06A0;
    out[4..8].copy_from_slice(&(frame_data_len as u32).to_le_bytes());
    out[8..12].copy_from_slice(&((32 + bgm.len()) as u32).to_le_bytes());
    out[12..14].copy_from_slice(&((n - 1) as u16).to_le_bytes());
    out[14..16].copy_from_slice(&version.to_le_bytes());
    out.extend_from_slice(&(table_len as u16).to_le_bytes());
    out.extend_from_slice(&[0_u8; 6]);
    let mut rel = 0_u32;
    for frame in frames {
        out.extend_from_slice(&rel.to_le_bytes());
        rel += frame.len() as u32;
    }
    for frame in frames {
        out.extend_from_slice(frame);
    }
    let mut flag_bytes = flags.to_vec();
    flag_bytes.resize(n, 0);
    out.extend_from_slice(&flag_bytes);
    while !out.len().is_multiple_of(4) {
        out.push(0);
    }
    out.extend_from_slice(&(bgm.len() as u32).to_le_bytes());
    out.extend_from_slice(&[0_u8; 12]);
    out.push(speed_byte);
    out.push(speed_byte);
    out.resize(out.len() + 14, 0);
    out.extend_from_slice(bgm);
    out
}
