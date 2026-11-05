//! One picture: a 24-byte header, then a zstd stream of RGBA8 rows for the whole canvas. Most
//! pictures draw a small part of the canvas, so each is cropped to that part as it is decoded.
//! The header layout is what the sample files show; the 4 bytes at 20 are not yet identified and
//! are not checked (docs/FLIPACLIP.md).

use std::io::Read;

use super::error::FlipaError;

const SIGNATURE: [u8; 8] = *b"\x89FCI\r\n\x1a\n";
const HEADER_LEN: usize = 24;

/// The part of a canvas a picture draws: straight RGBA8, at (`x`, `y`) on the canvas.
#[derive(Debug)]
pub(super) struct Drawn {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

fn bad(entry: &str, reason: &'static str) -> FlipaError {
    FlipaError::BadPicture {
        entry: entry.to_owned(),
        reason,
    }
}

/// Decodes one picture of a `canvas`-sized project. `None` when it draws nothing. `budget` is the
/// decoded bytes still allowed across the whole import, and is reduced by this picture's size.
pub(super) fn decode(
    entry: &str,
    data: &[u8],
    canvas: (u32, u32),
    budget: &mut u64,
) -> Result<Option<Drawn>, FlipaError> {
    if data.len() < HEADER_LEN || data[..8] != SIGNATURE {
        return Err(bad(entry, "not a FlipaClip picture"));
    }
    if u16::from_le_bytes([data[8], data[9]]) != 1 {
        return Err(bad(entry, "picture version is not 1"));
    }
    if usize::from(u16::from_le_bytes([data[10], data[11]])) != HEADER_LEN {
        return Err(bad(entry, "header is not 24 bytes"));
    }
    let length = u32::from_be_bytes([0, data[12], data[13], data[14]]);
    if usize::try_from(length).ok() != Some(data.len()) {
        return Err(bad(entry, "length field does not match the entry"));
    }
    let width = u32::from(u16::from_be_bytes([data[16], data[17]]));
    let height = u32::from(u16::from_be_bytes([data[18], data[19]]));
    if (width, height) != canvas {
        return Err(bad(entry, "size differs from the canvas"));
    }
    let expected = u64::from(width) * u64::from(height) * 4;
    *budget = budget
        .checked_sub(expected)
        .ok_or(FlipaError::TooLarge("the pictures decoded together"))?;
    let decoder = zstd::stream::read::Decoder::new(&data[HEADER_LEN..])
        .map_err(|_| bad(entry, "the pixels are not a zstd stream"))?;
    let mut rgba = Vec::new();
    // One byte past the expected size, so a picture that inflates too far is seen, not trusted.
    decoder
        .take(expected + 1)
        .read_to_end(&mut rgba)
        .map_err(|_| bad(entry, "the zstd stream is corrupt"))?;
    if rgba.len() as u64 != expected {
        return Err(bad(entry, "the pixels decode to the wrong size"));
    }
    Ok(crop(&rgba, width, height))
}

/// The bounding box of the pixels that are not fully transparent, copied out and un-premultiplied.
/// The samples store premultiplied colour: no colour channel is ever above its alpha.
fn crop(rgba: &[u8], width: u32, height: u32) -> Option<Drawn> {
    let (w, h) = (width as usize, height as usize);
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0_usize, 0_usize);
    for y in 0..h {
        let row = &rgba[y * w * 4..(y + 1) * w * 4];
        let pixels = row.as_chunks::<4>().0;
        if let (Some(first), Some(last)) = (
            pixels.iter().position(|p| p[3] != 0),
            pixels.iter().rposition(|p| p[3] != 0),
        ) {
            x0 = x0.min(first);
            x1 = x1.max(last + 1);
            y0 = y0.min(y);
            y1 = y1.max(y + 1);
        }
    }
    if x0 >= x1 || y0 >= y1 {
        return None;
    }
    let (cw, ch) = (x1 - x0, y1 - y0);
    let mut out = Vec::with_capacity(cw * ch * 4);
    for y in y0..y1 {
        let start = (y * w + x0) * 4;
        out.extend_from_slice(&rgba[start..start + cw * 4]);
    }
    let table = unpremultiply_table();
    for px in out.as_chunks_mut::<4>().0 {
        unpremultiply(px, &table);
    }
    Some(Drawn {
        x: u32::try_from(x0).ok()?,
        y: u32::try_from(y0).ok()?,
        width: u32::try_from(cw).ok()?,
        height: u32::try_from(ch).ok()?,
        rgba: out,
    })
}

/// `table[alpha][colour]` is the straight colour of a premultiplied `colour` at `alpha`. Built once
/// per import: a division per channel of every edge pixel was most of an import's time.
type Table = [[u8; 256]; 256];

fn unpremultiply_table() -> Box<Table> {
    let mut table = Box::new([[0_u8; 256]; 256]);
    for (alpha, row) in table.iter_mut().enumerate().skip(1) {
        let a = alpha as u32;
        for (colour, out) in row.iter_mut().enumerate() {
            let value = (colour as u32 * 255 + a / 2) / a;
            *out = u8::try_from(value.min(255)).unwrap_or(255);
        }
    }
    table
}

fn unpremultiply(px: &mut [u8; 4], table: &Table) {
    let alpha = usize::from(px[3]);
    if alpha == 0 || alpha == 255 {
        return;
    }
    for channel in &mut px[..3] {
        *channel = table[alpha][usize::from(*channel)];
    }
}
