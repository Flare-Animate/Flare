//! SWF container: bit-level reader/writer, tag records, header, zlib, and the
//! character-id bookkeeping that lets an import → export round trip keep the
//! ids the original file used.
//!
//! Written from the public SWF File Format Specification (v19). The idea of
//! keeping the source file's character ids (and of treating id 0 as reserved)
//! follows Next2Flash's `char_id_allocator.py`
//! (https://github.com/SSF2-Mods-Official/Next2Flash, MIT, (c) 2026 masterwebx;
//! see THIRD-PARTY-NOTICES.md), which compacts ids instead; here they are kept.

use flate2::{read::ZlibDecoder, write::ZlibEncoder, Compression};
use std::collections::BTreeSet;
use std::io::{Read, Write};

pub const END: u16 = 0;
pub const SHOW_FRAME: u16 = 1;
pub const DEFINE_SHAPE: u16 = 2;
pub const PLACE_OBJECT: u16 = 4;
pub const REMOVE_OBJECT: u16 = 5;
pub const DEFINE_BITS: u16 = 6;
pub const JPEG_TABLES: u16 = 8;
pub const SET_BACKGROUND: u16 = 9;
pub const DEFINE_FONT: u16 = 10;
pub const DEFINE_BITS_LOSSLESS: u16 = 20;
pub const DEFINE_BITS_JPEG2: u16 = 21;
pub const DEFINE_SHAPE2: u16 = 22;
pub const PLACE_OBJECT2: u16 = 26;
pub const REMOVE_OBJECT2: u16 = 28;
pub const DEFINE_SHAPE3: u16 = 32;
pub const DEFINE_FONT2: u16 = 48;
pub const DEFINE_BITS_JPEG3: u16 = 35;
pub const DEFINE_BITS_LOSSLESS2: u16 = 36;
pub const DEFINE_SPRITE: u16 = 39;
pub const FRAME_LABEL: u16 = 43;
pub const DEFINE_SHAPE4: u16 = 83;
pub const DEFINE_FONT3: u16 = 75;
pub const PLACE_OBJECT3: u16 = 70;

/// Twips per pixel.
pub const TWIPS: f32 = 20.0;

pub fn twips(px: f32) -> i32 {
    (px * TWIPS).round() as i32
}

// ---------------------------------------------------------------- bits

/// MSB-first bit writer.
#[derive(Default)]
pub struct BitW {
    pub buf: Vec<u8>,
    cur: u8,
    n: u8,
}

