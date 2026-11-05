//! Decoder for `.pli` vector levels. Ported from OpenToonz (`toonz/sources/image/pli/pli_io.cpp`
//! and `tiio_pli.cpp`, BSD-3-Clause, Copyright (c) DWANGO Co., Ltd. and contributors; see
//! docs/ENGINEERING.md for the notice).
//!
//! A file is a header and a flat run of tags. Strokes are `THICK_QUADRATIC_CHAIN` tags; a `COLOR`
//! tag sets the style id of the strokes that follow it in a frame's `IMAGE` tag, and tags refer to
//! each other by file offset, so tags are indexed by offset as they are read. Only files written
//! by version 5.7 or later are read (the thickness encoding before it differs). Filled regions
//! (`INTERSECTION_DATA`) are counted, not decoded.

use std::collections::HashMap;

use crate::error::ToonzError;
use crate::limits::MAX_PLI_TAGS;
use crate::palette::{Style, parse_style};

const MAX_STROKES: usize = 1_000_000;
const MAX_CHUNKS: usize = 4_000_000;
const MAX_FRAMES: usize = 100_000;
/// Fixed-point scale of coordinates until a `PRECISION_SCALE` tag says otherwise (OpenToonz
/// `REGION_COMPUTING_PRECISION`).
const DEFAULT_PRECISION: f64 = 128.0;

const T_SET8: u16 = 1;
const T_SET16: u16 = 2;
const T_SET32: u16 = 3;
const T_PALETTE: u16 = 5;
const T_PALETTE_ALPHA: u16 = 6;
const T_CHAIN: u16 = 10;
const T_GROUP: u16 = 14;
const T_IMAGE: u16 = 16;
const T_COLOR: u16 = 17;
const T_STYLE: u16 = 20;
const T_INTERSECTION: u16 = 21;
const T_LOOP: u16 = 23;
const T_PRECISION: u16 = 25;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThickPoint {
    pub x: f64,
    pub y: f64,
    /// Half the line width at the point, in the level's units.
    pub thick: f64,
}

/// One quadratic Bezier piece: start, control and end point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Chunk(pub [ThickPoint; 3]);

#[derive(Debug, Clone, PartialEq)]
pub struct Stroke {
    /// Id of the palette style that colours the stroke.
    pub style: u32,
    pub looped: bool,
    pub chunks: Vec<Chunk>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VectorFrame {
    pub number: u32,
    pub suffix: String,
    pub strokes: Vec<Stroke>,
    /// Filled regions the frame holds; they are not decoded.
    pub regions: usize,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct VectorLevel {
    pub version: (u8, u8),
    pub frames: Vec<VectorFrame>,
    /// The palette the file carries, by style id.
    pub styles: Vec<(u32, Style)>,
    /// Colours of the old `PALETTE` tags, which have no style ids.
    pub legacy_colors: Vec<[u8; 4]>,
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
    big: bool,
}

impl Reader<'_> {
    fn bad<T>(&self, what: &str) -> Result<T, ToonzError> {
        Err(ToonzError::malformed("pli", what))
    }

    fn take(&mut self, n: usize) -> Result<&[u8], ToonzError> {
        let end = self.at.checked_add(n).filter(|e| *e <= self.bytes.len());
        let Some(end) = end else {
            return self.bad("the file ends inside a tag");
        };
        let slice = &self.bytes[self.at..end];
        self.at = end;
        Ok(slice)
    }

    fn u8(&mut self) -> Result<u8, ToonzError> {
        Ok(self.take(1)?[0])
    }

    fn uint(&mut self, n: usize) -> Result<u32, ToonzError> {
        let big = self.big;
        let bytes = self.take(n)?;
        let mut value = 0u32;
        for (i, b) in bytes.iter().enumerate() {
            let shift = if big { 8 * (n - 1 - i) } else { 8 * i };
            value |= u32::from(*b) << shift;
        }
        Ok(value)
    }
}

/// A tag body being read with the current dynamic width.
struct Body<'a> {
    data: &'a [u8],
    at: usize,
    big: bool,
    width: usize,
}

impl Body<'_> {
    fn left(&self) -> usize {
        self.data.len() - self.at
    }

    fn byte(&mut self) -> Result<u8, ToonzError> {
        let b = self.data.get(self.at).copied();
        self.at += 1;
        b.ok_or_else(|| ToonzError::malformed("pli", "a tag body is too short"))
    }

    fn raw(&mut self, width: usize) -> Result<u32, ToonzError> {
        let mut value = 0u32;
        for i in 0..width {
            let b = u32::from(self.byte()?);
            let shift = if self.big { 8 * (width - 1 - i) } else { 8 * i };
            value |= b << shift;
        }
        Ok(value)
    }

    fn unsigned(&mut self) -> Result<u32, ToonzError> {
        self.raw(self.width)
    }

    /// Sign and magnitude: the top bit of the value is the sign.
    fn signed(&mut self) -> Result<i64, ToonzError> {
        self.signed_of(self.width)
    }

    fn signed_of(&mut self, width: usize) -> Result<i64, ToonzError> {
        let v = self.raw(width)?;
        let sign = 1u64 << (8 * width - 1);
        let magnitude = i64::try_from(u64::from(v) & (sign - 1)).unwrap_or(0);
        Ok(if u64::from(v) & sign == 0 {
            magnitude
        } else {
            -magnitude
        })
    }

    fn float(&mut self) -> Result<f64, ToonzError> {
        let whole = self.signed()?;
        let frac = f64::from(self.unsigned()?) / 65536.0;
        Ok(if whole < 0 {
            whole as f64 - frac
        } else {
            whole as f64 + frac
        })
    }
}

