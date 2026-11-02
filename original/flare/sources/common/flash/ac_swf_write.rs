//! Document → SWF. Library bitmaps become DefineBitsJPEG2/3 or
//! DefineBitsLossless(2) (byte-identical to the imported encoding while the
//! pixels are unedited), vector shapes DefineShape3, symbols DefineSprite;
//! the scene's timeline becomes PlaceObject2/RemoveObject2/ShowFrame.
//!
//! Character ids the document remembers from a SWF import
//! (`BitmapAsset::swf_char_id`, `Document::swf_char_ids`) are reused; new
//! characters get fresh ids above the largest one, so nothing collides.
//!
//! Built from the public SWF File Format Specification.

use crate::swf::*;
use ac_core::bitmap::{BitmapAsset, BitmapCodec, BitmapId};
use ac_core::geom::{Matrix, Pt, Seg};
use ac_core::model::*;
use std::collections::{BTreeMap, HashMap};

/// The SWF plus what could not be carried over and the ids that were used.
pub struct Exported {
    pub bytes: Vec<u8>,
    pub warnings: Vec<String>,
    pub bitmap_ids: BTreeMap<BitmapId, u16>,
    pub symbol_ids: BTreeMap<SymbolId, u16>,
}

/// SWF version written: 11 has everything used here.
pub const VERSION: u8 = 11;

pub fn export(doc: &Document) -> Result<Exported, String> {
    let (doc, mut warnings) = ac_core::frames::for_interchange(&doc.with_floats_committed());
    let reserved: Vec<u16> = doc.bitmaps.iter().filter_map(|b| b.swf_char_id).chain(doc.swf_char_ids.values().copied()).collect();
    let mut x = X { doc: &doc, ids: CharIds::new(reserved), defs: vec![], bitmap_ids: BTreeMap::new(), symbol_ids: BTreeMap::new(), shape_cache: HashMap::new(), bitmap_shapes: HashMap::new(), state: HashMap::new(), warn: vec![] };
    for b in &doc.bitmaps {
        let id = x.ids.claim(b.swf_char_id)?;
        x.bitmap_ids.insert(b.id, id);
    }
    for s in &doc.library {
        let id = x.ids.claim(doc.swf_char_ids.get(&s.id).copied())?;
        x.symbol_ids.insert(s.id, id);
    }
    for b in &doc.bitmaps {
        let t = x.bitmap_tag(b);
        x.defs.push(t);
    }
    for s in &doc.library {
        x.symbol(s.id)?;
    }
    // main timeline: the scenes play one after another
    let mut frames = Vec::new();
    for tl in doc.scenes() {
        for f in 0..tl.frame_count() {
            frames.push((tl, f));
        }
    }
    let n_frames = frames.len();
    let body = x.timeline_frames(&frames, 0)?;
    let mut tags = Vec::new();
    let bg = doc.background;
    tags.push(RawTag { code: SET_BACKGROUND, body: vec![bg.r, bg.g, bg.b] });
    tags.append(&mut x.defs);
    tags.extend(body);
    if n_frames > u16::MAX as usize {
        return Err("over 65535 frames".into());
    }
    let stage = [0, twips(doc.width), 0, twips(doc.height)];
    let bytes = write(VERSION, stage, doc.fps, n_frames as u16, &tags, true);
    warnings.append(&mut x.warn);
    warnings.sort();
    warnings.dedup();
    Ok(Exported { bytes, warnings, bitmap_ids: x.bitmap_ids, symbol_ids: x.symbol_ids })
}

#[derive(Clone, Copy, PartialEq)]
enum St {
    Building,
    Done,
}

struct X<'a> {
    doc: &'a Document,
    ids: CharIds,
    /// Character definitions, each after what it uses.
    defs: Vec<RawTag>,
    bitmap_ids: BTreeMap<BitmapId, u16>,
    symbol_ids: BTreeMap<SymbolId, u16>,
    shape_cache: HashMap<String, u16>,
    bitmap_shapes: HashMap<BitmapId, u16>,
    state: HashMap<SymbolId, St>,
    warn: Vec<String>,
}

