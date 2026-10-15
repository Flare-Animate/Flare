//! SWF → Document. Reads bitmaps (DefineBits, JPEG2/3, Lossless/2), shapes
//! (DefineShape 1-4), sprites and the main timeline. Every imported item
//! remembers the character id it had (`BitmapAsset::swf_char_id`,
//! `Document::swf_char_ids`) and bitmaps keep their encoded bytes, so
//! `ac_swf_write::export` can write them back with the same ids and bytes.
//!
//! Fonts, text, buttons, sounds, actions and filters are not imported yet;
//! each kind found is reported in the warnings.
//!
//! Built from the public SWF File Format Specification.

use crate::swf::*;
use ac_core::bitmap::{BitmapAsset, BitmapCodec};
use ac_core::geom::{Matrix, Path, Pt, Seg};
use ac_core::model::*;
use std::collections::{BTreeMap, HashMap};

pub struct Imported {
    pub doc: Document,
    pub warnings: Vec<String>,
}

pub fn import(bytes: &[u8]) -> Result<Imported, String> {
    let swf = parse(bytes)?;
    let mut doc = Document {
        width: (swf.stage[1] - swf.stage[0]) as f32 / TWIPS,
        height: (swf.stage[3] - swf.stage[2]) as f32 / TWIPS,
        fps: swf.frame_rate,
        ..Default::default()
    };
    let mut warn: Vec<String> = Vec::new();
    let mut unsupported: BTreeMap<u16, usize> = BTreeMap::new();

    // names from SymbolClass / ExportAssets
    let mut names: HashMap<u16, String> = HashMap::new();
    for t in &swf.tags {
        if t.code == 76 || t.code == 56 {
            let mut r = BitR::new(&t.body);
            let n = r.u16();
            for _ in 0..n {
                let id = r.u16();
                let name = r.string();
                if id != 0 {
                    names.insert(id, name);
                }
            }
        }
    }

    // pass 1: bitmaps
    let mut jpeg_tables: Option<Vec<u8>> = None;
    let mut bitmap_of: HashMap<u16, u32> = HashMap::new();
    for t in &swf.tags {
        let made = match t.code {
            JPEG_TABLES => {
                jpeg_tables = Some(t.body.clone());
                None
            }
            DEFINE_BITS | DEFINE_BITS_JPEG2 | DEFINE_BITS_JPEG3 | DEFINE_BITS_LOSSLESS | DEFINE_BITS_LOSSLESS2 => Some(read_bitmap(t, jpeg_tables.as_deref())),
            _ => None,
        };
        if let Some(res) = made {
            let ch = u16::from_le_bytes([t.body.first().copied().unwrap_or(0), t.body.get(1).copied().unwrap_or(0)]);
            match res {
                Ok(mut a) => {
                    a.id = doc.next_bitmap_id();
                    a.name = names.get(&ch).cloned().unwrap_or_else(|| format!("Bitmap {ch}"));
                    a.swf_char_id = Some(ch);
                    bitmap_of.insert(ch, a.id);
                    doc.bitmaps.push(a);
                }
                Err(e) => warn.push(format!("bitmap {ch}: {e}")),
            }
        }
    }

    // pass 2: symbol ids for every shape and sprite
    let mut symbol_of: HashMap<u16, SymbolId> = HashMap::new();
    for t in &swf.tags {
        if matches!(t.code, DEFINE_SHAPE | DEFINE_SHAPE2 | DEFINE_SHAPE3 | DEFINE_SHAPE4 | DEFINE_SPRITE) && t.body.len() >= 2 {
            let ch = u16::from_le_bytes([t.body[0], t.body[1]]);
            let id = doc.next_symbol_id;
            doc.next_symbol_id += 1;
            symbol_of.insert(ch, id);
            doc.swf_char_ids.insert(id, ch);
        }
    }
    // pass 3: the symbols themselves
    let mut ctx = Ctx { symbol_of: &symbol_of, bitmap_of: &bitmap_of, warn: &mut warn };
    for t in &swf.tags {
        match t.code {
            DEFINE_SHAPE | DEFINE_SHAPE2 | DEFINE_SHAPE3 | DEFINE_SHAPE4 => {
                let ch = u16::from_le_bytes([t.body[0], t.body[1]]);
                let shapes = read_shape(t.code, &t.body, &mut ctx);
                let name = names.get(&ch).cloned().unwrap_or_else(|| format!("Shape {ch}"));
                let mut tl = Timeline::new(&name);
                tl.layers[0].keyframes.insert(0, Keyframe::with(shapes.into_iter().map(|s| Element::shape("", s)).collect()));
                doc.library.push(Symbol { id: symbol_of[&ch], name, kind: SymbolKind::Graphic, timeline: tl });
            }
            DEFINE_SPRITE => {
                let ch = u16::from_le_bytes([t.body[0], t.body[1]]);
                let inner = parse_tags(&t.body[4.min(t.body.len())..]);
                let name = names.get(&ch).cloned().unwrap_or_else(|| format!("Sprite {ch}"));
                let tl = timeline(&name, &inner, &mut ctx);
                doc.library.push(Symbol { id: symbol_of[&ch], name, kind: SymbolKind::MovieClip, timeline: tl });
            }
            DEFINE_BITS | JPEG_TABLES | DEFINE_BITS_JPEG2 | DEFINE_BITS_JPEG3 | DEFINE_BITS_LOSSLESS | DEFINE_BITS_LOSSLESS2 => {}
            SET_BACKGROUND if t.body.len() >= 3 => doc.background = Color::rgb(t.body[0], t.body[1], t.body[2]),
            c if is_character_tag(c) => *unsupported.entry(c).or_default() += 1,
            _ => {}
        }
    }
    doc.scene = timeline("Scene 1", &swf.tags, &mut ctx);
    // class names -> linkage
    for t in &swf.tags {
        if t.code == 76 {
            let mut r = BitR::new(&t.body);
            for _ in 0..r.u16() {
                let (id, name) = (r.u16(), r.string());
                if let Some(&s) = symbol_of.get(&id) {
                    doc.linkage.insert(s, name);
                }
            }
        }
    }
    for (c, n) in unsupported {
        warn.push(format!("{n} tag(s) of type {c} ({}) not imported", tag_name(c)));
    }
    doc.warnings.extend(warn.iter().cloned());
    Ok(Imported { doc, warnings: warn })
}

