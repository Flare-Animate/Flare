//! Bitmaps in the library (File › Import). Stored as PNG bytes inside the
//! document (base64 in JSON); decoded once and cached for rendering.

use base64::Engine;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

pub type BitmapId = u32;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BitmapAsset {
    pub id: BitmapId,
    pub name: String,
    pub width: u32,
    pub height: u32,
    /// Smooth when scaled (Animate's "Allow smoothing").
    #[serde(default = "yes")]
    pub smoothing: bool,
    /// The image data was missing when the document was imported: `png` is a
    /// "MISSING" checkerboard placeholder, kept so the document still lays out
    /// and exports. Survives save (.acx, .fla/XFL).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub missing: bool,
    #[serde(serialize_with = "b64_ser", deserialize_with = "b64_de")]
    pub png: Vec<u8>,
    /// Size the document declared for this bitmap when it differs from the
    /// pixel buffer (a missing 4000x3000 image gets a clamped placeholder but
    /// keeps its declared size; the renderer scales the placeholder to it).
    /// 0 = same as `width` / `height`.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub declared_width: u32,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub declared_height: u32,
    /// Character id this bitmap had in the SWF it was imported from (kept so
    /// an export reuses it; see `ac_xfl::swf`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub swf_char_id: Option<u16>,
    /// The encoded data as imported (JPEG/PNG/lossless), written back
    /// byte-identical while `png` is unedited.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original: Option<OriginalBitmap>,
}

/// How an imported bitmap was encoded.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BitmapCodec {
    /// A JPEG file (SWF DefineBitsJPEG2 / the JPEG part of JPEG3).
    Jpeg,
    /// A PNG file.
    Png,
    /// A GIF file.
    Gif,
    /// SWF DefineBitsLossless body (after the character id).
    SwfLossless,
    /// SWF DefineBitsLossless2 body (after the character id).
    SwfLossless2,
    /// SWF DefineBitsJPEG3 body (after the character id).
    SwfJpeg3,
}

/// The imported encoding of a bitmap, tied to the pixels it was decoded to
/// (`png_hash`): once the pixels change it no longer applies.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OriginalBitmap {
    pub codec: BitmapCodec,
    #[serde(serialize_with = "b64_ser", deserialize_with = "b64_de")]
    pub bytes: Vec<u8>,
    pub png_hash: u64,
}

/// FNV-1a over all bytes (change detection, not security).
pub fn fnv64(b: &[u8]) -> u64 {
    let mut h: u64 = 1469598103934665603;
    for x in b {
        h = (h ^ *x as u64).wrapping_mul(1099511628211);
    }
    h
}

fn is_zero(v: &u32) -> bool {
    *v == 0
}

fn yes() -> bool {
    true
}

pub(crate) fn b64_ser<S: Serializer>(v: &[u8], s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&base64::engine::general_purpose::STANDARD.encode(v))
}
pub(crate) fn b64_de<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
    let s = String::deserialize(d)?;
    base64::engine::general_purpose::STANDARD.decode(s.as_bytes()).map_err(serde::de::Error::custom)
}

/// Decoded, premultiplied RGBA pixels (tiny-skia's layout).
#[derive(Clone, Debug)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub smoothing: bool,
}

fn key(b: &BitmapAsset) -> (u32, usize, u64) {
    // cheap content fingerprint: length + a sample of bytes
    let mut h: u64 = 1469598103934665603;
    let step = (b.png.len() / 4096).max(1);
    for x in b.png.iter().step_by(step) {
        h = (h ^ *x as u64).wrapping_mul(1099511628211);
    }
    (b.id, b.png.len(), h ^ b.smoothing as u64)
}

impl BitmapAsset {
    /// The size layouts use: the declared one when recorded, else the pixel size.
    pub fn declared_size(&self) -> (u32, u32) {
        (if self.declared_width > 0 { self.declared_width } else { self.width }, if self.declared_height > 0 { self.declared_height } else { self.height })
    }

    /// The imported encoding, if the pixels are still the ones it decodes to.
    pub fn pristine_original(&self) -> Option<&OriginalBitmap> {
        self.original.as_ref().filter(|o| o.png_hash == fnv64(&self.png))
    }

    /// Remember `bytes` (in `codec`) as this bitmap's imported encoding.
    pub fn set_original(&mut self, codec: BitmapCodec, bytes: Vec<u8>) {
        self.original = Some(OriginalBitmap { codec, bytes, png_hash: fnv64(&self.png) });
    }