impl BitW {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn ub(&mut self, bits: u32, v: u32) {
        for i in (0..bits).rev() {
            self.cur = (self.cur << 1) | ((v >> i) & 1) as u8;
            self.n += 1;
            if self.n == 8 {
                self.buf.push(self.cur);
                self.cur = 0;
                self.n = 0;
            }
        }
    }
    pub fn sb(&mut self, bits: u32, v: i32) {
        self.ub(bits, (v as u32) & if bits >= 32 { u32::MAX } else { (1u32 << bits) - 1 });
    }
    pub fn flag(&mut self, b: bool) {
        self.ub(1, b as u32);
    }
    pub fn align(&mut self) {
        if self.n > 0 {
            self.buf.push(self.cur << (8 - self.n));
            self.cur = 0;
            self.n = 0;
        }
    }
    pub fn u8(&mut self, v: u8) {
        self.align();
        self.buf.push(v);
    }
    pub fn u16(&mut self, v: u16) {
        self.align();
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub fn u32(&mut self, v: u32) {
        self.align();
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub fn bytes(&mut self, v: &[u8]) {
        self.align();
        self.buf.extend_from_slice(v);
    }
    pub fn finish(mut self) -> Vec<u8> {
        self.align();
        self.buf
    }
}

/// Smallest signed bit count that holds every value (at least 1).
pub fn sb_bits(vals: &[i32]) -> u32 {
    let mut n = 1;
    for &v in vals {
        let need = if v >= 0 { 32 - (v as u32).leading_zeros() + 1 } else { 32 - (!(v as u32)).leading_zeros() + 1 };
        n = n.max(need);
    }
    n
}

pub fn ub_bits(v: u32) -> u32 {
    (32 - v.leading_zeros()).max(1)
}

/// MSB-first bit reader over a slice; reads past the end return zeros.
pub struct BitR<'a> {
    pub d: &'a [u8],
    pub pos: usize,
    bit: u8,
}

impl<'a> BitR<'a> {
    pub fn new(d: &'a [u8]) -> Self {
        BitR { d, pos: 0, bit: 0 }
    }
    pub fn at(d: &'a [u8], pos: usize) -> Self {
        BitR { d, pos, bit: 0 }
    }
    pub fn eof(&self) -> bool {
        self.pos >= self.d.len()
    }
    pub fn ub(&mut self, n: u32) -> u32 {
        let mut v = 0u32;
        for _ in 0..n {
            let byte = *self.d.get(self.pos).unwrap_or(&0);
            v = (v << 1) | ((byte >> (7 - self.bit)) & 1) as u32;
            self.bit += 1;
            if self.bit == 8 {
                self.bit = 0;
                self.pos += 1;
            }
        }
        v
    }
    pub fn sb(&mut self, n: u32) -> i32 {
        if n == 0 {
            return 0;
        }
        let v = self.ub(n);
        if n < 32 && v & (1 << (n - 1)) != 0 {
            (v | (u32::MAX << n)) as i32
        } else {
            v as i32
        }
    }
    pub fn flag(&mut self) -> bool {
        self.ub(1) != 0
    }
    pub fn align(&mut self) {
        if self.bit != 0 {
            self.bit = 0;
            self.pos += 1;
        }
    }
    pub fn u8(&mut self) -> u8 {
        self.align();
        let v = *self.d.get(self.pos).unwrap_or(&0);
        self.pos += 1;
        v
    }
    pub fn u16(&mut self) -> u16 {
        let a = self.u8() as u16;
        a | (self.u8() as u16) << 8
    }
    pub fn i16(&mut self) -> i16 {
        self.u16() as i16
    }
    pub fn u32(&mut self) -> u32 {
        let a = self.u16() as u32;
        a | (self.u16() as u32) << 16
    }
    pub fn rest(&mut self) -> &'a [u8] {
        self.align();
        let s = self.d.get(self.pos..).unwrap_or(&[]);
        self.pos = self.d.len();
        s
    }
    pub fn skip(&mut self, n: usize) {
        self.align();
        self.pos += n;
    }
    pub fn string(&mut self) -> String {
        self.align();
        let start = self.pos.min(self.d.len());
        let end = self.d[start..].iter().position(|&b| b == 0).map_or(self.d.len(), |p| start + p);
        self.pos = end + 1;
        String::from_utf8_lossy(&self.d[start..end]).into_owned()
    }
}

// ---------------------------------------------------------------- tags

/// A tag record: code and body (without header).
#[derive(Clone, Debug, PartialEq)]
pub struct RawTag {
    pub code: u16,
    pub body: Vec<u8>,
}

/// Tag header + body. Long form when the body needs it, or when `long` (the
/// spec wants it for bitmap tags).
pub fn tag_bytes(code: u16, body: &[u8], long: bool) -> Vec<u8> {
    let mut out = Vec::with_capacity(body.len() + 6);
    if long || body.len() >= 0x3F {
        out.extend_from_slice(&((code << 6) | 0x3F).to_le_bytes());
        out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    } else {
        out.extend_from_slice(&((code << 6) | body.len() as u16).to_le_bytes());
    }
    out.extend_from_slice(body);
    out
}

/// Tags are written long for these (bitmap payloads).
fn wants_long(code: u16) -> bool {
    matches!(code, DEFINE_BITS | DEFINE_BITS_JPEG2 | DEFINE_BITS_JPEG3 | DEFINE_BITS_LOSSLESS | DEFINE_BITS_LOSSLESS2)
}

/// Parse consecutive tag records (stops after End or at truncation).
pub fn parse_tags(d: &[u8]) -> Vec<RawTag> {
    let mut p = 0;
    let mut out = Vec::new();
    while p + 2 <= d.len() {
        let h = u16::from_le_bytes([d[p], d[p + 1]]);
        p += 2;
        let (code, mut len) = (h >> 6, (h & 0x3F) as usize);
        if len == 0x3F {
            if p + 4 > d.len() {
                break;
            }
            len = u32::from_le_bytes([d[p], d[p + 1], d[p + 2], d[p + 3]]) as usize;
            p += 4;
        }
        if p + len > d.len() {
            break;
        }
        out.push(RawTag { code, body: d[p..p + len].to_vec() });
        p += len;
        if code == END {
            break;
        }
    }
    out
}

pub fn zlib(data: &[u8]) -> Vec<u8> {
    let mut e = ZlibEncoder::new(Vec::new(), Compression::best());
    let _ = e.write_all(data);
    e.finish().unwrap_or_default()
}

pub fn unzlib(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    ZlibDecoder::new(data).read_to_end(&mut out).map_err(|e| format!("zlib: {e}"))?;
    Ok(out)
}

// ---------------------------------------------------------------- file

