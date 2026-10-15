//! Bitmaps (docs/animate-ui.md §7.3, §7.4): the library's bitmap items (XFL's
//! DOMBitmapItem, in the media list), bitmaps on stage (DOMBitmapInstance) and
//! bitmap fills (BitmapFill). An item keeps its image file as imported or read,
//! PNG, JPEG or GIF bytes or the bitmap data Animate keeps in the archive's bin/
//! folder, and decodes it once, on first use. trace.rs vectorizes them.

use std::borrow::Cow;
use std::collections::HashSet;
use std::fmt;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::{Arc, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use roxmltree::Node;
use tiny_skia::{self as sk, FilterQuality, IntSize, Pixmap, SpreadMode, Transform};

use crate::Result;
use crate::doc::*;
use crate::xfl;
use crate::xfl_write::{self, Xml};

/// An item of the document's media list.
#[derive(Clone, Debug, PartialEq)]
pub enum Media {
    Bitmap(BitmapItem),
    /// Sounds, video and so on: the XFL element's name and its XML, saved back as is.
    Unsupported { name: String, xml: String },
}

/// A bitmap of the library (XFL's DOMBitmapItem).
#[derive(Clone, Debug, PartialEq)]
pub struct BitmapItem {
    /// Its library path, folders and all: "Photos/sky.jpg".
    pub name: String,
    pub data: BitmapData,
    /// Allow Smoothing: scaled or turned, it's drawn smoothed rather than in blocks.
    pub smoothing: bool,
    /// Compression: lossless (PNG/GIF) rather than a photo (JPEG).
    pub lossless: bool,
    /// A photo's JPEG quality, 0..=100; None leaves it to the publish settings.
    pub quality: Option<u8>,
    /// What the model doesn't read of the DOMBitmapItem: its files' names in the XFL,
    /// the file it was imported from, linkage...
    pub extra: Extra,
}

/// A library bitmap on stage (DOMBitmapInstance): its pixels cover (0, 0) to its
/// size, one unit each, placed by `matrix`.
#[derive(Clone, Debug, PartialEq)]
pub struct BitmapInstance {
    /// The library item's name.
    pub name: String,
    pub matrix: Matrix,
    pub extra: Extra,
}

/// A bitmap fill (XFL's BitmapFill).
#[derive(Clone, Debug, PartialEq)]
pub struct BitmapFill {
    /// The library item's name.
    pub name: String,
    /// From the bitmap's pixels to the shape's coordinates.
    pub matrix: Matrix,
    /// Past its edges the bitmap's edge pixels go on, where a tiled one repeats.
    pub clipped: bool,
    pub extra: Extra,
}

/// A bitmap's image file, shared by the copies of the document (undo keeps one
/// a step) and decoded on first use.
#[derive(Clone)]
pub struct BitmapData {
    bytes: Arc<[u8]>,
    pixels: Arc<OnceLock<Option<Pixmap>>>,
}

impl BitmapData {
    pub fn new(bytes: Vec<u8>) -> BitmapData {
        BitmapData { bytes: bytes.into(), pixels: Arc::default() }
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn format(&self) -> Option<Format> {
        Format::of(&self.bytes)
    }

    /// The image, premultiplied as tiny-skia draws it; None if it can't be read.
    pub fn pixels(&self) -> Option<&Pixmap> {
        self.pixels.get_or_init(|| decode(&self.bytes)).as_ref()
    }
}

impl PartialEq for BitmapData {
    fn eq(&self, other: &BitmapData) -> bool {
        Arc::ptr_eq(&self.bytes, &other.bytes) || self.bytes == other.bytes
    }
}

impl fmt::Debug for BitmapData {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "BitmapData({:?}, {} bytes)", self.format(), self.bytes.len())
    }
}

/// What a bitmap's bytes hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Png,
    Jpeg,
    Gif,
    /// Animate's own lossless bitmap data (bin/*.dat).
    Lossless,
}

impl Format {
    pub fn of(b: &[u8]) -> Option<Format> {
        Some(match b {
            [0x89, b'P', b'N', b'G', ..] => Format::Png,
            [0xFF, 0xD8, ..] => Format::Jpeg,
            [b'G', b'I', b'F', b'8', ..] => Format::Gif,
            [3, 5, ..] => Format::Lossless,
            _ => return None,
        })
    }
}

impl BitmapItem {
    /// A bitmap of image file `bytes`, as Animate imports one: smoothed, a JPEG as a
    /// photo and anything else lossless.
    pub fn new(name: &str, bytes: Vec<u8>) -> BitmapItem {
        let data = BitmapData::new(bytes);
        let photo = data.format() == Some(Format::Jpeg);
        let mut extra = Extra::default();
        // ponytail: JSFL's names for the original compression; unconfirmed in Animate's XFL.
        extra.set("originalCompressionType", Some(if photo { "photo" } else { "lossless" }));
        // ponytail: smoothing on, as recalled of Animate 2024's imports.
        BitmapItem { name: name.into(), data, smoothing: true, lossless: !photo, quality: None, extra }
    }