    /// Stand-in for an image whose data is gone: a magenta / black checkerboard
    /// with a white border and a red cross, so it can't be mistaken for
    /// artwork. Flagged `missing` so the Library panel and saves keep track.
    pub fn missing_placeholder(id: BitmapId, name: &str, width: u32, height: u32) -> BitmapAsset {
        let (w, h) = (width.clamp(16, 2048), height.clamp(16, 2048));
        let mut rgba = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            for x in 0..w {
                let edge = x < 2 || y < 2 || x + 2 >= w || y + 2 >= h;
                // the two diagonals, about 3 px thick
                let dx = x as f32 / w as f32;
                let dy = y as f32 / h as f32;
                let cross = ((dx - dy).abs() * w.max(h) as f32) < 1.5 || ((dx + dy - 1.0).abs() * w.max(h) as f32) < 1.5;
                let px = if edge {
                    [255, 255, 255, 255]
                } else if cross {
                    [255, 0, 0, 255]
                } else if ((x / 8) + (y / 8)) % 2 == 0 {
                    [255, 0, 255, 255]
                } else {
                    [0, 0, 0, 255]
                };
                rgba.extend_from_slice(&px);
            }
        }
        let mut a = BitmapAsset::from_rgba(id, name, w, h, &rgba).expect("placeholder encodes");
        a.missing = true;
        // the pixel buffer is clamped; the declared size is kept (the renderer scales to it)
        if (width.max(1), height.max(1)) != (w, h) {
            a.declared_width = width.max(1);
            a.declared_height = height.max(1);
        }
        a
    }

    /// Encode straight (non-premultiplied) RGBA as a new asset.
    pub fn from_rgba(id: BitmapId, name: &str, width: u32, height: u32, rgba: &[u8]) -> Result<BitmapAsset, String> {
        let mut out = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut out, width, height);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            let mut w = enc.write_header().map_err(|e| e.to_string())?;
            w.write_image_data(rgba).map_err(|e| e.to_string())?;
        }
        Ok(BitmapAsset { id, name: name.into(), width, height, smoothing: true, missing: false, png: out, declared_width: 0, declared_height: 0, swf_char_id: None, original: None })
    }

    /// Wrap PNG bytes (checked by decoding them once).
    pub fn from_png(name: &str, png: Vec<u8>) -> Result<BitmapAsset, String> {
        let img = decode_png(&png, true).ok_or("not a PNG image this build can read")?;
        Ok(BitmapAsset { id: 0, name: name.into(), width: img.width, height: img.height, smoothing: true, missing: false, png, declared_width: 0, declared_height: 0, swf_char_id: None, original: None })
    }

    /// Decoded pixels (cached across frames and undo snapshots).
    pub fn image(&self) -> Option<Arc<Image>> {
        if let Some(live) = live_pixels(self.id) {
            return Some(live);
        }
        type Cache = Mutex<HashMap<(u32, usize, u64), Arc<Image>>>;
        static CACHE: OnceLock<Cache> = OnceLock::new();
        let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
        let k = key(self);
        if let Some(i) = cache.lock().ok()?.get(&k) {
            return Some(i.clone());
        }
        let img = Arc::new(decode_png(&self.png, self.smoothing)?);
        let mut c = cache.lock().ok()?;
        if c.len() > 256 {
            c.clear();
        }
        c.insert(k, img.clone());
        Some(img)
    }
}

type Live = Mutex<HashMap<BitmapId, Arc<Image>>>;
fn live() -> &'static Live {
    static LIVE: OnceLock<Live> = OnceLock::new();
    LIVE.get_or_init(|| Mutex::new(HashMap::new()))
}
/// While a raster brush stroke is in progress its bitmap shows these pixels
/// (no PNG round trip per mouse move); `None` ends the override.
pub fn set_live_pixels(id: BitmapId, img: Option<Arc<Image>>) {
    if let Ok(mut m) = live().lock() {
        match img {
            Some(i) => {
                m.insert(id, i);
            }
            None => {
                m.remove(&id);
            }
        }
    }
}
fn live_pixels(id: BitmapId) -> Option<Arc<Image>> {
    live().lock().ok()?.get(&id).cloned()
}

fn decode_png(bytes: &[u8], smoothing: bool) -> Option<Image> {
    let mut dec = png::Decoder::new(bytes);
    dec.set_transformations(png::Transformations::normalize_to_color8() | png::Transformations::ALPHA);
    let mut r = dec.read_info().ok()?;
    let mut buf = vec![0; r.output_buffer_size()];
    let info = r.next_frame(&mut buf).ok()?;
    let (w, h) = (info.width, info.height);
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    match info.color_type {
        png::ColorType::Rgba => rgba.extend_from_slice(&buf[..(w * h * 4) as usize]),
        png::ColorType::GrayscaleAlpha => {
            for px in buf[..(w * h * 2) as usize].chunks(2) {
                rgba.extend_from_slice(&[px[0], px[0], px[0], px[1]]);
            }
        }
        _ => return None,
    }
    for px in rgba.chunks_mut(4) {
        let a = px[3] as u16;
        for c in &mut px[..3] {
            *c = ((*c as u16 * a + 127) / 255) as u8;
        }
    }
    Some(Image { width: w, height: h, rgba, smoothing })
}