fn is_character_tag(c: u16) -> bool {
    // buttons, fonts, text, sounds, morph shapes, video, binary data
    matches!(c, 7 | 10 | 11 | 13 | 14 | 33 | 34 | 37 | 46 | 48 | 60 | 62 | 73 | 74 | 75 | 84 | 87 | 88 | 91)
}

fn tag_name(c: u16) -> &'static str {
    match c {
        10 | 48 | 75 | 91 => "font",
        11 | 33 | 37 | 74 => "text",
        7 | 34 => "button",
        14 | 60 | 18 | 19 => "sound/video",
        _ => "unsupported",
    }
}

struct Ctx<'a> {
    symbol_of: &'a HashMap<u16, SymbolId>,
    bitmap_of: &'a HashMap<u16, u32>,
    warn: &'a mut Vec<String>,
}

// ---------------------------------------------------------------- bitmaps

fn read_bitmap(t: &RawTag, tables: Option<&[u8]>) -> Result<BitmapAsset, String> {
    let b = &t.body;
    if b.len() < 2 {
        return Err("truncated".into());
    }
    let data = &b[2..];
    match t.code {
        DEFINE_BITS => {
            let tables = tables.ok_or("DefineBits without JPEGTables")?;
            // tables end with FFD9, the image starts with FFD8: splice
            let mut jpeg = tables.strip_suffix(&[0xFF, 0xD9]).unwrap_or(tables).to_vec();
            jpeg.extend_from_slice(data.strip_prefix(&[0xFF, 0xD8]).unwrap_or(data));
            from_encoded(&jpeg, BitmapCodec::Jpeg, jpeg.clone())
        }
        DEFINE_BITS_JPEG2 => from_encoded(strip_bad_header(data), sniff(data), data.to_vec()),
        DEFINE_BITS_JPEG3 => {
            if data.len() < 4 {
                return Err("truncated".into());
            }
            let off = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
            let img = data.get(4..4 + off).ok_or("alpha offset out of range")?;
            let mut a = from_encoded(strip_bad_header(img), BitmapCodec::SwfJpeg3, data.to_vec())?;
            let alpha = &data[4 + off..];
            if !alpha.is_empty() {
                let alpha = unzlib(alpha)?;
                let mut rgba = rgba_of(&a)?;
                if alpha.len() >= rgba.len() / 4 {
                    for (px, al) in rgba.chunks_exact_mut(4).zip(&alpha) {
                        px[3] = *al;
                        // JPEG3 colours are premultiplied
                        unmul(px);
                    }
                    let mut n = BitmapAsset::from_rgba(0, "", a.width, a.height, &rgba)?;
                    n.set_original(BitmapCodec::SwfJpeg3, data.to_vec());
                    a = n;
                }
            }
            Ok(a)
        }
        DEFINE_BITS_LOSSLESS | DEFINE_BITS_LOSSLESS2 => {
            let v2 = t.code == DEFINE_BITS_LOSSLESS2;
            if data.len() < 5 {
                return Err("truncated".into());
            }
            let (fmt, w, h) = (data[0], u16::from_le_bytes([data[1], data[2]]) as usize, u16::from_le_bytes([data[3], data[4]]) as usize);
            let (table, rest) = if fmt == 3 {
                (*data.get(5).ok_or("truncated")? as usize + 1, &data[6..])
            } else {
                (0, &data[5..])
            };
            let raw = unzlib(rest)?;
            let mut rgba = vec![0u8; w * h * 4];
            match fmt {
                3 => {
                    let esz = if v2 { 4 } else { 3 };
                    let pal = raw.get(..table * esz).ok_or("short colour table")?;
                    let stride = (w + 3) & !3;
                    for y in 0..h {
                        for x in 0..w {
                            let i = *raw.get(table * esz + y * stride + x).ok_or("short pixel data")? as usize;
                            let e = pal.get(i * esz..i * esz + esz).unwrap_or(&[0, 0, 0, 0][..esz]);
                            let o = (y * w + x) * 4;
                            if v2 {
                                rgba[o..o + 4].copy_from_slice(&[e[0], e[1], e[2], e[3]]);
                                unmul(&mut rgba[o..o + 4]);
                            } else {
                                rgba[o..o + 4].copy_from_slice(&[e[0], e[1], e[2], 255]);
                            }
                        }
                    }
                }
                4 if !v2 => {
                    let stride = (w * 2 + 3) & !3;
                    for y in 0..h {
                        for x in 0..w {
                            let o = y * stride + x * 2;
                            let v = u16::from_be_bytes([*raw.get(o).ok_or("short pixel data")?, *raw.get(o + 1).ok_or("short pixel data")?]);
                            let c = |s: u32| {
                                let v5 = ((v >> s) & 31) as u32;
                                ((v5 << 3) | (v5 >> 2)) as u8
                            };
                            let d = (y * w + x) * 4;
                            rgba[d..d + 4].copy_from_slice(&[c(10), c(5), c(0), 255]);
                        }
                    }
                }
                5 => {
                    if raw.len() < w * h * 4 {
                        return Err("short pixel data".into());
                    }
                    for (i, px) in raw.chunks_exact(4).take(w * h).enumerate() {
                        let o = i * 4;
                        if v2 {
                            rgba[o..o + 4].copy_from_slice(&[px[1], px[2], px[3], px[0]]);
                            unmul(&mut rgba[o..o + 4]);
                        } else {
                            rgba[o..o + 4].copy_from_slice(&[px[1], px[2], px[3], 255]);
                        }
                    }
                }
                f => return Err(format!("unknown bitmap format {f}")),
            }
            let mut a = BitmapAsset::from_rgba(0, "", w as u32, h as u32, &rgba)?;
            a.set_original(if v2 { BitmapCodec::SwfLossless2 } else { BitmapCodec::SwfLossless }, data.to_vec());
            Ok(a)
        }
        _ => Err("not a bitmap".into()),
    }
}