    /// Its size in pixels: the image's, else the size its DOMBitmapItem gave.
    pub fn size(&self) -> Option<(u32, u32)> {
        if let Some(p) = self.data.pixels() {
            return Some((p.width(), p.height()));
        }
        let px = |a: &str| Some((self.extra.get(a)?.parse::<f32>().ok()? / 20.0).round() as u32);
        Some((px("frameRight")?, px("frameBottom")?))
    }
}

impl Document {
    /// The library bitmap named `name`.
    pub fn bitmap(&self, name: &str) -> Option<&BitmapItem> {
        self.bitmaps().find(|b| b.name == name)
    }

    pub fn bitmap_mut(&mut self, name: &str) -> Option<&mut BitmapItem> {
        self.media.iter_mut().find_map(|m| match m {
            Media::Bitmap(b) if b.name == name => Some(b),
            _ => None,
        })
    }

    /// The library's bitmaps, in the media list's order.
    pub fn bitmaps(&self) -> impl Iterator<Item = &BitmapItem> {
        self.media.iter().filter_map(|m| match m {
            Media::Bitmap(b) => Some(b),
            Media::Unsupported { .. } => None,
        })
    }
}

/// `wanted`, or that with " copy" (then " copy 2"...) before its extension
/// while a library item has the name, case aside.
pub fn free_name(doc: &Document, wanted: &str) -> String {
    let taken = |n: &str| doc.library.keys().chain(doc.bitmaps().map(|b| &b.name)).any(|k| k.eq_ignore_ascii_case(n));
    let (stem, ext) = match wanted.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() && !ext.contains('/') => (stem, format!(".{ext}")),
        _ => (wanted, String::new()),
    };
    let copies = (1..).map(|n| if n == 1 { format!("{stem} copy{ext}") } else { format!("{stem} copy {n}{ext}") });
    std::iter::once(wanted.to_string()).chain(copies).find(|n| !taken(n)).unwrap_or_default()
}

/// File > Import: the image file at `path` (PNG, JPEG or GIF) as a new library
/// bitmap named after the file (`free_name`). Returns its name.
// ponytail: a name in use gets " copy" where Animate asks to replace the item; and an image
// sequence (a numbered name) comes in alone, without Animate's offer to import the rest.
pub fn import(doc: &mut Document, path: &Path) -> Result<String> {
    let bytes = std::fs::read(path)?;
    let file = path.file_name().map_or("Bitmap".into(), |n| n.to_string_lossy().into_owned());
    let mut b = BitmapItem::new(&free_name(doc, &file), bytes);
    if b.data.pixels().is_none() {
        return Err(format!("{file} isn't a PNG, JPEG or GIF image minnamate can read.").into());
    }
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs()).to_string();
    // ponytail: the source path is kept as given (absolute from the editor), where Animate keeps it
    // relative to the document; `update` reads either.
    b.extra.set("sourceExternalFilepath", Some(&path.to_string_lossy()));
    b.extra.set("sourceLastImported", Some(&now));
    b.extra.set("externalFileSize", Some(&b.data.bytes().len().to_string()));
    let name = b.name.clone();
    doc.media.push(Media::Bitmap(b));
    Ok(name)
}

/// The Library's Update: reads bitmap `b` again from the file it was imported
/// from (relative to `base`, the document's folder, if it says so). Returns
/// whether that changed it.
pub fn update(b: &mut BitmapItem, base: Option<&Path>) -> Result<bool> {
    let source = b.extra.get("sourceExternalFilepath").ok_or("it wasn't imported from a file")?;
    let path = base.map_or_else(|| Path::new(source).to_path_buf(), |dir| dir.join(source));
    let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    if bytes[..] == *b.data.bytes() {
        return Ok(false);
    }
    let data = BitmapData::new(bytes);
    if data.pixels().is_none() {
        return Err(format!("{} isn't an image minnamate can read.", path.display()).into());
    }
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs()).to_string();
    b.extra.set("sourceLastImported", Some(&now));
    b.extra.set("externalFileSize", Some(&data.bytes().len().to_string()));
    b.data = data;
    Ok(true)
}

// ---- Decoding ----------------------------------------------------------------------------

fn decode(b: &[u8]) -> Option<Pixmap> {
    match Format::of(b)? {
        Format::Png => Pixmap::decode_png(b).ok(),
        Format::Jpeg => jpeg(b),
        Format::Gif => gif(b),
        Format::Lossless => lossless(b),
    }
}

fn jpeg(b: &[u8]) -> Option<Pixmap> {
    use zune_jpeg::zune_core::{bytestream::ZCursor, colorspace::ColorSpace, options::DecoderOptions};
    let options = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::RGBA);
    let mut decoder = zune_jpeg::JpegDecoder::new_with_options(ZCursor::new(b), options);
    let rgba = decoder.decode().ok()?;
    let (w, h) = decoder.dimensions()?;
    Pixmap::from_vec(rgba, IntSize::from_wh(w as u32, h as u32)?)
}

