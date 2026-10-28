//! Image file decoding for bitmap import (feature `codecs`, on by default):
//! PNG, JPEG, GIF, BMP, TGA, TIFF, WebP and OpenEXR through the `image` crate
//! (MIT/Apache-2.0). PNGs are kept byte for byte; everything else is stored
//! as PNG. Floating-point images (EXR, 32-bit TIFF) are linear light: they are
//! encoded to sRGB and clipped to 0..1 (no tone mapping).

use crate::bitmap::BitmapAsset;

/// File extensions this build can import as bitmaps (lower case).
pub const EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "bmp", "tga", "tif", "tiff", "webp", "exr"];

/// Is `path`'s extension one of [`EXTENSIONS`]?
pub fn is_image(path: &std::path::Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

fn srgb(v: f32) -> u8 {
    let v = v.clamp(0.0, 1.0);
    let s = if v <= 0.003_130_8 { v * 12.92 } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 };
    (s * 255.0).round() as u8
}

/// Decode an image file's bytes into straight RGBA8 (width, height, pixels).
/// `name` (a file name) settles formats without a signature, such as TGA.
pub fn decode_rgba(name: &str, bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
    let img = match image::load_from_memory(bytes) {
        Ok(i) => i,
        Err(e) => match image::ImageFormat::from_path(name) {
            Ok(f) => image::load_from_memory_with_format(bytes, f).map_err(|e| e.to_string())?,
            Err(_) => return Err(e.to_string()),
        },
    };
    let (w, h) = (img.width(), img.height());
    use image::ColorType::*;
    let px = match img.color() {
        Rgb32F | Rgba32F => img.to_rgba32f().pixels().flat_map(|p| [srgb(p[0]), srgb(p[1]), srgb(p[2]), (p[3].clamp(0.0, 1.0) * 255.0).round() as u8]).collect(),
        _ => img.to_rgba8().into_raw(),
    };
    Ok((w, h, px))
}

/// A library bitmap from an image file's bytes (`name` is shown in the Library).
pub fn bitmap_from_bytes(name: &str, bytes: Vec<u8>) -> Result<BitmapAsset, String> {
    if bytes.starts_with(b"\x89PNG") {
        if let Ok(a) = BitmapAsset::from_png(name, bytes.clone()) {
            return Ok(a);
        }
    }
    let (w, h, px) = decode_rgba(name, &bytes).map_err(|e| format!("{name}: {e}"))?;
    BitmapAsset::from_rgba(0, name, w, h, &px)
}

/// A library bitmap from an image file (named after the file).
pub fn load_bitmap(path: &std::path::Path) -> Result<BitmapAsset, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "Bitmap".into());
    bitmap_from_bytes(&name, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode(fmt: image::ImageFormat, img: image::DynamicImage) -> Vec<u8> {
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, fmt).unwrap();
        out.into_inner()
    }

    #[test]
    fn reads_every_listed_format() {
        let rgba = image::RgbaImage::from_fn(6, 4, |x, _| if x < 3 { image::Rgba([255, 0, 0, 255]) } else { image::Rgba([0, 0, 255, 255]) });
        for fmt in [image::ImageFormat::Png, image::ImageFormat::Bmp, image::ImageFormat::Tga, image::ImageFormat::Tiff, image::ImageFormat::WebP, image::ImageFormat::Gif] {
            let name = format!("t.{}", fmt.extensions_str()[0]);
            let a = bitmap_from_bytes(&name, encode(fmt, image::DynamicImage::ImageRgba8(rgba.clone()))).unwrap_or_else(|e| panic!("{fmt:?}: {e}"));
            let img = a.image().unwrap();
            assert_eq!((img.width, img.height), (6, 4), "{fmt:?}");
        }
        // linear float (EXR) → sRGB: 0.5 linear ≈ 188
        let f = image::Rgba32FImage::from_pixel(2, 2, image::Rgba([0.5, 1.0, 4.0, 1.0]));
        let (_, _, px) = decode_rgba("t.exr", &encode(image::ImageFormat::OpenExr, image::DynamicImage::ImageRgba32F(f))).unwrap();
        assert_eq!(&px[..4], &[188, 255, 255, 255]);
        assert!(bitmap_from_bytes("x", b"not an image".to_vec()).is_err());
    }
}
