//! A flattened Photoshop (PSD) file: one picture of RGBA at the stage's size, with its alpha as a
//! fourth channel. Photoshop's layers are not written: no permissive writer exists for them, so a
//! PSD export is the picture as one layer.

/// The largest side a PSD can have.
pub const MAX_PSD_SIDE: u32 = 30_000;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PsdError {
    #[error("a PSD is at most {MAX_PSD_SIDE} pixels on a side; this one is {0}x{1}")]
    TooLarge(u32, u32),
    #[error("the picture has {got} bytes; {width}x{height} pixels need {want}")]
    Size {
        width: u32,
        height: u32,
        got: usize,
        want: usize,
    },
}

/// The PSD bytes of `rgba` (straight alpha, row-major from the top-left) at `width` by `height`.
pub fn flattened_psd(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, PsdError> {
    if width == 0 || height == 0 || width > MAX_PSD_SIDE || height > MAX_PSD_SIDE {
        return Err(PsdError::TooLarge(width, height));
    }
    let want = width as usize * height as usize * 4;
    if rgba.len() != want {
        return Err(PsdError::Size {
            width,
            height,
            got: rgba.len(),
            want,
        });
    }
    let mut out = Vec::with_capacity(26 + 12 + 2 + want);
    out.extend_from_slice(b"8BPS");
    out.extend_from_slice(&1_u16.to_be_bytes()); // version
    out.extend_from_slice(&[0; 6]); // reserved
    out.extend_from_slice(&4_u16.to_be_bytes()); // channels: red, green, blue, alpha
    out.extend_from_slice(&height.to_be_bytes());
    out.extend_from_slice(&width.to_be_bytes());
    out.extend_from_slice(&8_u16.to_be_bytes()); // bits per channel
    out.extend_from_slice(&3_u16.to_be_bytes()); // RGB colour mode
    out.extend_from_slice(&0_u32.to_be_bytes()); // colour mode data: none
    out.extend_from_slice(&0_u32.to_be_bytes()); // image resources: none
    out.extend_from_slice(&0_u32.to_be_bytes()); // layer and mask information: none
    out.extend_from_slice(&0_u16.to_be_bytes()); // image data: uncompressed
    // The image data is planar: all red, then all green, blue and alpha.
    for channel in 0..4 {
        out.extend(rgba.as_chunks::<4>().0.iter().map(|px| px[channel]));
    }
    Ok(out)
}