enum Object {
    Chain { looped: bool, chunks: Vec<Chunk> },
    Color(u32),
    Group(Vec<u32>),
}

/// Decodes a `.pli` file.
pub fn decode(bytes: &[u8]) -> Result<VectorLevel, ToonzError> {
    let big = match bytes.get(..4) {
        Some(b"PLIM") => false,
        Some(b"MILP") => true,
        _ => return Err(ToonzError::UnknownFormat("not a PLI vector level")),
    };
    let mut r = Reader { bytes, at: 4, big };
    let (major, minor) = (r.u8()?, r.u8()?);
    if !(5..=150).contains(&major) || (major == 5 && minor < 7) {
        return Err(ToonzError::NotYetImplemented {
            format: "pli",
            what: "files written before version 5.7",
        });
    }
    if major > 5 || minor >= 8 {
        let len = r.uint(2)? as usize;
        r.take(len)?;
    }
    let _file_length = r.uint(4)?;
    let _frames = r.uint(2)?;
    if major > 6 || (major == 6 && minor >= 5) {
        r.u8()?;
    }
    r.take(2)?;
    let mut level = VectorLevel {
        version: (major, minor),
        ..VectorLevel::default()
    };
    let mut width = 2usize;
    let mut precision = DEFAULT_PRECISION;
    let mut objects: HashMap<u32, Object> = HashMap::new();
    let mut tags = 0usize;
    let mut chunk_total = 0usize;
    let mut stroke_total = 0usize;
    let mut regions = 0usize;
    while r.at < bytes.len() {
        tags += 1;
        if tags > MAX_PLI_TAGS {
            return Err(ToonzError::Limit {
                what: "PLI tags",
                max: MAX_PLI_TAGS,
            });
        }
        let offset = u32::try_from(r.at).unwrap_or(u32::MAX);
        let first = r.u8()?;
        let (kind, length_id) = if first == 0xFF {
            let v = r.uint(2)?;
            ((v & 0x3FFF) as u16, (v >> 14) as u8)
        } else {
            (u16::from(first & 0x3F), first >> 6)
        };
        let length = match length_id {
            0 => 0,
            1 => r.uint(1)?,
            2 => r.uint(2)?,
            _ => r.uint(4)?,
        } as usize;
        if kind == 0 {
            break;
        }
        let mut body = Body {
            data: r.take(length)?,
            at: 0,
            big,
            width,
        };
        match kind {
            T_SET8 => width = 1,
            T_SET16 => width = 2,
            T_SET32 => width = 4,
            T_PRECISION => {
                let v = body.signed()?;
                if v > 0 {
                    precision = v as f64;
                }
            }
            T_PALETTE | T_PALETTE_ALPHA => {
                let step = if kind == T_PALETTE { 3 } else { 4 };
                while body.left() >= step {
                    let mut c = [255u8; 4];
                    for slot in c.iter_mut().take(step) {
                        *slot = body.byte()?;
                    }
                    level.legacy_colors.push(c);
                }
            }
            T_CHAIN | T_LOOP => {
                let (chunks, n) = chain(&mut body, precision, chunk_total)?;
                chunk_total = n;
                objects.insert(
                    offset,
                    Object::Chain {
                        looped: kind == T_LOOP,
                        chunks,
                    },
                );
            }
            T_COLOR => {
                body.byte()?;
                body.byte()?;
                let id = if body.left() >= width {
                    body.unsigned()?
                } else {
                    0
                };
                objects.insert(offset, Object::Color(id));
            }
            T_GROUP => {
                body.byte()?;
                let mut offsets = Vec::new();
                while body.left() >= width {
                    offsets.push(body.unsigned()?);
                }
                objects.insert(offset, Object::Group(offsets));
            }
            T_INTERSECTION => regions += 1,
            T_STYLE => {
                if let Some(entry) = style(&mut body, (major, minor))? {
                    level.styles.push(entry);
                }
            }
            T_IMAGE => {
                if level.frames.len() >= MAX_FRAMES {
                    return Err(ToonzError::Limit {
                        what: "PLI frames",
                        max: MAX_FRAMES,
                    });
                }
                let number = body.raw(2)?;
                let suffix = if major >= 150 {
                    let n = body.raw(4)? as usize;
                    let mut s = Vec::new();
                    for _ in 0..n.min(body.left()) {
                        s.push(body.byte()?);
                    }
                    String::from_utf8_lossy(&s).into_owned()
                } else if major > 6 || (major == 6 && minor >= 6) {
                    let letter = body.byte()?;
                    if letter > 0 {
                        char::from(letter).to_string()
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                };
                let mut offsets = Vec::new();
                while body.left() >= width {
                    offsets.push(body.unsigned()?);
                }
                let mut frame = VectorFrame {
                    number,
                    suffix,
                    strokes: Vec::new(),
                    regions,
                };
                regions = 0;
                let mut style = 0u32;
                for o in offsets {
                    collect(&objects, o, &mut style, &mut frame.strokes, 0)?;
                    stroke_total += frame.strokes.len();
                }
                if stroke_total > MAX_STROKES {
                    return Err(ToonzError::Limit {
                        what: "PLI strokes",
                        max: MAX_STROKES,
                    });
                }
                level.frames.push(frame);
                objects.clear();
            }
            _ => {}
        }
    }
    Ok(level)
}

/// Reads one chain: a start point, then per piece the offsets of the control and end points.
/// Coordinates are fixed point, divided by `precision`; thicknesses are bytes scaled by the
/// chain's maximum.
fn chain(
    b: &mut Body<'_>,
    precision: f64,
    mut total: usize,
) -> Result<(Vec<Chunk>, usize), ToonzError> {
    let w = b.width;
    let max_thick = f64::from(b.byte()?) / 255.0;
    let scale = 1.0 / precision;
    let mut p = ThickPoint {
        x: scale * b.signed()? as f64,
        y: scale * b.signed()? as f64,
        thick: f64::from(b.byte()?) * max_thick,
    };
    let count = b.left() / (4 * w + 2);
    total += count;
    if total > MAX_CHUNKS {
        return Err(ToonzError::Limit {
            what: "PLI chunks",
            max: MAX_CHUNKS,
        });
    }
    let mut chunks = Vec::with_capacity(count);
    for _ in 0..count {
        let start = p;
        let (mut dx1, mut dy1) = (scale * b.signed()? as f64, scale * b.signed()? as f64);
        p.thick = f64::from(b.byte()?) * max_thick;
        let (mut dx2, mut dy2) = (scale * b.signed()? as f64, scale * b.signed()? as f64);
        // A control point on an end point breaks later geometry; OpenToonz nudges it.
        if dx1 == 0.0 && dy1 == 0.0 && (dx2 != 0.0 || dy2 != 0.0) {
            (dx1, dy1) = (0.001 * dx2, 0.001 * dy2);
            (dx2, dy2) = (0.999 * dx2, 0.999 * dy2);
        } else if dx2 == 0.0 && dy2 == 0.0 && (dx1 != 0.0 || dy1 != 0.0) {
            (dx2, dy2) = (0.001 * dx1, 0.001 * dy1);
            (dx1, dy1) = (0.999 * dx1, 0.999 * dy1);
        }
        p.x += dx1;
        p.y += dy1;
        let control = p;
        p.thick = f64::from(b.byte()?) * max_thick;
        p.x += dx2;
        p.y += dy2;
        chunks.push(Chunk([start, control, p]));
    }
    Ok((chunks, total))
}

fn collect(
    objects: &HashMap<u32, Object>,
    offset: u32,
    style: &mut u32,
    out: &mut Vec<Stroke>,
    depth: usize,
) -> Result<(), ToonzError> {
    if depth > 8 {
        return Err(ToonzError::malformed("pli", "groups nest too deeply"));
    }
    match objects.get(&offset) {
        Some(Object::Color(id)) => *style = *id,
        Some(Object::Chain { looped, chunks }) => out.push(Stroke {
            style: *style,
            looped: *looped,
            chunks: chunks.clone(),
        }),
        Some(Object::Group(children)) => {
            for c in children {
                collect(objects, *c, style, out, depth + 1)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// A palette style tag: id, page, then typed parameters, read with 2-byte numbers whatever the
/// current width. Turned into the text form of a `.tpl` style so one parser reads both.
fn style(b: &mut Body<'_>, version: (u8, u8)) -> Result<Option<(u32, Style)>, ToonzError> {
    b.width = 2;
    let id = b.raw(2)?;
    let page = if version.0 > 5 || version.1 >= 6 {
        b.raw(2)?
    } else {
        0
    };
    let mut text = String::new();
    while b.left() > 0 {
        match b.byte()? {
            1 => text.push_str(&format!("{} ", b.byte()?)),
            2 | 3 => text.push_str(&format!("{} ", b.float()?)),
            4 => text.push_str(&format!("{} ", b.raw(2)?)),
            6 => {
                let n = b.raw(2)? as usize;
                let mut s = Vec::new();
                for _ in 0..n {
                    s.push(b.byte()?);
                }
                text.push_str(&format!("\"{}\" ", String::from_utf8_lossy(&s)));
            }
            // A raster parameter: not part of any colour.
            _ => return Ok(None),
        }
    }
    // Page 65535 marks an unused style; style 0 is the page names.
    if page == 65535 || text.is_empty() || id == 0 {
        return Ok(None);
    }
    Ok(Some((id, parse_style(&text))))
}