/// A parsed SWF: header fields and top-level tags.
#[derive(Clone, Debug)]
pub struct Swf {
    pub version: u8,
    /// Stage rectangle in twips: x_min, x_max, y_min, y_max.
    pub stage: [i32; 4],
    pub frame_rate: f32,
    pub frame_count: u16,
    pub tags: Vec<RawTag>,
}

fn rect(r: &mut BitR) -> [i32; 4] {
    let n = r.ub(5);
    let v = [r.sb(n), r.sb(n), r.sb(n), r.sb(n)];
    r.align();
    v
}

pub fn write_rect(w: &mut BitW, r: [i32; 4]) {
    let n = sb_bits(&r);
    w.ub(5, n);
    for v in r {
        w.sb(n, v);
    }
    w.align();
}

/// Read an FWS or CWS file (ZWS/LZMA is reported as unsupported).
pub fn parse(bytes: &[u8]) -> Result<Swf, String> {
    if bytes.len() < 8 {
        return Err("too short to be a SWF".into());
    }
    let version = bytes[3];
    let body = match &bytes[..3] {
        b"FWS" => bytes[8..].to_vec(),
        b"CWS" => unzlib(&bytes[8..])?,
        b"ZWS" => return Err("LZMA-compressed SWF (ZWS) is not supported".into()),
        _ => return Err("not a SWF file".into()),
    };
    let mut r = BitR::new(&body);
    let stage = rect(&mut r);
    let rate = r.u16();
    let frame_count = r.u16();
    let tags = parse_tags(&body[r.pos.min(body.len())..]);
    Ok(Swf { version, stage, frame_rate: rate as f32 / 256.0, frame_count, tags })
}

/// Header facts found by `validate`.
#[derive(Clone, Debug, PartialEq)]
pub struct SwfCheck {
    /// "FWS", "CWS" or "ZWS".
    pub signature: String,
    pub version: u8,
    pub declared_len: u32,
    /// Total length with the 8-byte header, after decompression.
    pub actual_len: usize,
}

/// Strict structural check: signature, FileLength equal to the (decompressed)
/// length, header present, tag stream inside the data and closed by End.
pub fn validate(bytes: &[u8]) -> Result<SwfCheck, String> {
    if bytes.len() < 8 {
        return Err("too short to be a SWF".into());
    }
    let sig = String::from_utf8_lossy(&bytes[..3]).into_owned();
    let body = match &bytes[..3] {
        b"FWS" => bytes[8..].to_vec(),
        b"CWS" => unzlib(&bytes[8..])?,
        b"ZWS" => return Err("LZMA-compressed SWF (ZWS) is not supported yet; recompress it as FWS/CWS first".into()),
        _ => return Err("not a SWF file (bad signature)".into()),
    };
    let declared = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    let actual = 8 + body.len();
    if declared as usize != actual {
        return Err(format!("malformed SWF: header FileLength={declared} but the {}length is {actual}", if sig == "FWS" { "" } else { "decompressed " }));
    }
    if body.is_empty() {
        return Err("malformed SWF: no data after the header".into());
    }
    let mut r = BitR::new(&body);
    let _ = rect(&mut r);
    r.u32();
    let mut p = r.pos;
    if p > body.len() {
        return Err("malformed SWF: truncated header".into());
    }
    loop {
        if p + 2 > body.len() {
            return Err("malformed SWF: missing End tag".into());
        }
        let h = u16::from_le_bytes([body[p], body[p + 1]]);
        p += 2;
        let (code, mut len) = (h >> 6, (h & 0x3F) as usize);
        if len == 0x3F {
            if p + 4 > body.len() {
                return Err("malformed SWF: tag stream runs past end of file".into());
            }
            len = u32::from_le_bytes([body[p], body[p + 1], body[p + 2], body[p + 3]]) as usize;
            p += 4;
        }
        if p.saturating_add(len) > body.len() {
            return Err(format!("malformed SWF: tag {code} (length {len}) runs past end of file"));
        }
        p += len;
        if code == END {
            return Ok(SwfCheck { signature: sig, version: bytes[3], declared_len: declared, actual_len: actual });
        }
    }
}