fn u16le(b: &[u8], i: usize) -> Option<usize> {
    Some(u16::from_le_bytes([*b.get(i)?, *b.get(i + 1)?]) as usize)
}

/// A GIF colour table at `at`, if `flags` says there's one (2^(n+1) colours of 3 bytes), and moves past it.
fn table<'b>(b: &'b [u8], at: &mut usize, flags: u8) -> Option<&'b [u8]> {
    let len = if flags & 0x80 != 0 { 3 << ((flags & 7) + 1) } else { 0 };
    let t = b.get(*at..*at + len)?;
    *at += len;
    Some(t)
}

/// A GIF's first image, on a clear canvas the size of its screen.
// ponytail: an animated GIF gives its first frame, where Animate imports every frame, numbered,
// into a folder of their own.
fn gif(b: &[u8]) -> Option<Pixmap> {
    // The data sub-blocks from `at`, each led by its length, up to a 0: their bytes, and where they end.
    let blocks = |mut at: usize| -> Option<(Vec<u8>, usize)> {
        let mut data = Vec::new();
        loop {
            let len = *b.get(at)? as usize;
            if len == 0 {
                return Some((data, at + 1));
            }
            data.extend_from_slice(b.get(at + 1..at + 1 + len)?);
            at += 1 + len;
        }
    };
    let (w, h) = (u16le(b, 6)?, u16le(b, 8)?);
    let mut at = 13;
    let global = table(b, &mut at, *b.get(10)?)?;
    let mut clear = None;
    loop {
        match *b.get(at)? {
            // An extension: a Graphic Control one says which colour is see-through.
            0x21 => {
                let (data, end) = blocks(at + 2)?;
                if *b.get(at + 1)? == 0xF9 && data.len() >= 4 && data[0] & 1 != 0 {
                    clear = Some(data[3]);
                }
                at = end;
            }
            0x2C => {
                let (x0, y0, iw, ih) = (u16le(b, at + 1)?, u16le(b, at + 3)?, u16le(b, at + 5)?, u16le(b, at + 7)?);
                let flags = *b.get(at + 9)?;
                at += 10;
                let local = table(b, &mut at, flags)?;
                let colors = if flags & 0x80 != 0 { local } else { global };
                let code_size = *b.get(at)?;
                if !(2..=8).contains(&code_size) {
                    return None;
                }
                let (lzw, _) = blocks(at + 1)?;
                let mut indices = Vec::new();
                // A stream cut short still gives what it holds.
                let _ = weezl::decode::Decoder::new(weezl::BitOrder::Lsb, code_size).into_vec(&mut indices).decode(&lzw);
                // Interlaced rows come every 8th from 0, every 8th from 4, every 4th from 2, then the odd ones.
                let rows: Vec<usize> = match flags & 0x40 {
                    0 => (0..ih).collect(),
                    _ => [(0, 8), (4, 8), (2, 4), (1, 2)].into_iter().flat_map(|(from, step)| (from..ih).step_by(step)).collect(),
                };
                let mut pm = Pixmap::new(w as u32, h as u32)?;
                let px = pm.data_mut();
                for (k, &row) in rows.iter().enumerate() {
                    for col in 0..iw {
                        let (x, y) = (x0 + col, y0 + row);
                        let Some(&i) = indices.get(k * iw + col) else { break };
                        let Some(rgb) = colors.get(i as usize * 3..i as usize * 3 + 3) else { continue };
                        if Some(i) != clear && x < w && y < h {
                            px[(y * w + x) * 4..][..4].copy_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
                        }
                    }
                }
                return Some(pm);
            }
            _ => return None,
        }
    }
}

/// Animate's lossless bitmap data (bin/*.dat), as public notes on the format
/// describe it: 03 05; the row length in bytes, width and height (u16 LE each);
/// the frame in twips (left, right, top, bottom: i32 LE); an alpha flag and a
/// compressed flag; then premultiplied ARGB rows, as zlib data in chunks each led
/// by its u16 LE length, up to an empty one.
// ponytail: unconfirmed against Animate, and its 8-bit palette kind isn't read (it stays as
// read, unshown). Check against an Animate .fla's bin/ when one is at hand.
const MAX_SIDE: usize = 16384;
const MAX_BYTES: usize = 512 << 20;

fn lossless(b: &[u8]) -> Option<Pixmap> {
    let (row, w, h) = (u16le(b, 2)?, u16le(b, 4)?, u16le(b, 6)?);
    let (alpha, compressed) = (*b.get(24)? != 0, *b.get(25)? != 0);
    if row != w * 4 {
        return None;
    }
    // Capped before anything is allocated: the header is the file's to set.
    let size = w.checked_mul(h).and_then(|n| n.checked_mul(4))?;
    if w > MAX_SIDE || h > MAX_SIDE || size > MAX_BYTES {
        return None;
    }
    let mut argb = Vec::new();
    if compressed {
        let (mut z, mut at) = (Vec::new(), 26);
        loop {
            let len = u16le(b, at)?;
            if len == 0 {
                break;
            }
            z.extend_from_slice(b.get(at + 2..at + 2 + len)?);
            at += 2 + len;
        }
        // Bounded to what the header says it holds (plus a little slack, to tell "too long").
        flate2::read::ZlibDecoder::new(&z[..]).take(size as u64 + 16).read_to_end(&mut argb).ok()?;
        if argb.len() > size {
            return None;
        }
    } else {
        argb = b.get(26..)?.to_vec();
    }
    let rgba = argb.get(..row * h)?.chunks_exact(4).flat_map(|p| {
        let a = if alpha { p[0] } else { 255 };
        [p[1].min(a), p[2].min(a), p[3].min(a), a]
    });
    Pixmap::from_vec(rgba.collect(), IntSize::from_wh(w as u32, h as u32)?)
}