/// One thing at a depth.
#[derive(Clone, PartialEq)]
struct Placed {
    ch: u16,
    m: Matrix,
    cx: Option<ColorXform>,
    name: String,
}

impl<'a> X<'a> {
    // ------------------------------------------------------------ bitmaps

    fn bitmap_tag(&mut self, b: &BitmapAsset) -> RawTag {
        let id = self.bitmap_ids[&b.id];
        if let Some(o) = b.pristine_original() {
            let code = match o.codec {
                BitmapCodec::Jpeg | BitmapCodec::Png | BitmapCodec::Gif => DEFINE_BITS_JPEG2,
                BitmapCodec::SwfLossless => DEFINE_BITS_LOSSLESS,
                BitmapCodec::SwfLossless2 => DEFINE_BITS_LOSSLESS2,
                BitmapCodec::SwfJpeg3 => DEFINE_BITS_JPEG3,
            };
            let mut body = id.to_le_bytes().to_vec();
            body.extend_from_slice(&o.bytes);
            return RawTag { code, body };
        }
        let (w, h, rgba) = match decode_rgba(&b.png) {
            Some(v) => v,
            None => {
                self.warn.push(format!("bitmap '{}' could not be decoded: written as a 1x1 transparent image", b.name));
                (1, 1, vec![0; 4])
            }
        };
        let mut argb = Vec::with_capacity(rgba.len());
        for p in rgba.chunks_exact(4) {
            let a = p[3] as u16;
            let pm = |c: u8| ((c as u16 * a + 127) / 255) as u8;
            argb.extend_from_slice(&[p[3], pm(p[0]), pm(p[1]), pm(p[2])]);
        }
        let mut body = id.to_le_bytes().to_vec();
        body.push(5);
        body.extend_from_slice(&(w as u16).to_le_bytes());
        body.extend_from_slice(&(h as u16).to_le_bytes());
        body.extend_from_slice(&zlib(&argb));
        RawTag { code: DEFINE_BITS_LOSSLESS2, body }
    }

    // ------------------------------------------------------------ symbols

    /// Define symbol `id` (and what it uses). Returns its character id.
    fn symbol(&mut self, id: SymbolId) -> Result<u16, String> {
        let ch = *self.symbol_ids.get(&id).ok_or_else(|| format!("instance of missing symbol {id}"))?;
        match self.state.get(&id) {
            Some(St::Done) => return Ok(ch),
            Some(St::Building) => return Err(format!("symbol {id} contains itself")),
            None => {}
        }
        self.state.insert(id, St::Building);
        let doc = self.doc;
        let sym = doc.library.iter().find(|s| s.id == id).ok_or("missing symbol")?;
        if sym.kind == SymbolKind::Button {
            self.warn.push(format!("button '{}' exported as a movie clip (button states need DefineButton2)", sym.name));
        }
        if let Some(shape) = lone_shape(&sym.timeline) {
            let tag = self.shape_tag(ch, shape);
            self.defs.push(tag);
        } else {
            let n = sym.timeline.frame_count();
            let frames: Vec<_> = (0..n).map(|f| (&sym.timeline, f)).collect();
            let ctl = self.timeline_frames(&frames, 0)?;
            let mut body = ch.to_le_bytes().to_vec();
            body.extend_from_slice(&(n.min(u16::MAX as u32) as u16).to_le_bytes());
            for t in ctl {
                body.extend_from_slice(&tag_bytes(t.code, &t.body, false));
            }
            body.extend_from_slice(&tag_bytes(END, &[], false));
            self.defs.push(RawTag { code: DEFINE_SPRITE, body });
        }
        self.state.insert(id, St::Done);
        Ok(ch)
    }

    // ----------------------------------------------------------- timeline