/// Serialize with the given tags (End is appended). `compress` writes CWS.
pub fn write(version: u8, stage: [i32; 4], frame_rate: f32, frame_count: u16, tags: &[RawTag], compress: bool) -> Vec<u8> {
    let mut w = BitW::new();
    write_rect(&mut w, stage);
    w.u16((frame_rate.clamp(0.0, 255.99) * 256.0).round() as u16);
    w.u16(frame_count);
    let mut body = w.finish();
    for t in tags.iter().filter(|t| t.code != END) {
        body.extend_from_slice(&tag_bytes(t.code, &t.body, wants_long(t.code)));
    }
    body.extend_from_slice(&tag_bytes(END, &[], false));
    let total = (body.len() + 8) as u32;
    let mut out = Vec::new();
    out.extend_from_slice(if compress { b"CWS" } else { b"FWS" });
    out.push(version);
    out.extend_from_slice(&total.to_le_bytes());
    if compress {
        out.extend_from_slice(&zlib(&body));
    } else {
        out.extend_from_slice(&body);
    }
    out
}

// ------------------------------------------------------------ char ids

/// Hands out character ids for an export: ids an item already has are kept
/// (first claimant wins); everything else gets a fresh id above the largest
/// one reserved, so nothing collides. Id 0 is never given out (it is the
/// root timeline).
#[derive(Default)]
pub struct CharIds {
    used: BTreeSet<u16>,
    next: u32,
}

impl CharIds {
    /// `reserved`: every id any item wants to keep (so fresh ids stay above them).
    pub fn new(reserved: impl IntoIterator<Item = u16>) -> Self {
        let max = reserved.into_iter().filter(|&i| i != 0).max().unwrap_or(0);
        CharIds { used: BTreeSet::new(), next: max as u32 + 1 }
    }
    /// The id for an item that wants `wanted`, or a fresh one if it is 0/None/taken.
    pub fn claim(&mut self, wanted: Option<u16>) -> Result<u16, String> {
        if let Some(w) = wanted.filter(|&w| w != 0) {
            if self.used.insert(w) {
                return Ok(w);
            }
        }
        self.fresh()
    }
    pub fn fresh(&mut self) -> Result<u16, String> {
        while self.next <= u16::MAX as u32 && self.used.contains(&(self.next as u16)) {
            self.next += 1;
        }
        if self.next > u16::MAX as u32 {
            return Err("more than 65535 characters: SWF character ids ran out".into());
        }
        let id = self.next as u16;
        self.next += 1;
        self.used.insert(id);
        Ok(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bits_roundtrip() {
        let mut w = BitW::new();
        w.ub(5, 19);
        w.sb(7, -33);
        w.flag(true);
        w.u16(0xBEEF);
        let b = w.finish();
        let mut r = BitR::new(&b);
        assert_eq!(r.ub(5), 19);
        assert_eq!(r.sb(7), -33);
        assert!(r.flag());
        assert_eq!(r.u16(), 0xBEEF);
        assert_eq!(sb_bits(&[0]), 1);
        assert_eq!(sb_bits(&[-1]), 1);
        assert_eq!(sb_bits(&[1]), 2);
        assert_eq!(sb_bits(&[-2, 1]), 2);
        assert_eq!(sb_bits(&[127, -128]), 8);
    }

    #[test]
    fn container_roundtrip_both_compressions() {
        let tags = vec![RawTag { code: SET_BACKGROUND, body: vec![1, 2, 3] }, RawTag { code: SHOW_FRAME, body: vec![] }];
        for c in [false, true] {
            let b = write(11, [0, 11000, 0, 8000], 24.0, 1, &tags, c);
            assert_eq!(&b[..3], if c { b"CWS" } else { b"FWS" });
            let s = parse(&b).unwrap();
            assert_eq!(s.version, 11);
            assert_eq!(s.stage, [0, 11000, 0, 8000]);
            assert_eq!(s.frame_rate, 24.0);
            assert_eq!(s.tags.iter().map(|t| t.code).collect::<Vec<_>>(), vec![SET_BACKGROUND, SHOW_FRAME, END]);
        }
        assert!(parse(b"nope....").is_err());
        assert!(parse(b"ZWS\x0b\0\0\0\0").is_err());
    }

    #[test]
    fn ids_keep_then_allocate_above_max() {
        let mut ids = CharIds::new([5, 9, 3, 0]);
        assert_eq!(ids.claim(Some(9)).unwrap(), 9);
        assert_eq!(ids.claim(Some(5)).unwrap(), 5);
        // collision -> fresh, above the largest reserved id
        assert_eq!(ids.claim(Some(9)).unwrap(), 10);
        assert_eq!(ids.claim(None).unwrap(), 11);
        assert_eq!(ids.claim(Some(0)).unwrap(), 12);
        assert_eq!(ids.claim(Some(3)).unwrap(), 3);
    }

    #[test]
    fn ids_run_out_cleanly() {
        let mut ids = CharIds::new([65535]);
        assert!(ids.fresh().is_err());
        assert_eq!(ids.claim(Some(65535)).unwrap(), 65535);
    }
}