/// `pm` as Animate's lossless bitmap data (see `lossless`).
fn to_lossless(pm: &Pixmap) -> Option<Vec<u8>> {
    let row = u16::try_from(pm.width() * 4).ok()?;
    let (w, h) = (row / 4, u16::try_from(pm.height()).ok()?);
    let mut out = vec![3, 5];
    [row, w, h].iter().for_each(|v| out.extend(v.to_le_bytes()));
    [0, i32::from(w) * 20, 0, i32::from(h) * 20].iter().for_each(|v| out.extend(v.to_le_bytes()));
    out.extend([pm.pixels().iter().any(|p| p.alpha() < 255) as u8, 1]);
    for chunk in zlib(&argb(pm)).chunks(2048) {
        out.extend((chunk.len() as u16).to_le_bytes());
        out.extend_from_slice(chunk);
    }
    out.extend([0, 0]);
    Some(out)
}

/// Premultiplied ARGB, as SWF and Animate store lossless bitmaps.
fn argb(pm: &Pixmap) -> Vec<u8> {
    pm.data().chunks_exact(4).flat_map(|p| [p[3], p[0], p[1], p[2]]).collect()
}

fn zlib(data: &[u8]) -> Vec<u8> {
    let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    // Into memory, which can't fail.
    let _ = z.write_all(data);
    z.finish().unwrap_or_default()
}

// ---- XFL ---------------------------------------------------------------------------------

/// The attributes of a DOMBitmapItem the model reads. Saving writes them, and
/// its files' names and frame (size in twips).
const ITEM_ATTRS: &[&str] = &["name", "allowSmoothing", "compressionType", "quality"];
/// What an image that can be read makes of its DOMBitmapItem: saving writes them afresh.
const FILE_ATTRS: &[&str] = &["href", "bitmapDataHRef", "frameRight", "frameBottom"];

/// The items of a `<media>` list. `read` reads a file of the XFL by its path in it.
pub(crate) fn read_media(media: Node, read: &mut dyn FnMut(&str) -> Result<Vec<u8>>) -> Result<Vec<Media>> {
    let items = media.children().filter(Node::is_element).map(|n| {
        Ok(match n.tag_name().name() {
            "DOMBitmapItem" => Media::Bitmap(read_item(n, read)?),
            other => Media::Unsupported { name: other.into(), xml: xfl::source(n) },
        })
    });
    items.collect()
}

fn read_item(n: Node, read: &mut dyn FnMut(&str) -> Result<Vec<u8>>) -> Result<BitmapItem> {
    // The image as imported (in LIBRARY/, in XFL folders), else Animate's copy in bin/;
    // one minnamate can't read is kept, to save back.
    let paths = [("LIBRARY", "href"), ("bin", "bitmapDataHRef")].map(|(dir, a)| n.attribute(a).filter(|h| safe(h)).map(|h| format!("{dir}/{h}")));
    let found: Vec<Vec<u8>> = paths.into_iter().flatten().filter_map(|p| read(&p).ok()).filter(|b| !b.is_empty()).collect();
    let bytes = found.iter().find(|b| Format::of(b).is_some()).or(found.first());
    let data = BitmapData::new(bytes.cloned().unwrap_or_default());
    // ponytail: without compressionType, a JPEG is taken for a photo and the rest for lossless.
    let lossless = match n.attribute("compressionType") {
        Some(c) => c == "lossless",
        None => data.format() != Some(Format::Jpeg),
    };
    // An image that can't be read keeps its files' names and size, to be saved back as it was.
    let owned = if data.pixels().is_some() { [ITEM_ATTRS, FILE_ATTRS].concat() } else { ITEM_ATTRS.to_vec() };
    let extra = xfl::extra(n, &owned, &[]);
    Ok(BitmapItem {
        name: xfl::attr(n, "name")?.into(),
        smoothing: n.attribute("allowSmoothing") == Some("true"),
        lossless,
        quality: n.attribute("quality").and_then(|q| q.parse().ok()).filter(|&q: &u8| q <= 100),
        data,
        extra,
    })
}

pub(crate) fn read_instance(n: Node) -> Result<BitmapInstance> {
    Ok(BitmapInstance {
        name: xfl::attr(n, "libraryItemName")?.into(),
        matrix: xfl::matrix(n)?,
        extra: xfl::extra(n, &["libraryItemName"], &["matrix"]),
    })
}