    fn timeline_frames(&mut self, frames: &[(&Timeline, u32)], _base: u16) -> Result<Vec<RawTag>, String> {
        // per frame: bottom-to-top list of placements per layer
        let mut per_frame: Vec<Vec<Vec<Placed>>> = Vec::with_capacity(frames.len());
        let mut slots: Vec<usize> = Vec::new();
        let mut labels: Vec<Option<String>> = Vec::new();
        for &(tl, f) in frames {
            let mut layers = Vec::new();
            let mut label = None;
            for (li, layer) in tl.layers.iter().enumerate().rev() {
                let mut v = Vec::new();
                if layer.kind == LayerKind::Normal && layer.visible {
                    if let Some((els, _)) = layer_elements_at(layer, f) {
                        for e in &els {
                            self.flatten(e, &Matrix::IDENTITY, None, &mut v)?;
                        }
                    }
                    if let Some((start, kf, _)) = layer.span_at(f) {
                        if start == f && kf.label_kind != LabelKind::Comment {
                            label = label.or_else(|| kf.label.clone());
                        }
                    }
                } else if layer.kind != LayerKind::Normal {
                    self.warn.push(format!("{:?} layer '{}' is not exported", layer.kind, layer.name));
                }
                if slots.len() <= li {
                    slots.resize(li + 1, 0);
                }
                slots[li] = slots[li].max(v.len());
                layers.push((li, v));
            }
            // store by layer index
            let mut by = vec![Vec::new(); tl.layers.len()];
            for (li, v) in layers {
                by[li] = v;
            }
            per_frame.push(by);
            labels.push(label);
        }
        // depth of (layer, slot): lowest layer first
        let n_layers = slots.len();
        let mut base = vec![0usize; n_layers];
        let mut d = 1usize;
        for li in (0..n_layers).rev() {
            base[li] = d;
            d += slots[li];
        }
        if d > u16::MAX as usize {
            return Err("too many display-list depths".into());
        }
        let mut tags = Vec::new();
        let mut prev: HashMap<u16, Placed> = HashMap::new();
        for (fi, by) in per_frame.iter().enumerate() {
            if let Some(l) = &labels[fi] {
                let mut b = l.as_bytes().to_vec();
                b.push(0);
                tags.push(RawTag { code: FRAME_LABEL, body: b });
            }
            let mut cur: HashMap<u16, Placed> = HashMap::new();
            for (li, v) in by.iter().enumerate() {
                for (j, p) in v.iter().enumerate() {
                    cur.insert((base[li] + j) as u16, p.clone());
                }
            }
            let mut depths: Vec<u16> = prev.keys().chain(cur.keys()).copied().collect();
            depths.sort_unstable();
            depths.dedup();
            for dpt in depths {
                match (prev.get(&dpt), cur.get(&dpt)) {
                    (Some(a), Some(b)) if a == b => {}
                    (Some(a), Some(b)) if a.ch == b.ch => tags.push(place(dpt, b, true)),
                    (Some(_), Some(b)) => {
                        tags.push(remove(dpt));
                        tags.push(place(dpt, b, false));
                    }
                    (Some(_), None) => tags.push(remove(dpt)),
                    (None, Some(b)) => tags.push(place(dpt, b, false)),
                    (None, None) => {}
                }
            }
            prev = cur;
            tags.push(RawTag { code: SHOW_FRAME, body: vec![] });
        }
        Ok(tags)
    }