fn unmul(px: &mut [u8]) {
    let a = px[3] as u32;
    if a != 0 && a != 255 {
        for c in &mut px[..3] {
            *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
        }
    }
}

fn strip_bad_header(d: &[u8]) -> &[u8] {
    // old exporters wrote FF D9 FF D8 before the image
    d.strip_prefix(&[0xFF, 0xD9, 0xFF, 0xD8]).map_or(d, |_| &d[4..])
}

fn sniff(d: &[u8]) -> BitmapCodec {
    if d.starts_with(&[0x89, b'P', b'N', b'G']) {
        BitmapCodec::Png
    } else if d.starts_with(b"GIF8") {
        BitmapCodec::Gif
    } else {
        BitmapCodec::Jpeg
    }
}

fn from_encoded(img: &[u8], codec: BitmapCodec, keep: Vec<u8>) -> Result<BitmapAsset, String> {
    let rgba = image::load_from_memory(img).map_err(|e| format!("cannot decode image: {e}"))?.to_rgba8();
    let (w, h) = rgba.dimensions();
    let mut a = BitmapAsset::from_rgba(0, "", w, h, rgba.as_raw())?;
    a.set_original(codec, keep);
    Ok(a)
}

fn rgba_of(a: &BitmapAsset) -> Result<Vec<u8>, String> {
    Ok(image::load_from_memory(&a.png).map_err(|e| e.to_string())?.to_rgba8().into_raw())
}