/// A `<BitmapFill>`. Its matrix is SWF's, from bitmap pixels to twips, but for a
/// translation in pixels: 20 times the model's, which goes to pixels.
// ponytail: that scale is from Animate's XFL as seen in files, unconfirmed by a spec.
pub(crate) fn read_fill(n: Node) -> Result<BitmapFill> {
    let m = xfl::matrix(n)?;
    Ok(BitmapFill {
        name: xfl::attr(n, "bitmapPath")?.into(),
        matrix: Matrix { a: m.a / 20.0, b: m.b / 20.0, c: m.c / 20.0, d: m.d / 20.0, ..m },
        clipped: n.attribute("bitmapIsClipped") == Some("true"),
        extra: xfl::extra(n, &["bitmapPath", "bitmapIsClipped"], &["matrix"]),
    })
}

/// The `<media>` list, if the document has media.
/// The `<media>` list: the bitmaps and kept items, then `sounds` (sound.rs's items, as XML).
pub(crate) fn write_media(x: &mut Xml, media: &[Media], sounds: &[String]) {
    if media.is_empty() && sounds.is_empty() {
        return;
    }
    x.el("media", &[], |x| {
        for (m, files) in media.iter().zip(hrefs(media)) {
            match m {
                Media::Bitmap(b) => write_item(x, b, files.unwrap_or_default()),
                Media::Unsupported { xml, .. } => x.raw(xml),
            }
        }
        sounds.iter().for_each(|s| x.raw(s));
    });
}

fn write_item(x: &mut Xml, b: &BitmapItem, (href, bin): (String, String)) {
    let mut attrs = vec![("name", b.name.clone())];
    if b.smoothing {
        attrs.push(("allowSmoothing", "true".into()));
    }
    attrs.push(("compressionType", if b.lossless { "lossless" } else { "photo" }.into()));
    attrs.extend(b.quality.map(|q| ("quality", q.to_string())));
    let mut owned = ITEM_ATTRS.to_vec();
    // File names the item keeps stay where they were.
    for (a, v) in [("href", href), ("bitmapDataHRef", bin)] {
        if b.extra.get(a) != Some(v.as_str()) {
            attrs.push((a, v));
            owned.push(a);
        }
    }
    if let Some(p) = b.data.pixels() {
        attrs.extend([("frameRight", (p.width() * 20).to_string()), ("frameBottom", (p.height() * 20).to_string())]);
        owned.extend(["frameRight", "frameBottom"]);
    }
    x.kept("DOMBitmapItem", &attrs, &b.extra, &owned, |_| {});
}

pub(crate) fn write_instance(x: &mut Xml, b: &BitmapInstance) {
    x.kept("DOMBitmapInstance", &[("libraryItemName", b.name.clone())], &b.extra, &["libraryItemName"], |x| xfl_write::matrix(x, &b.matrix));
}

pub(crate) fn write_fill(x: &mut Xml, f: &BitmapFill) {
    let mut attrs = vec![("bitmapPath", f.name.clone())];
    if f.clipped {
        attrs.push(("bitmapIsClipped", "true".into()));
    }
    let m = f.matrix;
    let k = |v: f32| xfl_write::unscale(v, 20.0);
    let file = [("a", k(m.a), 1.0), ("b", k(m.b), 0.0), ("c", k(m.c), 0.0), ("d", k(m.d), 1.0), ("tx", m.tx, 0.0), ("ty", m.ty, 0.0)];
    let file = xfl_write::nums(&file);
    x.kept("BitmapFill", &attrs, &f.extra, &["bitmapPath", "bitmapIsClipped"], |x| {
        if !file.is_empty() {
            x.el("matrix", &[], |x| x.el("Matrix", &file, |_| {}));
        }
    });
}

/// The files saving writes for the document's bitmaps, by path in the XFL: each
/// image file as imported, under LIBRARY/, and Animate's copy in bin/ (a JPEG as
/// it is, others as its lossless data).
pub(crate) fn files(media: &[Media]) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    for (m, files) in media.iter().zip(hrefs(media)) {
        let (Media::Bitmap(b), Some((href, bin))) = (m, files) else { continue };
        let (library, bin) = (format!("LIBRARY/{href}"), format!("bin/{bin}"));
        let bytes = b.data.bytes().to_vec();
        match b.data.format() {
            Some(Format::Lossless) => out.push((bin, bytes)),
            Some(Format::Jpeg) => out.extend([(library, bytes.clone()), (bin, bytes)]),
            Some(Format::Png | Format::Gif) => {
                out.extend(b.data.pixels().and_then(to_lossless).map(|dat| (bin, dat)));
                out.push((library, bytes));
            }
            // Bytes minnamate can't read stay where they were read from: saving copies the source's files.
            None => {}
        }
    }
    out
}