    fn flatten(&mut self, e: &Element, parent: &Matrix, cx: Option<ColorXform>, out: &mut Vec<Placed>) -> Result<(), String> {
        let m = parent.concat(&e.matrix);
        let mut own = ColorXform::IDENTITY;
        own.mul[3] = e.alpha;
        if let Some((c, amt)) = e.tint {
            let amt = amt.clamp(0.0, 1.0);
            for (i, v) in [c.r, c.g, c.b].into_iter().enumerate() {
                own.mul[i] = 1.0 - amt;
                own.add[i] = v as f32 * amt;
            }
        }
        if let Some(f) = e.color_fx {
            own = f.after(&own);
        }
        let total = ColorXform::compose(cx, Some(own)).filter(|c| !c.is_identity());
        if !e.filters.is_empty() || e.blend != Default::default() {
            self.warn.push("filters and blend modes are not exported to SWF".into());
        }
        if e.three_d.is_some() || e.mesh.is_some() {
            self.warn.push("3D transforms and Asset Warp meshes are not exported to SWF".into());
        }
        let name = e.name.clone();
        match &e.kind {
            ElementKind::Shape(s) => {
                let key = format!("{:?}", s);
                let ch = match self.shape_cache.get(&key) {
                    Some(&c) => c,
                    None => {
                        let c = self.ids.fresh()?;
                        let t = self.shape_tag(c, s);
                        self.defs.push(t);
                        self.shape_cache.insert(key, c);
                        c
                    }
                };
                out.push(Placed { ch, m, cx: total, name });
            }
            ElementKind::Bitmap(r) => {
                let ch = match self.bitmap_shapes.get(&r.bitmap) {
                    Some(&c) => c,
                    None => {
                        let Some(b) = self.doc.bitmap(r.bitmap) else {
                            self.warn.push(format!("bitmap {} is not in the library", r.bitmap));
                            return Ok(());
                        };
                        let c = self.ids.fresh()?;
                        let (w, h) = (b.width as f32, b.height as f32);
                        let mut s = Shape::new(ac_core::geom::Path::rect(0.0, 0.0, w, h), None, None);
                        s.bitmap_fill = Some(BitmapFill { bitmap: r.bitmap, matrix: Matrix::IDENTITY });
                        let t = self.shape_tag(c, &s);
                        self.defs.push(t);
                        self.bitmap_shapes.insert(r.bitmap, c);
                        c
                    }
                };
                out.push(Placed { ch, m, cx: total, name });
            }
            ElementKind::Instance(i) => {
                let ch = self.symbol(i.symbol)?;
                if !matches!(i.loop_mode, LoopMode::Loop) || i.first_frame != 0 {
                    self.warn.push("graphic symbol playback options (play once, first frame...) are not exported: instances loop".into());
                }
                out.push(Placed { ch, m, cx: total, name });
            }
            ElementKind::Group(kids) => {
                for k in kids {
                    self.flatten(k, &m, total, out)?;
                }
            }
            ElementKind::Text(_) => self.warn.push("text is not exported to SWF yet".into()),
            ElementKind::Canvas(_) => self.warn.push("Merge Drawing canvases are not exported to SWF yet (use Object Drawing shapes)".into()),
            ElementKind::Particles(_) => self.warn.push("particle emitters are not exported to SWF".into()),
        }
        Ok(())
    }

    // ------------------------------------------------------------- shapes

    fn shape_tag(&mut self, ch: u16, s: &Shape) -> RawTag {
        let mut bitmap_fill = None;
        if let Some(bf) = &s.bitmap_fill {
            match self.bitmap_ids.get(&bf.bitmap) {
                Some(&id) => bitmap_fill = Some((id, bf.matrix)),
                None => self.warn.push(format!("shape fills with missing bitmap {}", bf.bitmap)),
            }
        }
        if s.gradient.is_some() {
            self.warn.push("gradient fills are exported as their fallback solid colour".into());
        }
        let fill = if bitmap_fill.is_some() { Fill::Bitmap(bitmap_fill.unwrap().0, bitmap_fill.unwrap().1) } else if let Some(c) = s.fill { Fill::Solid(c) } else { Fill::None };
        let stroke = s.stroke.filter(|st| st.width > 0.0);
        let (body, _) = shape_body(&s.path.segs, fill, stroke, ch);
        RawTag { code: DEFINE_SHAPE3, body }
    }
}