// ----------------------------------------------------------------- shapes

type Tw = (i32, i32);

#[derive(Clone, Debug)]
enum FillSt {
    Solid(Color),
    Bitmap(u16, Matrix),
}

#[derive(Clone, Copy, Debug)]
enum Kind {
    Line,
    Quad(Tw),
}

#[derive(Clone, Copy, Debug)]
struct Edge {
    from: Tw,
    to: Tw,
    kind: Kind,
    fill0: usize,
    fill1: usize,
    line: usize,
}

fn read_color(r: &mut BitR, alpha: bool) -> Color {
    let (cr, g, b) = (r.u8(), r.u8(), r.u8());
    Color { r: cr, g, b, a: if alpha { r.u8() } else { 255 } }
}

fn read_matrix(r: &mut BitR) -> Matrix {
    r.align();
    let (mut a, mut d, mut b, mut c) = (1.0, 1.0, 0.0, 0.0);
    if r.flag() {
        let n = r.ub(5);
        a = r.sb(n) as f32 / 65536.0;
        d = r.sb(n) as f32 / 65536.0;
    }
    if r.flag() {
        let n = r.ub(5);
        b = r.sb(n) as f32 / 65536.0;
        c = r.sb(n) as f32 / 65536.0;
    }
    let n = r.ub(5);
    let (tx, ty) = (r.sb(n) as f32 / TWIPS, r.sb(n) as f32 / TWIPS);
    r.align();
    Matrix { a, b, c, d, tx, ty }
}

/// A fill-style array; also returns (width, colour) of line styles via `lines`.
fn read_styles(r: &mut BitR, code: u16, fills: &mut Vec<FillSt>, lines: &mut Vec<(f32, Color)>, warn: &mut Vec<String>) {
    let alpha = code >= DEFINE_SHAPE3;
    let mut n = r.u8() as usize;
    if n == 0xFF && code >= DEFINE_SHAPE2 {
        n = r.u16() as usize;
    }
    for _ in 0..n {
        let ty = r.u8();
        let f = match ty {
            0x00 => FillSt::Solid(read_color(r, alpha)),
            0x10 | 0x12 | 0x13 => {
                read_matrix(r);
                // GRADIENT: spread/interpolation bits, count, records
                let flags = r.u8();
                let cnt = (flags & 0x0F) as usize;
                let mut first = None;
                for _ in 0..cnt {
                    let _ratio = r.u8();
                    let c = read_color(r, alpha);
                    first.get_or_insert(c);
                }
                if ty == 0x13 {
                    r.u16();
                }
                warn.push("gradient fills imported as their first colour".into());
                FillSt::Solid(first.unwrap_or(Color::rgb(0, 0, 0)))
            }
            0x40..=0x43 => {
                let id = r.u16();
                let m = read_matrix(r);
                // bitmap-space is 20 twips per pixel: undo that scale
                FillSt::Bitmap(id, Matrix { a: m.a / 20.0, b: m.b / 20.0, c: m.c / 20.0, d: m.d / 20.0, tx: m.tx, ty: m.ty })
            }
            _ => FillSt::Solid(Color::rgb(0, 0, 0)),
        };
        fills.push(f);
    }
    let mut n = r.u8() as usize;
    if n == 0xFF && code >= DEFINE_SHAPE2 {
        n = r.u16() as usize;
    }
    for _ in 0..n {
        if code == DEFINE_SHAPE4 {
            let w = r.u16();
            let b0 = r.u8();
            let b1 = r.u8();
            let join = (b0 >> 4) & 3;
            let has_fill = (b0 >> 3) & 1 == 1;
            let _ = b1;
            if join == 2 {
                r.u16();
            }
            let c = if has_fill {
                let ty = r.u8();
                match ty {
                    0x00 => read_color(r, true),
                    0x40..=0x43 => {
                        r.u16();
                        read_matrix(r);
                        Color::rgb(0, 0, 0)
                    }
                    _ => {
                        read_matrix(r);
                        let cnt = (r.u8() & 0x0F) as usize;
                        let mut first = None;
                        for _ in 0..cnt {
                            r.u8();
                            first.get_or_insert(read_color(r, true));
                        }
                        if ty == 0x13 {
                            r.u16();
                        }
                        first.unwrap_or(Color::rgb(0, 0, 0))
                    }
                }
            } else {
                read_color(r, true)
            };
            lines.push((w as f32 / TWIPS, c));
        } else {
            let w = r.u16();
            let c = read_color(r, alpha);
            lines.push((w as f32 / TWIPS, c));
        }
    }
}