/// Where each bitmap of `media` keeps its image in the XFL (None for other media):
/// its file under LIBRARY/ and Animate's copy under bin/, as the item names them,
/// else after its name and its place. Made-up names go round every other one,
/// ignoring case as Windows and macOS do, so no bitmap's file overwrites another's.
fn hrefs(media: &[Media]) -> Vec<Option<(String, String)>> {
    let kept = |b: &BitmapItem, a: &str| b.extra.get(a).filter(|h| safe(h)).map(str::to_string);
    let mut taken = HashSet::new();
    for m in media {
        if let Media::Bitmap(b) = m {
            taken.extend(kept(b, "href").map(|h| format!("LIBRARY/{h}").to_lowercase()));
            taken.extend(kept(b, "bitmapDataHRef").map(|h| format!("bin/{h}").to_lowercase()));
        }
    }
    // The first of `names` free in `dir`, taken from now on.
    fn free(taken: &mut HashSet<String>, dir: &str, mut names: impl Iterator<Item = String>) -> String {
        names.find(|h| taken.insert(format!("{dir}/{h}").to_lowercase())).unwrap_or_default()
    }
    let files = media.iter().enumerate().map(|(i, m)| {
        let Media::Bitmap(b) = m else { return None };
        let href = kept(b, "href").unwrap_or_else(|| {
            let name = xfl_write::href(&b.name).unwrap_or_else(|_| format!("Bitmap {}", i + 1));
            let ext = match b.data.format() {
                Some(Format::Jpeg) => "jpg",
                Some(Format::Gif) => "gif",
                _ => "png",
            };
            let image = [".png", ".jpg", ".jpeg", ".gif"].iter().any(|e| name.to_ascii_lowercase().ends_with(e));
            let (stem, ext) = match name.rsplit_once('.') {
                Some((stem, ext)) if image => (stem.to_string(), ext.to_string()),
                _ => (name, ext.to_string()),
            };
            free(&mut taken, "LIBRARY", (1..).map(|n| if n == 1 { format!("{stem}.{ext}") } else { format!("{stem} {n}.{ext}") }))
        });
        let bin = kept(b, "bitmapDataHRef").unwrap_or_else(|| free(&mut taken, "bin", (0..).map(|k| format!("M {} {k}.dat", i + 1))));
        Some((href, bin))
    });
    files.collect()
}

/// A file's path in the XFL that stays inside it.
fn safe(href: &str) -> bool {
    !href.contains(['\\', ':']) && href.split('/').all(|p| !p.is_empty() && p != "." && p != "..")
}

// ---- Drawing -----------------------------------------------------------------------------

/// `px` with its colours through `cx`.
// ponytail: a colour effect copies the bitmap at every draw; keep the tinted copies if many tinted
// bitmaps slow the stage down.
fn tinted(px: &Pixmap, cx: ColorTransform) -> Cow<'_, Pixmap> {
    if cx == ColorTransform::IDENTITY {
        return Cow::Borrowed(px);
    }
    let mut out = px.clone();
    for p in out.data_mut().chunks_exact_mut(4) {
        let a = p[3] as f32 / 255.0;
        let straight = |c: u8| if a > 0.0 { c as f32 / 255.0 / a } else { 0.0 };
        let [r, g, b, a] = cx.apply([straight(p[0]), straight(p[1]), straight(p[2]), a]);
        p.copy_from_slice(&[r * a, g * a, b * a, a].map(|v| (v * 255.0).round() as u8));
    }
    Cow::Owned(out)
}

fn quality(smooth: bool) -> FilterQuality {
    if smooth { FilterQuality::Bilinear } else { FilterQuality::Nearest }
}

/// Draws bitmap `b` from (0, 0) to its size, through `ts`, its colours through
/// `cx`. False, drawing nothing, if its image can't be read.
pub(crate) fn draw(pm: &mut Pixmap, b: &BitmapItem, ts: Transform, cx: ColorTransform) -> bool {
    let Some(px) = b.data.pixels() else { return false };
    let px = tinted(px, cx);
    let px: &Pixmap = &px;
    let Some(rect) = sk::Rect::from_xywh(0.0, 0.0, px.width() as f32, px.height() as f32) else { return false };
    let shader = sk::Pattern::new(px.as_ref(), SpreadMode::Pad, quality(b.smoothing), 1.0, Transform::identity());
    pm.fill_rect(rect, &sk::Paint { shader, ..Default::default() }, ts, None);
    true
}

/// The pixels bitmap fill `p` paints with, their colours through `cx`; None for
/// other paints, or a bitmap that's missing or can't be read.
pub(crate) fn fill_pixels<'a>(doc: &'a Document, p: &Paint, cx: ColorTransform) -> Option<Cow<'a, Pixmap>> {
    let Paint::Bitmap(f) = p else { return None };
    Some(tinted(doc.bitmap(&f.name)?.data.pixels()?, cx))
}

/// The tiny-skia paint of bitmap fill `f`, its bitmap's pixels being `px`.
pub(crate) fn sk_paint<'p>(f: &BitmapFill, smooth: bool, px: &'p Pixmap) -> sk::Paint<'p> {
    let m = f.matrix;
    let spread = if f.clipped { SpreadMode::Pad } else { SpreadMode::Repeat };
    let shader = sk::Pattern::new(px.as_ref(), spread, quality(smooth), 1.0, Transform::from_row(m.a, m.b, m.c, m.d, m.tx, m.ty));
    sk::Paint { shader, ..Default::default() }
}