fn decode_rgba(png: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let img = image::load_from_memory(png).ok()?.to_rgba8();
    Some((img.width(), img.height(), img.into_raw()))
}

fn lone_shape(tl: &Timeline) -> Option<&Shape> {
    let layers: Vec<_> = tl.layers.iter().filter(|l| l.kind == LayerKind::Normal).collect();
    if tl.layers.len() != 1 || layers.len() != 1 || tl.frame_count() != 1 {
        return None;
    }
    let l = layers[0];
    if l.keyframes.len() != 1 || !l.visible {
        return None;
    }
    let kf = l.keyframes.values().next()?;
    if kf.elements.len() != 1 || kf.label.is_some() {
        return None;
    }
    let e = &kf.elements[0];
    match &e.kind {
        ElementKind::Shape(s) if e.matrix == Matrix::IDENTITY && e.alpha == 1.0 && e.tint.is_none() && e.color_fx.is_none() && e.filters.is_empty() && e.three_d.is_none() && e.mesh.is_none() && s.gradient.is_none() => Some(s),
        _ => None,
    }
}

fn remove(depth: u16) -> RawTag {
    RawTag { code: REMOVE_OBJECT2, body: depth.to_le_bytes().to_vec() }
}

fn place(depth: u16, p: &Placed, mv: bool) -> RawTag {
    let mut w = BitW::new();
    let has_name = !p.name.is_empty() && !mv;
    let flags = (has_name as u8) << 5 | (p.cx.is_some() as u8) << 3 | 1 << 2 | ((!mv) as u8) << 1 | mv as u8;
    w.u8(flags);
    w.u16(depth);
    if !mv {
        w.u16(p.ch);
    }
    write_matrix(&mut w, &p.m, 1.0);
    if let Some(c) = &p.cx {
        write_cxform(&mut w, c);
    }
    if has_name {
        w.bytes(p.name.as_bytes());
        w.u8(0);
    }
    RawTag { code: PLACE_OBJECT2, body: w.finish() }
}

/// MATRIX; `unit` multiplies every entry (20 for bitmap-fill matrices, which
/// map pixels to twips).
pub fn write_matrix(w: &mut BitW, m: &Matrix, unit: f32) {
    let fx = |v: f32| (v * 65536.0).round() as i32;
    w.align();
    if (m.a, m.d) != (1.0, 1.0) || unit != 1.0 {
        let (a, d) = (fx(m.a * unit), fx(m.d * unit));
        let n = sb_bits(&[a, d]);
        w.flag(true);
        w.ub(5, n);
        w.sb(n, a);
        w.sb(n, d);
    } else {
        w.flag(false);
    }
    if m.b != 0.0 || m.c != 0.0 {
        let (b, c) = (fx(m.b * unit), fx(m.c * unit));
        let n = sb_bits(&[b, c]);
        w.flag(true);
        w.ub(5, n);
        w.sb(n, b);
        w.sb(n, c);
    } else {
        w.flag(false);
    }
    let (tx, ty) = (twips(m.tx * if unit == 1.0 { 1.0 } else { unit / 20.0 }), twips(m.ty * if unit == 1.0 { 1.0 } else { unit / 20.0 }));
    let n = sb_bits(&[tx, ty]);
    w.ub(5, n);
    w.sb(n, tx);
    w.sb(n, ty);
    w.align();
}

fn write_cxform(w: &mut BitW, c: &ColorXform) {
    let mul: Vec<i32> = c.mul.iter().map(|v| (v * 256.0).round() as i32).collect();
    let add: Vec<i32> = c.add.iter().map(|v| v.round().clamp(-255.0, 255.0) as i32).collect();
    let has_mul = mul.iter().any(|&v| v != 256);
    let has_add = add.iter().any(|&v| v != 0);
    w.align();
    w.flag(has_add);
    w.flag(has_mul);
    let mut all = vec![];
    if has_mul {
        all.extend_from_slice(&mul);
    }
    if has_add {
        all.extend_from_slice(&add);
    }
    let n = sb_bits(&all);
    w.ub(4, n);
    if has_mul {
        for v in &mul {
            w.sb(n, *v);
        }
    }
    if has_add {
        for v in &add {
            w.sb(n, *v);
        }
    }
    w.align();
}

