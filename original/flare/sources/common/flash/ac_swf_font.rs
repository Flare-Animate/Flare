//! SWF fonts (DefineFont2 / DefineFont3) ↔ TrueType.
//!
//! * `parse_define_font` reads glyph outlines, advances and the code table.
//! * `to_ttf` writes a minimal TrueType font (head, hhea, maxp, hmtx, cmap
//!   format 4, loca, glyf, name, post, OS/2) in pure Rust.
//! * `from_ttf` reads a TrueType/OpenType font (ttf-parser) and
//!   `define_font3` writes it as a DefineFont3 tag.
//!
//! Outlines are held TrueType-style: y up, em units (`UPEM` = 1024), closed
//! contours of on/off-curve points (off-curve = quadratic control).
//! DefineFont3 stores glyphs y down on a 20480-unit em (1/20 em unit).
//!
//! Format details from the public SWF File Format Specification and the
//! Apple/Microsoft TrueType specifications. The overall idea (convert glyph
//! shapes to a font file and back, reading `swf_font_to_ttf.py` /
//! `ttf_to_swf_font.py` in Next2Flash, MIT, (c) 2026 masterwebx; see
//! THIRD-PARTY-NOTICES.md) is theirs; this is an independent Rust writer.

use crate::swf::*;

/// Font units per em used for the in-memory outlines and the TrueType output.
pub const UPEM: i32 = 1024;