/// `b`'s box, (0, 0) to its size, as a shape filled with `fill`; no edges if its size isn't known.
fn frame(b: &BitmapItem, fill: Paint) -> Shape {
    let (w, h) = b.size().unwrap_or((0, 0));
    let mut s = crate::draw::rectangle([(0.0, 0.0), (w as f32, h as f32)], 0.0, None, Some(fill));
    if w == 0 || h == 0 {
        s.edges.clear();
    }
    s
}

/// What hits a bitmap, and what measures it: its box.
pub(crate) fn box_shape(b: &BitmapItem) -> Shape {
    frame(b, Paint::Solid([0.0; 4]))
}

/// Modify > Break Apart: the bitmap on stage as a shape filled with it, where
/// it shows. None if its library item is missing or its size unknown.
pub fn broken_apart(doc: &Document, inst: &BitmapInstance) -> Option<Shape> {
    let fill = BitmapFill { name: inst.name.clone(), matrix: Matrix::IDENTITY, clipped: false, extra: Extra::default() };
    let s = frame(doc.bitmap(&inst.name)?, Paint::Bitmap(fill));
    (!s.edges.is_empty()).then_some(Shape { matrix: inst.matrix, ..s })
}

// ---- SWF ---------------------------------------------------------------------------------

/// How a bitmap on stage publishes: a shape of its box, filled with it, clipped.
pub(crate) fn placed_shape(b: &BitmapItem) -> Shape {
    frame(b, Paint::Bitmap(BitmapFill { name: b.name.clone(), matrix: Matrix::IDENTITY, clipped: true, extra: Extra::default() }))
}

/// The SWF tag defining bitmap `b` as character `id`: a JPEG photo as its JPEG
/// data, anything else lossless (premultiplied ARGB). None if it can't be read.
// ponytail: a photo that was a PNG or GIF publishes lossless, as nothing here encodes JPEG, and a
// JPEG publishes as imported, whatever the quality asked. Encode (DefineBitsJPEG3, with its alpha)
// when published size matters.
pub(crate) fn swf_bitmap(id: swf::CharacterId, b: &BitmapItem) -> Option<swf::Tag<'_>> {
    let px = b.data.pixels()?;
    if !b.lossless && b.data.format() == Some(Format::Jpeg) {
        return Some(swf::Tag::DefineBitsJpeg2 { id, jpeg_data: b.data.bytes() });
    }
    let (width, height) = (u16::try_from(px.width()).ok()?, u16::try_from(px.height()).ok()?);
    let data = Cow::Owned(zlib(&argb(px)));
    Some(swf::Tag::DefineBitsLossless(swf::DefineBitsLossless { version: 2, id, format: swf::BitmapFormat::Rgb32, width, height, data }))
}