// ------------------------------------------------------------------ shapes

#[derive(Clone, Copy)]
enum Fill {
    None,
    Solid(Color),
    Bitmap(u16, Matrix),
}

type Tw = (i32, i32);

enum Edge {
    Line(Tw),
    Quad(Tw, Tw),
}

fn tw(p: Pt) -> Tw {
    (twips(p.x), twips(p.y))
}

/// Cubic as quadratics (4 pieces; control from the pieces' end tangents).
pub fn cubic_to_quads(p0: Pt, c1: Pt, c2: Pt, p3: Pt) -> Vec<(Pt, Pt)> {
    let at = |t: f32| {
        let u = 1.0 - t;
        let f = |a: f32, b: f32, c: f32, d: f32| u * u * u * a + 3.0 * u * u * t * b + 3.0 * u * t * t * c + t * t * t * d;
        Pt { x: f(p0.x, c1.x, c2.x, p3.x), y: f(p0.y, c1.y, c2.y, p3.y) }
    };
    let deriv = |t: f32| {
        let u = 1.0 - t;
        let f = |a: f32, b: f32, c: f32, d: f32| 3.0 * (u * u * (b - a) + 2.0 * u * t * (c - b) + t * t * (d - c));
        Pt { x: f(p0.x, c1.x, c2.x, p3.x), y: f(p0.y, c1.y, c2.y, p3.y) }
    };
    const N: usize = 4;
    let mut out = Vec::with_capacity(N);
    for i in 0..N {
        let (t0, t1) = (i as f32 / N as f32, (i + 1) as f32 / N as f32);
        let (a, b) = (at(t0), at(t1));
        let (da, db) = (deriv(t0), deriv(t1));
        // control: intersection of the end tangents; the midpoint of the chord's
        // cubic-derived estimate when they are parallel
        let det = da.x * db.y - da.y * db.x;
        let ctrl = if det.abs() > 1e-9 {
            let s = ((b.x - a.x) * db.y - (b.y - a.y) * db.x) / det;
            Pt { x: a.x + da.x * s, y: a.y + da.y * s }
        } else {
            Pt { x: (a.x + b.x) / 2.0, y: (a.y + b.y) / 2.0 }
        };
        out.push((ctrl, b));
    }
    out
}

/// One contour: start point and edges, absolute twips.
struct Contour {
    start: Tw,
    edges: Vec<Edge>,
    closed: bool,
}

fn contours(segs: &[Seg]) -> Vec<Contour> {
    let mut out: Vec<Contour> = Vec::new();
    let mut cur: Option<Contour> = None;
    let mut pos = Pt { x: 0.0, y: 0.0 };
    let mut start = pos;
    for s in segs {
        match *s {
            Seg::M(p) => {
                if let Some(c) = cur.take() {
                    out.push(c);
                }
                cur = Some(Contour { start: tw(p), edges: vec![], closed: false });
                pos = p;
                start = p;
            }
            Seg::L(p) => {
                cur.get_or_insert_with(|| Contour { start: tw(pos), edges: vec![], closed: false }).edges.push(Edge::Line(tw(p)));
                pos = p;
            }
            Seg::Q(c, p) => {
                cur.get_or_insert_with(|| Contour { start: tw(pos), edges: vec![], closed: false }).edges.push(Edge::Quad(tw(c), tw(p)));
                pos = p;
            }
            Seg::C(c1, c2, p) => {
                let c = cur.get_or_insert_with(|| Contour { start: tw(pos), edges: vec![], closed: false });
                for (q, e) in cubic_to_quads(pos, c1, c2, p) {
                    c.edges.push(Edge::Quad(tw(q), tw(e)));
                }
                pos = p;
            }
            Seg::Z => {
                if let Some(mut c) = cur.take() {
                    c.closed = true;
                    out.push(c);
                }
                pos = start;
            }
        }
    }
    if let Some(c) = cur {
        out.push(c);
    }
    out.retain(|c| !c.edges.is_empty());
    out
}