/// An outline point; `on` false = quadratic control point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OPt {
    pub x: i32,
    pub y: i32,
    pub on: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FontGlyph {
    /// UCS-2 code point this glyph is for.
    pub code: u16,
    /// Advance width in em units.
    pub advance: i32,
    pub contours: Vec<Vec<OPt>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SwfFont {
    pub name: String,
    pub bold: bool,
    pub italic: bool,
    pub ascent: i32,
    pub descent: i32,
    pub leading: i32,
    pub glyphs: Vec<FontGlyph>,
}

// ------------------------------------------------------- SWF glyph shapes

/// A glyph SHAPE record list → contours (units as stored; y flipped to up).
fn read_glyph(d: &[u8], scale: f32) -> Vec<Vec<OPt>> {
    let mut r = BitR::new(d);
    let (fbits, lbits) = (r.ub(4), r.ub(4));
    let sc = |v: i32| (v as f32 * scale).round() as i32;
    let mut contours: Vec<Vec<OPt>> = Vec::new();
    let mut cur: Vec<OPt> = Vec::new();
    let (mut f0, mut f1) = (0u32, 0u32);
    let mut left_only = false;
    let mut pos = (0i32, 0i32);
    let flush = |cur: &mut Vec<OPt>, contours: &mut Vec<Vec<OPt>>, left_only: bool| {
        if cur.len() > 1 && cur.first() == cur.last() {
            cur.pop();
        }
        if cur.len() >= 2 {
            let mut c = std::mem::take(cur);
            if left_only {
                c.reverse();
            }
            contours.push(c);
        } else {
            cur.clear();
        }
    };
    while !r.eof() {
        if !r.flag() {
            let flags = r.ub(5);
            if flags == 0 {
                break;
            }
            if flags & 1 != 0 {
                flush(&mut cur, &mut contours, left_only);
                let n = r.ub(5);
                pos = (r.sb(n), r.sb(n));
                cur.push(OPt { x: sc(pos.0), y: -sc(pos.1), on: true });
            }
            if flags & 2 != 0 {
                f0 = r.ub(fbits);
            }
            if flags & 4 != 0 {
                f1 = r.ub(fbits);
            }
            if flags & 8 != 0 {
                r.ub(lbits);
            }
            left_only = f0 != 0 && f1 == 0;
        } else if r.flag() {
            let n = r.ub(4) + 2;
            let (dx, dy) = if r.flag() {
                (r.sb(n), r.sb(n))
            } else if r.flag() {
                (0, r.sb(n))
            } else {
                (r.sb(n), 0)
            };
            pos = (pos.0 + dx, pos.1 + dy);
            cur.push(OPt { x: sc(pos.0), y: -sc(pos.1), on: true });
        } else {
            let n = r.ub(4) + 2;
            let c = (pos.0 + r.sb(n), pos.1 + r.sb(n));
            pos = (c.0 + r.sb(n), c.1 + r.sb(n));
            cur.push(OPt { x: sc(c.0), y: -sc(c.1), on: false });
            cur.push(OPt { x: sc(pos.0), y: -sc(pos.1), on: true });
        }
    }
    flush(&mut cur, &mut contours, left_only);
    contours
}

/// Contours → a glyph SHAPE (`k` = SWF units per em unit).
fn write_glyph(contours: &[Vec<OPt>], k: i32) -> Vec<u8> {
    let mut w = BitW::new();
    w.ub(4, 1); // one fill bit
    w.ub(4, 0);
    let mut first = true;
    for c in contours {
        let pts = closed_walk(c);
        if pts.len() < 2 {
            continue;
        }
        let t = |p: &OPt| (p.x * k, -p.y * k);
        let start = t(&pts[0]);
        w.flag(false);
        // move (+ fill style 1 on the first contour)
        w.ub(5, if first { 0x05 } else { 0x01 });
        let n = sb_bits(&[start.0, start.1]);
        w.ub(5, n);
        w.sb(n, start.0);
        w.sb(n, start.1);
        if first {
            w.ub(1, 1);
        }
        first = false;
        let mut p = start;
        let mut i = 1;
        while i < pts.len() {
            if pts[i].on {
                let q = t(&pts[i]);
                let (dx, dy) = (q.0 - p.0, q.1 - p.1);
                let n = sb_bits(&[dx, dy]).max(2);
                w.flag(true);
                w.flag(true);
                w.ub(4, n - 2);
                if dx != 0 && dy != 0 {
                    w.flag(true);
                    w.sb(n, dx);
                    w.sb(n, dy);
                } else {
                    w.flag(false);
                    w.flag(dx == 0);
                    w.sb(n, if dx == 0 { dy } else { dx });
                }
                p = q;
                i += 1;
            } else {
                let ctl = t(&pts[i]);
                let q = t(&pts[i + 1]);
                let (cx, cy, ax, ay) = (ctl.0 - p.0, ctl.1 - p.1, q.0 - ctl.0, q.1 - ctl.1);
                let n = sb_bits(&[cx, cy, ax, ay]).max(2);
                w.flag(true);
                w.flag(false);
                w.ub(4, n - 2);
                w.sb(n, cx);
                w.sb(n, cy);
                w.sb(n, ax);
                w.sb(n, ay);
                p = q;
                i += 2;
            }
        }
    }
    w.ub(6, 0);
    w.finish()
}

/// The contour as a walk that starts and ends on the same on-curve point, with
/// an on-curve point between every two controls (so edges are lines or
/// control + anchor pairs).
fn closed_walk(c: &[OPt]) -> Vec<OPt> {
    if c.is_empty() {
        return vec![];
    }
    // implied on-curve points between consecutive controls
    let mut ex: Vec<OPt> = Vec::new();
    for (i, p) in c.iter().enumerate() {
        let n = &c[(i + 1) % c.len()];
        ex.push(*p);
        if !p.on && !n.on {
            ex.push(OPt { x: (p.x + n.x) / 2, y: (p.y + n.y) / 2, on: true });
        }
    }
    let s = match ex.iter().position(|p| p.on) {
        Some(s) => s,
        None => return vec![],
    };
    ex.rotate_left(s);
    let start = ex[0];
    ex.push(start);
    ex
}

// ------------------------------------------------------- DefineFont2/3

/// Read a DefineFont2 (tag 48) or DefineFont3 (tag 75) body.
pub fn parse_define_font(code: u16, body: &[u8]) -> Result<SwfFont, String> {
    if code != DEFINE_FONT2 && code != DEFINE_FONT3 {
        return Err("not a DefineFont2/3 tag".into());
    }
    let scale = if code == DEFINE_FONT3 { 1.0 / 20.0 } else { 1.0 };
    let mut r = BitR::new(body);
    r.u16();
    let flags = r.u8();
    r.u8(); // language
    let nlen = r.u8() as usize;
    let name = String::from_utf8_lossy(body.get(r.pos..r.pos + nlen).ok_or("truncated name")?).trim_end_matches('\0').to_string();
    r.skip(nlen);
    let n = r.u16() as usize;
    let wide_off = flags & 0x08 != 0;
    let wide_codes = flags & 0x04 != 0 || code == DEFINE_FONT3;
    let table = r.pos;
    let mut offs = Vec::with_capacity(n + 1);
    for _ in 0..=n {
        offs.push(if wide_off { r.u32() as usize } else { r.u16() as usize });
    }
    // offs[n] is the code table offset
    let code_table = table + offs[n];
    let mut shapes = Vec::new();
    for i in 0..n {
        let (a, b) = (table + offs[i], table + offs[i + 1]);
        let d = body.get(a..b.min(body.len())).ok_or("glyph offset out of range")?;
        shapes.push(read_glyph(d, scale));
    }
    let mut cr = BitR::at(body, code_table);
    let codes: Vec<u16> = (0..n).map(|_| if wide_codes { cr.u16() } else { cr.u8() as u16 }).collect();
    let (mut ascent, mut descent, mut leading) = (0, 0, 0);
    let mut advances = vec![0i32; n];
    if flags & 0x80 != 0 {
        ascent = (cr.i16() as f32 * scale).round() as i32;
        descent = (cr.i16() as f32 * scale).round() as i32;
        leading = (cr.i16() as f32 * scale).round() as i32;
        for a in advances.iter_mut() {
            *a = (cr.i16() as f32 * scale).round() as i32;
        }
    }
    let glyphs = (0..n).map(|i| FontGlyph { code: codes[i], advance: advances[i], contours: std::mem::take(&mut shapes[i]) }).collect();
    Ok(SwfFont { name, bold: flags & 1 != 0, italic: flags & 2 != 0, ascent, descent, leading, glyphs })
}

/// Write a DefineFont3 tag (character id `id`).
pub fn define_font3(id: u16, f: &SwfFont) -> RawTag {
    let k = 20;
    let shapes: Vec<Vec<u8>> = f.glyphs.iter().map(|g| write_glyph(&g.contours, k)).collect();
    let n = f.glyphs.len();
    let mut w = BitW::new();
    w.u16(id);
    w.u8(0x80 | 0x08 | 0x04 | (f.italic as u8) << 1 | f.bold as u8);
    w.u8(0); // language
    let name = f.name.as_bytes();
    let name = &name[..name.len().min(255)];
    w.u8(name.len() as u8);
    w.bytes(name);
    w.u16(n as u16);
    // offsets are from the start of the offset table
    let table_len = 4 * (n + 1);
    let mut off = table_len;
    for s in &shapes {
        w.u32(off as u32);
        off += s.len();
    }
    w.u32(off as u32);
    for s in &shapes {
        w.bytes(s);
    }
    for g in &f.glyphs {
        w.u16(g.code);
    }
    w.u16((f.ascent * k).clamp(0, 32767) as u16);
    w.u16((f.descent * k).clamp(0, 32767) as u16);
    w.u16((f.leading * k).clamp(-32768, 32767) as u16);
    for g in &f.glyphs {
        w.u16((g.advance * k).clamp(0, 32767) as u16);
    }
    for g in &f.glyphs {
        let (mut x0, mut x1, mut y0, mut y1) = (i32::MAX, i32::MIN, i32::MAX, i32::MIN);
        for p in g.contours.iter().flatten() {
            x0 = x0.min(p.x);
            x1 = x1.max(p.x);
            y0 = y0.min(p.y);
            y1 = y1.max(p.y);
        }
        let b = if x0 > x1 { [0, 0, 0, 0] } else { [x0 * k, x1 * k, -y1 * k, -y0 * k] };
        write_rect(&mut w, b);
    }
    w.u16(0); // kerning records
    RawTag { code: DEFINE_FONT3, body: w.finish() }
}

// ----------------------------------------------------------------- TTF

fn be16(v: &mut Vec<u8>, x: u16) {
    v.extend_from_slice(&x.to_be_bytes());
}
fn be32(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_be_bytes());
}
fn bei16(v: &mut Vec<u8>, x: i16) {
    v.extend_from_slice(&x.to_be_bytes());
}

