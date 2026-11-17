//! Laying one premultiplied picture over another, and fading one.

use crate::Pixels;

/// `under` becomes `top` laid over it. The two are the same size (every picture in one run of a
/// graph is); a mismatch leaves `under` as it was.
pub(crate) fn over_in_place(under: &mut Pixels, top: &Pixels) {
    if under.width() != top.width() || under.height() != top.height() {
        return;
    }
    for (dst, src) in under
        .data_mut()
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(top.data().as_chunks::<4>().0.iter())
    {
        let rest = 255 - u32::from(src[3]);
        if rest == 255 {
            continue;
        }
        for c in 0..4 {
            let kept = (u32::from(dst[c]) * rest + 127) / 255;
            dst[c] = (u32::from(src[c]) + kept).min(255) as u8;
        }
    }
}

/// Every pixel at `opacity` (0 to 1) of its strength.
pub(crate) fn fade_in_place(pixels: &mut Pixels, opacity: f64) {
    let factor = (opacity.clamp(0.0, 1.0) * 256.0).round() as u32;
    if factor >= 256 {
        return;
    }
    for value in pixels.data_mut() {
        *value = ((u32::from(*value) * factor + 128) >> 8) as u8;
    }
}