/// Bitmap fill `f` on character `id`. SWF's matrix takes bitmap pixels to twips.
pub(crate) fn swf_fill(f: &BitmapFill, id: swf::CharacterId, smoothed: bool) -> swf::FillStyle {
    let m = f.matrix;
    let k = |v: f32| swf::Fixed16::from_f32(v * 20.0);
    let twips = |v: f32| swf::Twips::new((v * 20.0).round() as i32);
    let matrix = swf::Matrix { a: k(m.a), b: k(m.b), c: k(m.c), d: k(m.d), tx: twips(m.tx), ty: twips(m.ty) };
    swf::FillStyle::Bitmap { id, matrix, is_smoothed: smoothed, is_repeating: !f.clipped }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[test]
    fn lossless_refuses_bombs() {
        let head = |w: u16, h: u16| {
            let mut b = vec![3, 5];
            [w * 4, w, h].iter().for_each(|v| b.extend(v.to_le_bytes()));
            b.resize(24, 0);
            b.extend([1, 1]);
            b
        };
        // Header far past the cap.
        let mut big = head(16000, 16000);
        big.extend([0, 0]);
        assert!(lossless(&big).is_none());
        // Small header, a stream that inflates to far more than it says.
        let mut b = head(2, 2);
        let z = zlib(&vec![0u8; 64 << 20]);
        assert!(z.len() < 1 << 20 && z.len() < u16::MAX as usize * 40);
        for c in z.chunks(60000) {
            b.extend((c.len() as u16).to_le_bytes());
            b.extend_from_slice(c);
        }
        b.extend([0, 0]);
        assert!(lossless(&b).is_none());
    }

    /// A `w` × `h` PNG, each pixel `color(x, y)` (straight RGBA).
    pub(crate) fn png(w: u32, h: u32, color: impl Fn(u32, u32) -> [u8; 4]) -> Vec<u8> {
        let mut pm = Pixmap::new(w, h).unwrap();
        for (i, p) in pm.data_mut().chunks_exact_mut(4).enumerate() {
            let [r, g, b, a] = color(i as u32 % w, i as u32 / w);
            let k = |c: u8| ((c as u32 * a as u32 + 127) / 255) as u8;
            p.copy_from_slice(&[k(r), k(g), k(b), a]);
        }
        pm.encode_png().unwrap()
    }

    /// A 3 × 2 GIF: red, green, clear / blue, red, green.
    fn gif_bytes() -> Vec<u8> {
        let mut b = b"GIF89a".to_vec();
        b.extend([3, 0, 2, 0, 0x81, 0, 0]);
        // 4 colours: red, green, blue, black.
        b.extend([255, 0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0]);
        // Graphic Control: colour 3 is see-through.
        b.extend([0x21, 0xF9, 4, 1, 0, 0, 3, 0]);
        b.extend([0x2C, 0, 0, 0, 0, 3, 0, 2, 0, 0]);
        let lzw = weezl::encode::Encoder::new(weezl::BitOrder::Lsb, 2).encode(&[0, 1, 3, 2, 0, 1]).unwrap();
        b.push(2);
        b.push(lzw.len() as u8);
        b.extend(&lzw);
        b.extend([0, 0x3B]);
        b
    }

    fn rgba(pm: &Pixmap, x: u32, y: u32) -> [u8; 4] {
        let p = pm.pixel(x, y).unwrap();
        [p.red(), p.green(), p.blue(), p.alpha()]
    }

    #[test]
    fn decodes_png_gif_and_animates_lossless_data() {
        let png = png(2, 1, |x, _| if x == 0 { [255, 0, 0, 255] } else { [0, 0, 255, 128] });
        let item = BitmapItem::new("a.png", png);
        let px = item.data.pixels().unwrap();
        assert_eq!((rgba(px, 0, 0), rgba(px, 1, 0)), ([255, 0, 0, 255], [0, 0, 128, 128]), "premultiplied");
        assert!(item.lossless && item.smoothing && item.size() == Some((2, 1)));

        let gif = BitmapData::new(gif_bytes());
        let px = gif.pixels().unwrap();
        let row = |y| (0..3).map(|x| rgba(px, x, y)).collect::<Vec<_>>();
        assert_eq!(row(0), [[255, 0, 0, 255], [0, 255, 0, 255], [0, 0, 0, 0]]);
        assert_eq!(row(1), [[0, 0, 255, 255], [255, 0, 0, 255], [0, 255, 0, 255]]);

        // Animate's lossless data, written and read back.
        let dat = to_lossless(item.data.pixels().unwrap()).unwrap();
        assert_eq!(&dat[..8], &[3, 5, 8, 0, 2, 0, 1, 0]);
        let back = BitmapData::new(dat);
        assert_eq!(back.format(), Some(Format::Lossless));
        assert_eq!(back.pixels().unwrap().data(), item.data.pixels().unwrap().data());
        // The public notes' sample, a red pixel: zlib's header comes in a chunk of its own.
        let mut dat = vec![3, 5, 4, 0, 1, 0, 1, 0, 0, 0, 0, 0, 20, 0, 0, 0, 0, 0, 0, 0, 20, 0, 0, 0, 0, 1, 2, 0, 0x78, 0x01];
        dat.extend([10, 0, 0xFB, 0xFF, 0x9F, 0x81, 0x01, 0x00, 0x06, 0xFD, 0x01, 0xFF, 0, 0]);
        assert_eq!(rgba(BitmapData::new(dat).pixels().unwrap(), 0, 0), [255, 0, 0, 255]);

        assert_eq!(BitmapData::new(b"not an image".to_vec()).pixels(), None);
        assert_eq!(BitmapData::new(Vec::new()).pixels(), None);
    }

    #[test]
    fn names_and_paths() {
        let mut doc = Document::new(100.0, 100.0, 24.0);
        doc.media.push(Media::Bitmap(BitmapItem::new("a.png", Vec::new())));
        doc.media.push(Media::Bitmap(BitmapItem::new("a copy.png", Vec::new())));
        assert_eq!(free_name(&doc, "b.png"), "b.png");
        assert_eq!(free_name(&doc, "A.PNG"), "A copy 2.PNG", "taken ignoring case, as file names are");
        assert_eq!(free_name(&doc, "a"), "a");
        assert!(safe("Photos/sky.png") && safe("M 1 123.dat"));
        assert!(!safe("../x.png") && !safe("/x.png") && !safe("C:/x.png") && !safe("a\\b.png") && !safe(""));
        // A name's file goes under LIBRARY/ with an image's extension, one of its own whatever the case.
        let sky = |name: &str| Media::Bitmap(BitmapItem::new(name, super::tests::png(1, 1, |_, _| [0; 4])));
        let media = [Media::Unsupported { name: "DOMSoundItem".into(), xml: String::new() }, sky("Folder/sky"), sky("Folder/Sky.png")];
        let files = |a: &str, b: &str| Some((a.to_string(), b.to_string()));
        assert_eq!(hrefs(&media), [None, files("Folder/sky.png", "M 2 0.dat"), files("Folder/Sky 2.png", "M 3 0.dat")]);
    }
}