fn area(c: &Contour) -> f64 {
    let mut p = c.start;
    let mut a = 0.0f64;
    let mut add = |q: Tw, p: &mut Tw| {
        a += p.0 as f64 * q.1 as f64 - q.0 as f64 * p.1 as f64;
        *p = q;
    };
    for e in &c.edges {
        match e {
            Edge::Line(q) => add(*q, &mut p),
            Edge::Quad(k, q) => {
                add(*k, &mut p);
                add(*q, &mut p);
            }
        }
    }
    add(c.start, &mut p);
    a / 2.0
}

fn reversed(c: &Contour) -> Contour {
    // points: start, then each edge's (control?) end
    let mut pts: Vec<(Option<Tw>, Tw)> = Vec::new(); // (control into this point, point)
    pts.push((None, c.start));
    for e in &c.edges {
        match e {
            Edge::Line(q) => pts.push((None, *q)),
            Edge::Quad(k, q) => pts.push((Some(*k), *q)),
        }
    }
    let last = pts.last().unwrap().1;
    let mut edges = Vec::new();
    for i in (1..pts.len()).rev() {
        let target = pts[i - 1].1;
        edges.push(match pts[i].0 {
            Some(k) => Edge::Quad(k, target),
            None => Edge::Line(target),
        });
    }
    Contour { start: last, edges, closed: c.closed }
}

fn push_line(edges: &mut Vec<Edge>, from: Tw, to: Tw) {
    // keep the deltas within what a 17-bit edge field holds
    if (to.0 - from.0).abs() > 30000 || (to.1 - from.1).abs() > 30000 {
        let mid = ((from.0 + to.0) / 2, (from.1 + to.1) / 2);
        push_line(edges, from, mid);
        push_line(edges, mid, to);
    } else {
        edges.push(Edge::Line(to));
    }
}