fn read_shape(code: u16, body: &[u8], ctx: &mut Ctx) -> Vec<Shape> {
    let mut r = BitR::new(body);
    r.u16();
    let n = r.ub(5);
    for _ in 0..4 {
        r.sb(n);
    }
    r.align();
    if code == DEFINE_SHAPE4 {
        let n = r.ub(5);
        for _ in 0..4 {
            r.sb(n);
        }
        r.align();
        r.u8();
    }
    let (mut fills, mut lines) = (Vec::new(), Vec::new());
    read_styles(&mut r, code, &mut fills, &mut lines, ctx.warn);
    let mut edges: Vec<Edge> = Vec::new();
    let (mut fbits, mut lbits) = (r.ub(4), r.ub(4));
    let (mut fbase, mut lbase) = (0usize, 0usize);
    let (mut pos, mut f0, mut f1, mut ln) = ((0i32, 0i32), 0usize, 0usize, 0usize);
    let sty = |v: u32, base: usize| if v == 0 { 0 } else { v as usize + base };
    loop {
        if r.eof() {
            break;
        }
        if !r.flag() {
            let flags = r.ub(5);
            if flags == 0 {
                break;
            }
            if flags & 1 != 0 {
                let n = r.ub(5);
                pos = (r.sb(n), r.sb(n));
            }
            if flags & 2 != 0 {
                f0 = sty(r.ub(fbits), fbase);
            }
            if flags & 4 != 0 {
                f1 = sty(r.ub(fbits), fbase);
            }
            if flags & 8 != 0 {
                ln = sty(r.ub(lbits), lbase);
            }
            if flags & 16 != 0 && code >= DEFINE_SHAPE2 {
                r.align();
                fbase = fills.len();
                lbase = lines.len();
                read_styles(&mut r, code, &mut fills, &mut lines, ctx.warn);
                fbits = r.ub(4);
                lbits = r.ub(4);
            }
        } else if r.flag() {
            let n = r.ub(4) + 2;
            let (dx, dy) = if r.flag() {
                (r.sb(n), r.sb(n))
            } else if r.flag() {
                (0, r.sb(n))
            } else {
                (r.sb(n), 0)
            };
            let to = (pos.0 + dx, pos.1 + dy);
            edges.push(Edge { from: pos, to, kind: Kind::Line, fill0: f0, fill1: f1, line: ln });
            pos = to;
        } else {
            let n = r.ub(4) + 2;
            let c = (pos.0 + r.sb(n), pos.1 + r.sb(n));
            let to = (c.0 + r.sb(n), c.1 + r.sb(n));
            edges.push(Edge { from: pos, to, kind: Kind::Quad(c), fill0: f0, fill1: f1, line: ln });
            pos = to;
        }
    }
    build_shapes(&edges, &fills, &lines, ctx)
}

fn px(p: Tw) -> Pt {
    Pt { x: p.0 as f32 / TWIPS, y: p.1 as f32 / TWIPS }
}

fn reverse_edge(e: &Edge) -> Edge {
    Edge { from: e.to, to: e.from, ..*e }
}

