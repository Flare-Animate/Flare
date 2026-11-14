//! A QuickTime movie (`.mov`) is the MP4 with another brand. Both are ISO base media files: the
//! H.264 picture, the AAC sound and every box around them are the same, and what makes a file
//! QuickTime's is the `ftyp` box at its start naming `qt  ` as its brand. The brand is put into
//! the box the muxer wrote, in place and at the same size, so no offset in the file moves.

use std::io::{self, Read, Seek, SeekFrom, Write};

/// QuickTime's brand.
const QT: &[u8; 4] = b"qt  ";
/// The ISO base media brand, which the movie also is.
const ISOM: &[u8; 4] = b"isom";
/// The minor version QuickTime players expect with the `qt  ` brand.
const MINOR: u32 = 0x0000_0200;
/// Largest `ftyp` box this rewrites; the muxer's is 24 bytes.
const MAX_FTYP: usize = 64;

/// Rebrands the finished movie in `file` as QuickTime: major brand `qt  `, and `qt  ` then `isom`
/// among the compatible brands. A file that does not start with an `ftyp` box of the usual shape
/// is refused rather than changed.
pub(crate) fn rebrand<F: Read + Write + Seek>(file: &mut F) -> io::Result<()> {
    let mut header = [0u8; MAX_FTYP];
    file.seek(SeekFrom::Start(0))?;
    let read = read_up_to(file, &mut header)?;
    let size = header
        .get(0..4)
        .and_then(|b| b.try_into().ok())
        .map_or(0, |b: [u8; 4]| u32::from_be_bytes(b) as usize);
    // A box header, the major brand, the minor version and at least one compatible brand, whole
    // brands only, all within what was read.
    let usual = header.get(4..8) == Some(b"ftyp".as_slice())
        && size >= 20
        && size <= read
        && (size - 16) % 4 == 0;
    if !usual {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "the movie does not start with a file type box",
        ));
    }
    let mut ftyp = header[..size].to_vec();
    ftyp[8..12].copy_from_slice(QT);
    ftyp[12..16].copy_from_slice(&MINOR.to_be_bytes());
    for (n, brand) in ftyp[16..].as_chunks_mut::<4>().0.iter_mut().enumerate() {
        *brand = if n == 0 { *QT } else { *ISOM };
    }
    file.seek(SeekFrom::Start(0))?;
    file.write_all(&ftyp)?;
    file.flush()
}

/// Reads into `buffer` until it is full or the file ends, and says how much was read.
fn read_up_to<F: Read>(file: &mut F, buffer: &mut [u8]) -> io::Result<usize> {
    let mut filled = 0;
    while filled < buffer.len() {
        match file.read(&mut buffer[filled..])? {
            0 => break,
            n => filled += n,
        }
    }
    Ok(filled)
}