fn shape_body(segs: &[Seg], fill: Fill, stroke: Option<ac_core::model::Stroke>, ch: u16) -> (Vec<u8>, [i32; 4]) {
    let mut cs = contours(segs);
    // outer contours should run so that the fill (on the right) is inside
    if !matches!(fill, Fill::None) {
        let biggest = cs.iter().map(area).fold(0.0f64, |m, a| if a.abs() > m.abs() { a } else { m });
        if biggest < 0.0 {
            cs = cs.iter().map(reversed).collect();
        }
        // fills close their contours
        for c in &mut cs {
            c.closed = true;
        }
    }
    for c in &mut cs {
        if c.closed {
            let end = match c.edges.last() {
                Some(Edge::Line(p)) | Some(Edge::Quad(_, p)) => *p,
                None => c.start,
            };
            if end != c.start {
                let s = c.start;
                push_line(&mut c.edges, end, s);
            }
        }
    }
    // split long edges
    for c in &mut cs {
        let mut p = c.start;
        let mut out = Vec::new();
        for e in std::mem::take(&mut c.edges) {
            match e {
                Edge::Line(q) => {
                    push_line(&mut out, p, q);
                    p = q;
                }
                Edge::Quad(k, q) => {
                    split_quad(p, k, q, &mut out, 0);
                    p = q;
                }
            }
        }
        c.edges = out;
    }
    // bounds
    let (mut x0, mut x1, mut y0, mut y1) = (i32::MAX, i32::MIN, i32::MAX, i32::MIN);
    let mut grow = |p: Tw| {
        x0 = x0.min(p.0);
        x1 = x1.max(p.0);
        y0 = y0.min(p.1);
        y1 = y1.max(p.1);
    };
    for c in &cs {
        grow(c.start);
        for e in &c.edges {
            match e {
                Edge::Line(q) => grow(*q),
                Edge::Quad(k, q) => {
                    grow(*k);
                    grow(*q);
                }
            }
        }
    }
    if x0 > x1 {
        (x0, x1, y0, y1) = (0, 0, 0, 0);
    }
    let sw = stroke.map(|s| twips(s.width).max(1)).unwrap_or(0);
    let half = sw / 2 + 1;
    let bounds = [x0 - half, x1 + half, y0 - half, y1 + half];

    let mut w = BitW::new();
    w.u16(ch);
    write_rect(&mut w, bounds);
    // fill styles
    match fill {
        Fill::None => w.u8(0),
        Fill::Solid(c) => {
            w.u8(1);
            w.u8(0x00);
            w.bytes(&[c.r, c.g, c.b, c.a]);
        }
        Fill::Bitmap(id, m) => {
            w.u8(1);
            w.u8(0x40);
            w.u16(id);
            write_matrix(&mut w, &m, 20.0);
        }
    }
    // line styles
    match stroke {
        Some(s) => {
            w.u8(1);
            w.u16(sw as u16);
            w.bytes(&[s.color.r, s.color.g, s.color.b, s.color.a]);
        }
        None => w.u8(0),
    }
    let has_fill = !matches!(fill, Fill::None);
    let has_line = stroke.is_some();
    w.align();
    w.ub(4, has_fill as u32); // NumFillBits: 1 bit holds style 1
    w.ub(4, has_line as u32);
    let (fbits, lbits) = (has_fill as u32, has_line as u32);
    for c in &cs {
        // StyleChange with MoveTo and styles
        w.flag(false);
        w.flag(false); // new styles
        w.flag(has_line); // line style
        w.flag(has_fill); // fill style 1
        w.flag(has_fill); // fill style 0 (reset)
        w.flag(true); // move
        let n = sb_bits(&[c.start.0, c.start.1]);
        w.ub(5, n);
        w.sb(n, c.start.0);
        w.sb(n, c.start.1);
        if has_fill {
            w.ub(fbits, 0); // fill0 none
            w.ub(fbits, 1); // fill1 = style 1
        }
        if has_line {
            w.ub(lbits, 1);
        }
        let mut p = c.start;
        for e in &c.edges {
            match e {
                Edge::Line(q) => {
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
                    p = *q;
                }
                Edge::Quad(k, q) => {
                    let (cx, cy, ax, ay) = (k.0 - p.0, k.1 - p.1, q.0 - k.0, q.1 - k.1);
                    let n = sb_bits(&[cx, cy, ax, ay]).max(2);
                    w.flag(true);
                    w.flag(false);
                    w.ub(4, n - 2);
                    w.sb(n, cx);
                    w.sb(n, cy);
                    w.sb(n, ax);
                    w.sb(n, ay);
                    p = *q;
                }
            }
        }
    }
    // EndShapeRecord
    w.ub(6, 0);
    (w.finish(), bounds)
}

fn split_quad(p: Tw, k: Tw, q: Tw, out: &mut Vec<Edge>, depth: u32) {
    let big = [(k.0 - p.0), (k.1 - p.1), (q.0 - k.0), (q.1 - k.1)].iter().any(|v| v.abs() > 30000);
    if big && depth < 8 {
        // de Casteljau at 0.5, in integer twips
        let m1 = ((p.0 + k.0) / 2, (p.1 + k.1) / 2);
        let m2 = ((k.0 + q.0) / 2, (k.1 + q.1) / 2);
        let mid = ((m1.0 + m2.0) / 2, (m1.1 + m2.1) / 2);
        split_quad(p, m1, mid, out, depth + 1);
        split_quad(mid, m2, q, out, depth + 1);
    } else {
        out.push(Edge::Quad(k, q));
    }
}