/// Join directed edges end to start into paths.
fn stitch(edges: Vec<Edge>) -> Path {
    let mut by_start: HashMap<Tw, Vec<usize>> = HashMap::new();
    for (i, e) in edges.iter().enumerate() {
        by_start.entry(e.from).or_default().push(i);
    }
    let mut used = vec![false; edges.len()];
    let mut segs = Vec::new();
    for i in 0..edges.len() {
        if used[i] {
            continue;
        }
        used[i] = true;
        let first = edges[i].from;
        segs.push(Seg::M(px(first)));
        let mut cur = i;
        loop {
            let e = &edges[cur];
            segs.push(match e.kind {
                Kind::Line => Seg::L(px(e.to)),
                Kind::Quad(c) => Seg::Q(px(c), px(e.to)),
            });
            if e.to == first {
                segs.push(Seg::Z);
                break;
            }
            let next = by_start.get(&e.to).and_then(|v| v.iter().copied().find(|&j| !used[j]));
            match next {
                Some(j) => {
                    used[j] = true;
                    cur = j;
                }
                None => break,
            }
        }
    }
    Path { segs }
}

fn build_shapes(edges: &[Edge], fills: &[FillSt], lines: &[(f32, Color)], ctx: &mut Ctx) -> Vec<Shape> {
    let mut out = Vec::new();
    for (fi, f) in fills.iter().enumerate() {
        let id = fi + 1;
        let mut es: Vec<Edge> = Vec::new();
        for e in edges {
            if e.fill1 == id {
                es.push(*e);
            }
            if e.fill0 == id {
                es.push(reverse_edge(e));
            }
        }
        if es.is_empty() {
            continue;
        }
        let path = stitch(es);
        let mut s = Shape::new(path, None, None);
        match f {
            FillSt::Solid(c) => s.fill = Some(*c),
            FillSt::Bitmap(ch, m) => match ctx.bitmap_of.get(ch) {
                Some(&bid) => s.bitmap_fill = Some(BitmapFill { bitmap: bid, matrix: *m }),
                None => {
                    ctx.warn.push(format!("shape fills with missing bitmap {ch}"));
                    s.fill = Some(Color::rgb(255, 0, 255));
                }
            },
        }
        out.push(s);
    }
    for (li, (w, c)) in lines.iter().enumerate() {
        let id = li + 1;
        let es: Vec<Edge> = edges.iter().filter(|e| e.line == id).copied().collect();
        if es.is_empty() {
            continue;
        }
        let path = stitch(es);
        out.push(Shape::new(path, None, Some(Stroke { color: *c, width: *w })));
    }
    out
}

// -------------------------------------------------------------- timelines

#[derive(Clone, PartialEq)]
struct Slot {
    ch: u16,
    m: Matrix,
    cx: Option<[i32; 8]>, // mul rgba (8.8), add rgba
    name: String,
}

fn read_cxform(r: &mut BitR, alpha: bool) -> [i32; 8] {
    r.align();
    let has_add = r.flag();
    let has_mul = r.flag();
    let n = r.ub(4);
    let mut v = [256, 256, 256, 256, 0, 0, 0, 0];
    let k = if alpha { 4 } else { 3 };
    if has_mul {
        for x in v.iter_mut().take(k) {
            *x = r.sb(n);
        }
    }
    if has_add {
        for x in v.iter_mut().skip(4).take(k) {
            *x = r.sb(n);
        }
    }
    r.align();
    v
}