fn checksum(d: &[u8]) -> u32 {
    let mut sum = 0u32;
    for c in d.chunks(4) {
        let mut b = [0u8; 4];
        b[..c.len()].copy_from_slice(c);
        sum = sum.wrapping_add(u32::from_be_bytes(b));
    }
    sum
}

fn utf16be(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(|u| u.to_be_bytes()).collect()
}

/// A TrueType font (version 1.0 sfnt) holding `f`'s glyphs. Glyph 0 is a
/// `.notdef` box; glyph `i + 1` is `f.glyphs[i]`.
pub fn to_ttf(f: &SwfFont) -> Vec<u8> {
    let notdef = FontGlyph {
        code: 0,
        advance: UPEM / 2,
        contours: vec![
            vec![OPt { x: 64, y: 0, on: true }, OPt { x: 64, y: 700, on: true }, OPt { x: 448, y: 700, on: true }, OPt { x: 448, y: 0, on: true }],
            vec![OPt { x: 112, y: 48, on: true }, OPt { x: 400, y: 48, on: true }, OPt { x: 400, y: 652, on: true }, OPt { x: 112, y: 652, on: true }],
        ],
    };
    let all: Vec<&FontGlyph> = std::iter::once(&notdef).chain(f.glyphs.iter()).collect();
    let n = all.len();

    // glyf + loca
    let (mut glyf, mut loca) = (Vec::new(), Vec::new());
    let (mut max_pts, mut max_ctr) = (0usize, 0usize);
    let (mut gx0, mut gy0, mut gx1, mut gy1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    let mut lsb = Vec::with_capacity(n);
    let mut adv_max = 0;
    for g in &all {
        be32(&mut loca, glyf.len() as u32);
        adv_max = adv_max.max(g.advance);
        let contours: Vec<&Vec<OPt>> = g.contours.iter().filter(|c| c.len() >= 2).collect();
        if contours.is_empty() {
            lsb.push(0i16);
            continue;
        }
        let pts: Vec<&OPt> = contours.iter().flat_map(|c| c.iter()).collect();
        max_pts = max_pts.max(pts.len());
        max_ctr = max_ctr.max(contours.len());
        let (x0, x1) = (pts.iter().map(|p| p.x).min().unwrap(), pts.iter().map(|p| p.x).max().unwrap());
        let (y0, y1) = (pts.iter().map(|p| p.y).min().unwrap(), pts.iter().map(|p| p.y).max().unwrap());
        gx0 = gx0.min(x0);
        gy0 = gy0.min(y0);
        gx1 = gx1.max(x1);
        gy1 = gy1.max(y1);
        lsb.push(x0 as i16);
        bei16(&mut glyf, contours.len() as i16);
        for v in [x0, y0, x1, y1] {
            bei16(&mut glyf, v as i16);
        }
        let mut end = 0u16;
        for c in &contours {
            end += c.len() as u16;
            be16(&mut glyf, end - 1);
        }
        be16(&mut glyf, 0); // no instructions
        for p in &pts {
            glyf.push(p.on as u8); // flag: on-curve, 16-bit deltas
        }
        let mut prev = 0i32;
        for p in &pts {
            bei16(&mut glyf, (p.x - prev) as i16);
            prev = p.x;
        }
        prev = 0;
        for p in &pts {
            bei16(&mut glyf, (p.y - prev) as i16);
            prev = p.y;
        }
        while glyf.len() % 4 != 0 {
            glyf.push(0);
        }
    }
    be32(&mut loca, glyf.len() as u32);
    if gx0 > gx1 {
        (gx0, gy0, gx1, gy1) = (0, 0, 0, 0);
    }

    // head
    let mut head = Vec::new();
    be32(&mut head, 0x0001_0000);
    be32(&mut head, 0x0001_0000);
    be32(&mut head, 0); // checkSumAdjustment, patched below
    be32(&mut head, 0x5F0F_3CF5);
    be16(&mut head, 0x000B);
    be16(&mut head, UPEM as u16);
    head.extend_from_slice(&[0; 16]); // created, modified
    for v in [gx0, gy0, gx1, gy1] {
        bei16(&mut head, v as i16);
    }
    be16(&mut head, f.bold as u16 | (f.italic as u16) << 1);
    be16(&mut head, 8);
    bei16(&mut head, 2);
    bei16(&mut head, 1); // long loca
    bei16(&mut head, 0);

    // hhea + hmtx
    let (asc, desc) = (if f.ascent > 0 { f.ascent } else { 800 }, if f.descent > 0 { f.descent } else { 200 });
    let mut hhea = Vec::new();
    be32(&mut hhea, 0x0001_0000);
    bei16(&mut hhea, asc as i16);
    bei16(&mut hhea, -desc as i16);
    bei16(&mut hhea, f.leading as i16);
    be16(&mut hhea, adv_max as u16);
    bei16(&mut hhea, gx0 as i16);
    bei16(&mut hhea, 0);
    bei16(&mut hhea, gx1 as i16);
    bei16(&mut hhea, 1);
    bei16(&mut hhea, 0);
    bei16(&mut hhea, 0);
    hhea.extend_from_slice(&[0; 8]);
    bei16(&mut hhea, 0);
    be16(&mut hhea, n as u16);
    let mut hmtx = Vec::new();
    for (g, l) in all.iter().zip(&lsb) {
        be16(&mut hmtx, g.advance.clamp(0, 65535) as u16);
        bei16(&mut hmtx, *l);
    }

    // maxp
    let mut maxp = Vec::new();
    be32(&mut maxp, 0x0001_0000);
    be16(&mut maxp, n as u16);
    be16(&mut maxp, max_pts as u16);
    be16(&mut maxp, max_ctr as u16);
    for v in [0u16, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0] {
        be16(&mut maxp, v);
    }

    // cmap format 4
    let mut map: Vec<(u16, u16)> = f.glyphs.iter().enumerate().filter(|(_, g)| g.code != 0 && g.code != 0xFFFF).map(|(i, g)| (g.code, i as u16 + 1)).collect();
    map.sort_unstable();
    map.dedup_by_key(|m| m.0);
    let mut segs: Vec<(u16, u16, u16)> = Vec::new(); // start, end, first gid
    for &(c, g) in &map {
        match segs.last_mut() {
            Some(s) if s.1 + 1 == c && s.2 + (s.1 - s.0) + 1 == g => s.1 = c,
            _ => segs.push((c, c, g)),
        }
    }
    segs.push((0xFFFF, 0xFFFF, 0));
    let sc = segs.len() as u16;
    let mut sub = Vec::new();
    be16(&mut sub, 4);
    be16(&mut sub, 16 + 8 * sc);
    be16(&mut sub, 0);
    be16(&mut sub, sc * 2);
    let mut es = 0u16;
    while (1u32 << (es + 1)) <= sc as u32 {
        es += 1;
    }
    let sr = 2 * (1u16 << es);
    be16(&mut sub, sr);
    be16(&mut sub, es);
    be16(&mut sub, sc * 2 - sr);
    for s in &segs {
        be16(&mut sub, s.1);
    }
    be16(&mut sub, 0);
    for s in &segs {
        be16(&mut sub, s.0);
    }
    for s in &segs {
        // the last segment maps 0xFFFF to glyph 0: delta 1 wraps to it
        be16(&mut sub, if s.0 == 0xFFFF { 1 } else { s.2.wrapping_sub(s.0) });
    }
    for _ in &segs {
        be16(&mut sub, 0);
    }
    let mut cmap = Vec::new();
    be16(&mut cmap, 0);
    be16(&mut cmap, 1);
    be16(&mut cmap, 3);
    be16(&mut cmap, 1);
    be32(&mut cmap, 12);
    cmap.extend_from_slice(&sub);

    // name
    let family = if f.name.is_empty() { "Untitled" } else { &f.name };
    let style = match (f.bold, f.italic) {
        (false, false) => "Regular",
        (true, false) => "Bold",
        (false, true) => "Italic",
        (true, true) => "Bold Italic",
    };
    let full = format!("{family} {style}");
    let ps: String = full.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect::<String>();
    let ps = if ps.is_empty() { "Untitled".to_string() } else { ps };
    let strings = [(1u16, family.to_string()), (2, style.to_string()), (4, full.clone()), (6, ps)];
    let mut name = Vec::new();
    be16(&mut name, 0);
    be16(&mut name, strings.len() as u16);
    be16(&mut name, 6 + 12 * strings.len() as u16);
    let mut data = Vec::new();
    for (id, s) in &strings {
        let b = utf16be(s);
        be16(&mut name, 3);
        be16(&mut name, 1);
        be16(&mut name, 0x409);
        be16(&mut name, *id);
        be16(&mut name, b.len() as u16);
        be16(&mut name, data.len() as u16);
        data.extend_from_slice(&b);
    }
    name.extend_from_slice(&data);

    // post 3.0
    let mut post = Vec::new();
    be32(&mut post, 0x0003_0000);
    be32(&mut post, 0);
    bei16(&mut post, -100);
    bei16(&mut post, 50);
    be32(&mut post, 0);
    post.extend_from_slice(&[0; 16]);

    // OS/2 version 3 (weight and style flags, metrics)
    let mut os2 = Vec::new();
    be16(&mut os2, 3);
    bei16(&mut os2, (all.iter().map(|g| g.advance as i64).sum::<i64>() / n.max(1) as i64) as i16);
    be16(&mut os2, if f.bold { 700 } else { 400 });
    be16(&mut os2, 5);
    be16(&mut os2, 0);
    for v in [0i16, 0, 0, 0, 0, 0, 0, 0, 0, 0] {
        bei16(&mut os2, v);
    }
    bei16(&mut os2, 0);
    os2.extend_from_slice(&[0; 10]); // panose
    os2.extend_from_slice(&[0; 16]); // unicode ranges
    os2.extend_from_slice(b"FLRE");
    be16(&mut os2, if f.italic { 1 } else { 0 } | if f.bold { 32 } else { 0 } | if !f.bold && !f.italic { 64 } else { 0 });
    be16(&mut os2, map.first().map_or(0, |m| m.0));
    be16(&mut os2, map.last().map_or(0, |m| m.0));
    bei16(&mut os2, asc as i16);
    bei16(&mut os2, -desc as i16);
    bei16(&mut os2, f.leading as i16);
    be16(&mut os2, asc.max(gy1) as u16);
    be16(&mut os2, desc.max(-gy0) as u16);
    os2.extend_from_slice(&[0; 8]); // code page ranges
    bei16(&mut os2, 0);
    bei16(&mut os2, 0);
    be16(&mut os2, 0);
    be16(&mut os2, 32);
    be16(&mut os2, 0);

    let mut tables: Vec<(&[u8; 4], Vec<u8>)> = vec![(b"OS/2", os2), (b"cmap", cmap), (b"glyf", glyf), (b"head", head), (b"hhea", hhea), (b"hmtx", hmtx), (b"loca", loca), (b"maxp", maxp), (b"name", name), (b"post", post)];
    tables.sort_by_key(|t| *t.0);
    let nt = tables.len() as u16;
    let mut es = 0u16;
    while (1u32 << (es + 1)) <= nt as u32 {
        es += 1;
    }
    let mut out = Vec::new();
    be32(&mut out, 0x0001_0000);
    be16(&mut out, nt);
    be16(&mut out, 16 * (1 << es));
    be16(&mut out, es);
    be16(&mut out, nt * 16 - 16 * (1 << es));
    let mut off = 12 + 16 * tables.len();
    let mut head_off = 0;
    for (tag, d) in &tables {
        out.extend_from_slice(*tag);
        be32(&mut out, checksum(d));
        be32(&mut out, off as u32);
        be32(&mut out, d.len() as u32);
        if *tag == b"head" {
            head_off = off;
        }
        off += (d.len() + 3) & !3;
    }
    for (_, d) in &tables {
        out.extend_from_slice(d);
        while out.len() % 4 != 0 {
            out.push(0);
        }
    }
    let adj = 0xB1B0_AFBAu32.wrapping_sub(checksum(&out));
    out[head_off + 8..head_off + 12].copy_from_slice(&adj.to_be_bytes());
    out
}

/// Glyph outlines of a TrueType/OpenType font for the characters it maps (BMP
/// only), scaled to `UPEM`.
pub fn from_ttf(data: &[u8]) -> Result<SwfFont, String> {
    let face = ttf_parser::Face::parse(data, 0).map_err(|e| format!("cannot read font: {e}"))?;
    let scale = UPEM as f32 / face.units_per_em() as f32;
    let sc = |v: f32| (v * scale).round() as i32;
    let mut codes: Vec<u16> = Vec::new();
    if let Some(cmap) = face.tables().cmap {
        for st in cmap.subtables {
            if st.is_unicode() {
                st.codepoints(|c| {
                    if c > 0 && c <= 0xFFFE && !(0xD800..0xE000).contains(&c) {
                        codes.push(c as u16);
                    }
                });
            }
        }
    }
    codes.sort_unstable();
    codes.dedup();
    let mut glyphs = Vec::new();
    for c in codes {
        let Some(gid) = face.glyph_index(char::from_u32(c as u32).unwrap_or('\0')) else { continue };
        let mut b = Builder::default();
        face.outline_glyph(gid, &mut b);
        let contours = b
            .done
            .into_iter()
            .map(|c| c.into_iter().map(|(x, y, on)| OPt { x: sc(x), y: sc(y), on }).collect())
            .collect();
        glyphs.push(FontGlyph { code: c, advance: sc(face.glyph_hor_advance(gid).unwrap_or(0) as f32), contours });
    }
    let name = face
        .names()
        .into_iter()
        .find(|n| n.name_id == ttf_parser::name_id::FAMILY && n.is_unicode())
        .and_then(|n| n.to_string())
        .unwrap_or_else(|| "Font".into());
    Ok(SwfFont {
        name,
        bold: face.is_bold(),
        italic: face.is_italic(),
        ascent: sc(face.ascender() as f32).max(0),
        descent: sc(-(face.descender() as f32)).max(0),
        leading: sc(face.line_gap() as f32),
        glyphs,
    })
}

#[derive(Default)]
struct Builder {
    cur: Vec<(f32, f32, bool)>,
    last: (f32, f32),
    done: Vec<Vec<(f32, f32, bool)>>,
}

impl Builder {
    fn finish(&mut self) {
        let mut c = std::mem::take(&mut self.cur);
        if c.len() > 1 && c.first() == c.last() {
            c.pop();
        }
        if c.len() >= 2 {
            self.done.push(c);
        }
    }
}

impl ttf_parser::OutlineBuilder for Builder {
    fn move_to(&mut self, x: f32, y: f32) {
        self.finish();
        self.cur.push((x, y, true));
        self.last = (x, y);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.cur.push((x, y, true));
        self.last = (x, y);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.cur.push((x1, y1, false));
        self.cur.push((x, y, true));
        self.last = (x, y);
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        use ac_core::geom::pt;
        let p0 = pt(self.last.0, self.last.1);
        for (c, e) in crate::swf_write::cubic_to_quads(p0, pt(x1, y1), pt(x2, y2), pt(x, y)) {
            self.cur.push((c.x, c.y, false));
            self.cur.push((e.x, e.y, true));
        }
        self.last = (x, y);
    }
    fn close(&mut self) {
        self.finish();
    }
}