fn timeline(name: &str, tags: &[RawTag], ctx: &mut Ctx) -> Timeline {
    let mut depths: BTreeMap<u16, Slot> = BTreeMap::new();
    // per depth: (frame, state) changes
    let mut track: BTreeMap<u16, Vec<(u32, Option<Slot>)>> = BTreeMap::new();
    let mut dirty: Vec<u16> = Vec::new();
    let mut frame = 0u32;
    let mut labels: BTreeMap<u32, String> = BTreeMap::new();
    for t in tags {
        match t.code {
            PLACE_OBJECT => {
                let mut r = BitR::new(&t.body);
                let (ch, depth) = (r.u16(), r.u16());
                let m = read_matrix(&mut r);
                let cx = (!r.eof()).then(|| read_cxform(&mut r, false));
                depths.insert(depth, Slot { ch, m, cx, name: String::new() });
                dirty.push(depth);
            }
            PLACE_OBJECT2 | PLACE_OBJECT3 => {
                let mut r = BitR::new(&t.body);
                let f = r.u8();
                let f3 = if t.code == PLACE_OBJECT3 { r.u8() } else { 0 };
                let depth = r.u16();
                if f3 & 0x08 != 0 || (f3 & 0x10 != 0 && f & 0x02 != 0) {
                    r.string();
                }
                let ch = (f & 0x02 != 0).then(|| r.u16());
                let m = (f & 0x04 != 0).then(|| read_matrix(&mut r));
                let cx = (f & 0x08 != 0).then(|| read_cxform(&mut r, true));
                if f & 0x10 != 0 {
                    r.u16();
                }
                let name = (f & 0x20 != 0).then(|| r.string());
                let mv = f & 0x01 != 0;
                let old = depths.get(&depth).cloned();
                let new = match (ch, mv, old) {
                    (Some(c), _, old) => Slot { ch: c, m: m.unwrap_or(Matrix::IDENTITY), cx: cx.or(if mv { old.as_ref().and_then(|o| o.cx) } else { None }), name: name.unwrap_or_else(|| if mv { old.map(|o| o.name).unwrap_or_default() } else { String::new() }) },
                    (None, true, Some(o)) => Slot { ch: o.ch, m: m.unwrap_or(o.m), cx: cx.or(o.cx), name: name.unwrap_or(o.name) },
                    _ => continue,
                };
                depths.insert(depth, new);
                dirty.push(depth);
            }
            REMOVE_OBJECT | REMOVE_OBJECT2 => {
                let mut r = BitR::new(&t.body);
                let depth = if t.code == REMOVE_OBJECT { r.u16(); r.u16() } else { r.u16() };
                if depths.remove(&depth).is_some() {
                    dirty.push(depth);
                }
            }
            FRAME_LABEL => {
                labels.insert(frame, BitR::new(&t.body).string());
            }
            SHOW_FRAME => {
                dirty.sort_unstable();
                dirty.dedup();
                for d in dirty.drain(..) {
                    let st = depths.get(&d).cloned();
                    let tr = track.entry(d).or_default();
                    if tr.last().map(|l| &l.1) != Some(&st) {
                        tr.push((frame, st));
                    }
                }
                frame += 1;
            }
            _ => {}
        }
    }
    let frames = frame.max(1);
    let mut tl = Timeline { name: name.into(), layers: Vec::new() };
    // highest depth on top
    for (d, changes) in track.iter().rev() {
        let mut layer = Layer::new(&format!("Depth {d}"), tl.layers.len());
        layer.keyframes.clear();
        layer.end = frames;
        if changes.first().map(|c| c.0) != Some(0) {
            layer.keyframes.insert(0, Keyframe::blank());
        }
        for (f, st) in changes {
            let kf = match st {
                None => Keyframe::blank(),
                Some(s) => match ctx.symbol_of.get(&s.ch) {
                    Some(&sym) => {
                        let mut e = Element::instance(&s.name, sym, s.m);
                        if let Some(c) = s.cx {
                            let rgb_id = c[..3] == [256, 256, 256] && c[4..7] == [0, 0, 0];
                            if rgb_id && c[7] == 0 {
                                e.alpha = c[3] as f32 / 256.0;
                            } else {
                                e.color_fx = Some(ColorXform { mul: [c[0] as f32 / 256.0, c[1] as f32 / 256.0, c[2] as f32 / 256.0, c[3] as f32 / 256.0], add: [c[4] as f32, c[5] as f32, c[6] as f32, c[7] as f32] });
                            }
                        }
                        Keyframe::with(vec![e])
                    }
                    None => {
                        ctx.warn.push(format!("depth {d} places character {} which is not a shape or sprite (text, button, font...): skipped", s.ch));
                        Keyframe::blank()
                    }
                },
            };
            layer.keyframes.insert(*f, kf);
        }
        tl.layers.push(layer);
    }
    if tl.layers.is_empty() {
        tl = Timeline::new(name);
        tl.layers[0].end = frames;
    }
    for (f, l) in labels {
        if let Some(layer) = tl.layers.last_mut() {
            if !layer.keyframes.contains_key(&f) {
                // a keyframe equal to the one in force, carrying the label
                let held = layer.keyframes.range(..f).next_back().map(|(_, k)| k.clone()).unwrap_or_else(Keyframe::blank);
                layer.keyframes.insert(f, held);
            }
            if let Some(k) = layer.keyframes.get_mut(&f) {
                k.label = Some(l);
            }
        }
    }
    tl
}
