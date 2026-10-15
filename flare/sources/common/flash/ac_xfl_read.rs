//! XFL → Document.

use crate::edges::{self, Piece};
use crate::Package;
use ac_core::bitmap::{BitmapAsset, BitmapId};
use ac_core::fx::{BevelType, BlendMode, Filter};
use ac_core::geom::{pt, Matrix, Path, Pt};
use ac_core::merge::{MergeShape, StrokeEdge};
use ac_core::model::*;
use ac_core::motion::{CurvePt, Ease};
use roxmltree::Node;
use std::collections::HashMap;

pub(crate) fn el(name: &str, matrix: Matrix, kind: ElementKind) -> Element {
    Element { name: name.into(), matrix, alpha: 1.0, tint: None, kind, three_d: None, filters: vec![], blend: BlendMode::Normal, color_fx: None, mesh: None }
}

struct Reader<'a> {
    pkg: &'a Package,
    warn: Vec<String>,
    symbols: HashMap<String, SymbolId>,
    bitmaps: HashMap<String, BitmapId>,
    sounds: HashMap<String, ac_core::sound::SoundId>,
    doc: Document,
    /// Art brushes from PaintBrushDefinitions.xml, by index.
    brushes: HashMap<u32, crate::brush::ArtBrush>,
    /// Art-brush strokes of raw shapes, waiting to go above the frame's canvas.
    extra: Vec<Element>,
}

/// A brush stroke's artwork as a filled shape.
fn art_shape(b: &crate::brush::ArtBrush, line: &[Pt], stroke: Stroke) -> Option<Shape> {
    let polys = crate::brush::along(b, line, stroke.width);
    let mut segs = vec![];
    for p in polys.iter().filter(|p| p.len() >= 3) {
        segs.push(ac_core::Seg::M(p[0]));
        segs.extend(p[1..].iter().map(|q| ac_core::Seg::L(*q)));
        segs.push(ac_core::Seg::Z);
    }
    (!segs.is_empty()).then(|| Shape::new(ac_core::Path { segs }, Some(stroke.color), None))
}

fn kids<'a, 'i>(n: Node<'a, 'i>, name: &'static str) -> impl Iterator<Item = Node<'a, 'i>> {
    n.children().filter(move |c| c.is_element() && c.tag_name().name() == name)
}
fn child<'a, 'i>(n: Node<'a, 'i>, name: &str) -> Option<Node<'a, 'i>> {
    n.children().find(|c| c.is_element() && c.tag_name().name() == name)
}
fn elems<'a, 'i>(n: Node<'a, 'i>) -> impl Iterator<Item = Node<'a, 'i>> {
    n.children().filter(|c| c.is_element())
}
fn f(n: Node, a: &str, d: f32) -> f32 {
    n.attribute(a).and_then(|v| v.trim().parse().ok()).unwrap_or(d)
}
fn flag(n: Node, a: &str) -> bool {
    n.attribute(a) == Some("true")
}
fn color(n: Node, a: &str, alpha_attr: &str, d: Color) -> Color {
    let mut c = n.attribute(a).and_then(Color::from_hex).unwrap_or(d);
    if let Some(al) = n.attribute(alpha_attr).and_then(|v| v.parse::<f32>().ok()) {
        c.a = (al.clamp(0.0, 1.0) * 255.0).round() as u8;
    }
    c
}
fn matrix_of(n: Node) -> Matrix {
    match child(n, "matrix").and_then(|m| child(m, "Matrix")) {
        Some(m) => Matrix { a: f(m, "a", 1.0), b: f(m, "b", 0.0), c: f(m, "c", 0.0), d: f(m, "d", 1.0), tx: f(m, "tx", 0.0), ty: f(m, "ty", 0.0) },
        None => Matrix::IDENTITY,
    }
}

/// Fill style of a shape.
#[derive(Clone, Debug)]
enum Fill {
    Solid(Color),
    Gradient(Gradient),
    Bitmap(BitmapFill),
}

/// Gradient space: the format's gradient square is ±16384 twips = ±819.2 px;
/// ours is ±1.
const GRAD: f32 = 819.2;

impl Reader<'_> {
    fn xml(&self, path: &str) -> Result<String, String> {
        let b = self.pkg.get(path).ok_or_else(|| format!("missing {path}"))?;
        let s = String::from_utf8_lossy(b);
        Ok(s.trim_start_matches('\u{feff}').to_string())
    }

    fn fill_of(&mut self, n: Node) -> Option<Fill> {
        let g = elems(n).next()?;
        Some(match g.tag_name().name() {
            "SolidColor" => Fill::Solid(color(g, "color", "alpha", Color::rgb(0, 0, 0))),
            k @ ("LinearGradient" | "RadialGradient") => {
                let m = matrix_of(g).concat(&Matrix::scale(GRAD, GRAD));
                let stops = kids(g, "GradientEntry").map(|e| (f(e, "ratio", 0.0).clamp(0.0, 1.0), color(e, "color", "alpha", Color::rgb(0, 0, 0)))).collect();
                Fill::Gradient(Gradient {
                    kind: if k == "LinearGradient" { GradientKind::Linear } else { GradientKind::Radial },
                    stops,
                    matrix: m,
                    spread: match g.attribute("spreadMethod") {
                        Some("reflect") => Spread::Reflect,
                        Some("repeat") => Spread::Repeat,
                        _ => Spread::Pad,
                    },
                    focal: f(g, "focalPointRatio", 0.0).clamp(-1.0, 1.0),
                })
            }
            "BitmapFill" => {
                let name = g.attribute("bitmapPath").unwrap_or("");
                match self.bitmaps.get(name) {
                    Some(&bitmap) => {
                        let m = matrix_of(g);
                        let s = 1.0 / 20.0;
                        Fill::Bitmap(BitmapFill { bitmap, matrix: Matrix { a: m.a * s, b: m.b * s, c: m.c * s, d: m.d * s, ..m } })
                    }
                    None => {
                        self.warn.push(format!("bitmap fill '{name}' not found: filled grey"));
                        Fill::Solid(Color::rgb(128, 128, 128))
                    }
                }
            }
            other => {
                self.warn.push(format!("fill type {other} not supported: filled grey"));
                Fill::Solid(Color::rgb(128, 128, 128))
            }
        })
    }

    fn stroke_of(&mut self, n: Node) -> (Stroke, StrokeStyle, Option<u32>) {
        let s = elems(n).next();
        let Some(s) = s else { return (Stroke { color: Color::rgb(0, 0, 0), width: 1.0 }, StrokeStyle::default(), None) };
        let mut brush = None;
        let col = child(s, "fill").and_then(|fl| self.fill_of(fl)).map(|fl| match fl {
            Fill::Solid(c) => c,
            Fill::Gradient(g) => g.stops.first().map(|s| s.1).unwrap_or(Color::rgb(0, 0, 0)),
            Fill::Bitmap(_) => Color::rgb(128, 128, 128),
        });
        let mut st = StrokeStyle {
            cap: match s.attribute("caps") {
                Some("none") => Cap::None,
                Some("square") => Cap::Square,
                _ => Cap::Round,
            },
            join: match s.attribute("joints") {
                Some("miter") => Join::Miter,
                Some("bevel") => Join::Bevel,
                _ => Join::Round,
            },
            ..Default::default()
        };
        match s.tag_name().name() {
            "SolidStroke" => {}
            "DashedStroke" => {
                st.dash = Dash::Dashed;
                st.dash_len = f(s, "dash1", 6.0);
                st.gap = f(s, "dash2", 6.0);
            }
            "DottedStroke" => {
                st.dash = Dash::Dotted;
                st.gap = f(s, "dotSpace", 3.0).max(0.5);
            }
            "RaggedStroke" => st.dash = Dash::Ragged,
            "StippleStroke" => st.dash = Dash::Stipple,
            "HatchedStroke" => st.dash = Dash::Hatched,
            "PaintBrushStroke" => {
                let i = f(s, "paintBrushDefinition", 0.0) as u32;
                if self.brushes.contains_key(&i) {
                    brush = Some(i);
                } else {
                    self.warn.push(format!("paint brush {i} not found: drawn as a solid line"));
                }
            }
            other => self.warn.push(format!("{other} drawn as a solid line")),
        }
        let width = if s.attribute("solidStyle") == Some("hairline") { 1.0 } else { f(s, "weight", 1.0) };
        (Stroke { color: col.unwrap_or(Color::rgb(0, 0, 0)), width }, st, brush)
    }

    /// The fill and stroke tables plus the edge records of a DOMShape-like node.
    #[allow(clippy::type_complexity)]
    fn shape_parts(&mut self, n: Node) -> (HashMap<u32, Fill>, HashMap<u32, (Stroke, StrokeStyle, Option<u32>)>, Vec<(u32, u32, u32, Vec<Piece>)>) {
        let mut fills = HashMap::new();
        if let Some(fs) = child(n, "fills") {
            for s in kids(fs, "FillStyle") {
                if let Some(fl) = self.fill_of(s) {
                    fills.insert(f(s, "index", 0.0) as u32, fl);
                }
            }
        }
        let mut strokes = HashMap::new();
        if let Some(ss) = child(n, "strokes") {
            for s in kids(ss, "StrokeStyle") {
                strokes.insert(f(s, "index", 0.0) as u32, self.stroke_of(s));
            }
        }
        let mut recs = vec![];
        if let Some(es) = child(n, "edges") {
            for e in kids(es, "Edge") {
                let Some(text) = e.attribute("edges") else { continue };
                match edges::parse(text) {
                    Ok(p) => recs.push((f(e, "fillStyle0", 0.0) as u32, f(e, "fillStyle1", 0.0) as u32, f(e, "strokeStyle", 0.0) as u32, p)),
                    Err(err) => self.warn.push(format!("skipped an edge: {err}")),
                }
            }
        }
        (fills, strokes, recs)
    }

    /// Oriented pieces bounding fill style `k` (fill on one consistent side).
    fn fill_pieces(recs: &[(u32, u32, u32, Vec<Piece>)], k: u32) -> Vec<Piece> {
        let mut v = vec![];
        for (f0, f1, _, ps) in recs {
            if *f0 == *f1 {
                continue; // same fill both sides: an interior edge
            }
            if *f1 == k {
                v.extend(ps.iter().copied());
            }
            if *f0 == k {
                v.extend(ps.iter().rev().map(|p| p.reversed()));
            }
        }
        v
    }

    /// A raw shape (merge drawing) → added into `canvas`.
    fn raw_shape(&mut self, n: Node, canvas: &mut MergeShape) {
        let m = matrix_of(n);
        let (fills, strokes, mut recs) = self.shape_parts(n);
        if m != Matrix::IDENTITY {
            for r in &mut recs {
                for p in &mut r.3 {
                    *p = Piece { a: m.apply(p.a), ctrl: p.ctrl.map(|c| m.apply(c)), b: m.apply(p.b) };
                }
            }
        }
        // Flash's raw shape is a planar map: its fill regions don't overlap,
        // so they go in together (one cut of what's below, one merge)
        let mut keys: Vec<_> = fills.keys().copied().collect();
        keys.sort();
        let mut regions = vec![];
        for k in keys {
            let pcs = Self::fill_pieces(&recs, k);
            if pcs.is_empty() {
                continue;
            }
            let chains = edges::chain(&pcs);
            let polys: Vec<Vec<Pt>> = chains.iter().map(|(c, closed)| edges::chain_points(c, *closed)).filter(|p| p.len() >= 3).collect();
            let paint = match &fills[&k] {
                Fill::Bitmap(bf) => ac_core::merge::FillRegion { fill: None, gradient: None, bitmap: Some(BitmapFill { bitmap: bf.bitmap, matrix: m.concat(&bf.matrix) }), contours: vec![] },
                Fill::Solid(c) => ac_core::merge::FillRegion { fill: Some(*c), gradient: None, bitmap: None, contours: vec![] },
                Fill::Gradient(g) => ac_core::merge::FillRegion { fill: None, gradient: Some(Gradient { matrix: m.concat(&g.matrix), ..g.clone() }), bitmap: None, contours: vec![] },
            };
            regions.push((polys, paint));
        }
        canvas.add_planar(&regions);
        let mut sk: Vec<_> = strokes.keys().copied().collect();
        sk.sort();
        // lines likewise: Flash's edges already meet the fills and each other
        // at their crossings, so they go in as they are
        for k in sk {
            let pcs: Vec<Piece> = recs.iter().filter(|r| r.2 == k).flat_map(|r| r.3.iter().copied()).collect();
            let (stroke, _, brush) = strokes[&k];
            for (c, closed) in edges::chain(&pcs) {
                let mut pts = edges::chain_points(&c, closed);
                if pts.len() < 2 {
                    continue;
                }
                if let Some(b) = brush.and_then(|i| self.brushes.get(&i)) {
                    if closed {
                        pts.push(pts[0]);
                    }
                    if let Some(sh) = art_shape(b, &pts, stroke) {
                        self.extra.push(el("", Matrix::IDENTITY, ElementKind::Shape(sh)));
                    }
                    continue;
                }
                canvas.strokes.push(StrokeEdge::new(pts, closed, stroke));
            }
        }
    }

    /// A drawing object → one Shape element (or a Group of them).
    fn drawing_object(&mut self, n: Node) -> Option<Element> {
        let m = matrix_of(n);
        let (fills, strokes, recs) = self.shape_parts(n);
        let mut shapes = vec![];
        let mut fk: Vec<_> = fills.keys().copied().collect();
        fk.sort();
        let mut sk: Vec<_> = strokes.keys().copied().collect();
        sk.sort();
        // the common case: one fill outlined by one stroke → one Shape
        if fk.len() == 1 && sk.len() == 1 && strokes[&sk[0]].2.is_none() && recs.iter().all(|r| (r.2 == sk[0]) == (r.0 == fk[0] || r.1 == fk[0])) {
            let pcs = Self::fill_pieces(&recs, fk[0]);
            let mut sh = self.fill_shape(&fills[&fk[0]], &edges::chain(&pcs));
            let (s, st, _) = strokes[&sk[0]];
            sh.stroke = Some(s);
            sh.stroke_style = st;
            shapes.push(sh);
        } else {
            for k in &fk {
                let pcs = Self::fill_pieces(&recs, *k);
                if !pcs.is_empty() {
                    shapes.push(self.fill_shape(&fills[k], &edges::chain(&pcs)));
                }
            }
            for k in &sk {
                let pcs: Vec<Piece> = recs.iter().filter(|r| r.2 == *k).flat_map(|r| r.3.iter().copied()).collect();
                if pcs.is_empty() {
                    continue;
                }
                let (s, st, brush) = strokes[k];
                if let Some(b) = brush.and_then(|i| self.brushes.get(&i)) {
                    for (c, closed) in edges::chain(&pcs) {
                        let mut pts = edges::chain_points(&c, closed);
                        if closed && !pts.is_empty() {
                            pts.push(pts[0]);
                        }
                        shapes.extend(art_shape(b, &pts, s));
                    }
                    continue;
                }
                let mut sh = Shape::new(edges::chains_path(&edges::chain(&pcs)), None, Some(s));
                sh.stroke_style = st;
                shapes.push(sh);
            }
        }
        match shapes.len() {
            0 => None,
            1 => Some(el("", m, ElementKind::Shape(shapes.pop().unwrap()))),
            _ => Some(el("", m, ElementKind::Group(shapes.into_iter().map(|s| el("", Matrix::IDENTITY, ElementKind::Shape(s))).collect()))),
        }
    }

    fn fill_shape(&self, fl: &Fill, chains: &[(Vec<Piece>, bool)]) -> Shape {
        let path = edges::chains_path(chains);
        let mut sh = Shape::new(path, None, None);
        match fl {
            Fill::Solid(c) => sh.fill = Some(*c),
            Fill::Gradient(g) => sh.gradient = Some(g.clone()),
            Fill::Bitmap(b) => sh.bitmap_fill = Some(*b),
        }
        sh
    }

    fn primitive(&mut self, n: Node, oval: bool) -> Element {
        let (x, y, w, h) = (f(n, "x", 0.0), f(n, "y", 0.0), f(n, "objectWidth", 0.0), f(n, "objectHeight", 0.0));
        let prim = if oval {
            Primitive::Oval { cx: x + w / 2.0, cy: y + h / 2.0, rx: w / 2.0, ry: h / 2.0, start: f(n, "startAngle", 0.0), end: f(n, "endAngle", 0.0), inner: f(n, "innerRadius", 0.0), close: n.attribute("closePath") != Some("false") }
        } else {
            let r = f(n, "topLeftRadius", 0.0);
            let rad = if flag(n, "lockFlag") || n.attribute("topRightRadius").is_none() { [r; 4] } else { [r, f(n, "topRightRadius", 0.0), f(n, "bottomRightRadius", 0.0), f(n, "bottomLeftRadius", 0.0)] };
            Primitive::Rect { x, y, w, h, radius: rad }
        };
        let mut sh = Shape::new(prim.path(), None, None);
        sh.primitive = Some(prim);
        if let Some(fl) = child(n, "fill").and_then(|fl| self.fill_of(fl)) {
            match fl {
                Fill::Solid(c) => sh.fill = Some(c),
                Fill::Gradient(g) => sh.gradient = Some(g),
                Fill::Bitmap(b) => sh.bitmap_fill = Some(b),
            }
        }
        if let Some(s) = child(n, "stroke") {
            let (s, st, brush) = self.stroke_of(s);
            if brush.is_some() {
                self.warn.push("paint brush on a rectangle/oval primitive drawn as a solid line".into());
            }
            sh.stroke = Some(s);
            sh.stroke_style = st;
        }
        el("", matrix_of(n), ElementKind::Shape(sh))
    }

    fn color_effect(&mut self, n: Node, e: &mut Element) {
        let Some(c) = child(n, "color").and_then(|c| child(c, "Color")) else { return };
        if let Some(b) = c.attribute("brightness").and_then(|v| v.parse::<f32>().ok()) {
            e.tint = ac_core::motion::brightness_tint(b, None);
            return;
        }
        if c.attribute("tintMultiplier").is_some() || c.attribute("tintColor").is_some() {
            e.tint = Some((color(c, "tintColor", "", Color::rgb(0, 0, 0)), f(c, "tintMultiplier", 0.0)));
        }
        let mul = [f(c, "redMultiplier", 1.0), f(c, "greenMultiplier", 1.0), f(c, "blueMultiplier", 1.0), f(c, "alphaMultiplier", 1.0)];
        let add = [f(c, "redOffset", 0.0), f(c, "greenOffset", 0.0), f(c, "blueOffset", 0.0), f(c, "alphaOffset", 0.0)];
        if e.tint.is_some() || (mul[..3] == [1.0; 3] && add == [0.0; 4]) {
            e.alpha = mul[3];
        } else {
            e.color_fx = Some(ColorXform { mul, add });
        }
    }

    fn filters(&mut self, n: Node) -> Vec<Filter> {
        let Some(fs) = child(n, "filters") else { return vec![] };
        let mut out = vec![];
        for x in elems(fs) {
            if x.attribute("isEnabled") == Some("false") {
                continue;
            }
            let (bx, by, q) = (f(x, "blurX", 5.0), f(x, "blurY", 5.0), f(x, "quality", 1.0) as u8);
            let strength = f(x, "strength", 1.0) * 100.0;
            out.push(match x.tag_name().name() {
                "DropShadowFilter" => Filter::DropShadow { blur_x: bx, blur_y: by, strength, quality: q, angle: f(x, "angle", 45.0), distance: f(x, "distance", 5.0), color: color(x, "color", "alpha", Color::rgb(0, 0, 0)), knockout: flag(x, "knockout"), inner: flag(x, "inner"), hide_object: flag(x, "hideObject") },
                "BlurFilter" => Filter::Blur { blur_x: bx, blur_y: by, quality: q },
                "GlowFilter" => Filter::Glow { blur_x: bx, blur_y: by, strength, quality: q, color: color(x, "color", "alpha", Color::rgb(255, 0, 0)), knockout: flag(x, "knockout"), inner: flag(x, "inner") },
                "BevelFilter" => Filter::Bevel {
                    blur_x: bx,
                    blur_y: by,
                    strength,
                    quality: q,
                    shadow: color(x, "shadowColor", "shadowAlpha", Color::rgb(0, 0, 0)),
                    highlight: color(x, "highlightColor", "highlightAlpha", Color::rgb(255, 255, 255)),
                    angle: f(x, "angle", 45.0),
                    distance: f(x, "distance", 5.0),
                    knockout: flag(x, "knockout"),
                    kind: match x.attribute("type") {
                        Some("outer") => BevelType::Outer,
                        Some("full") => BevelType::Full,
                        _ => BevelType::Inner,
                    },
                },
                "AdjustColorFilter" => Filter::AdjustColor { brightness: f(x, "brightness", 0.0), contrast: f(x, "contrast", 0.0), saturation: f(x, "saturation", 0.0), hue: f(x, "hue", 0.0) },
                tag @ ("GradientGlowFilter" | "GradientBevelFilter") => {
                    let stops: Vec<(f32, Color)> = kids(x, "GradientEntry").map(|e| (f(e, "ratio", 0.0).clamp(0.0, 1.0), color(e, "color", "alpha", Color::rgb(0, 0, 0)))).collect();
                    let glow = tag == "GradientGlowFilter";
                    let kind = match x.attribute("type") {
                        Some("outer") => BevelType::Outer,
                        Some("full") => BevelType::Full,
                        Some("inner") => BevelType::Inner,
                        _ if glow => BevelType::Outer,
                        _ => BevelType::Inner,
                    };
                    let (angle, distance, knockout) = (f(x, "angle", 45.0), f(x, "distance", 5.0), flag(x, "knockout"));
                    let d = Filter::default_of(if glow { "gradient glow" } else { "gradient bevel" }).unwrap();
                    let stops = if stops.len() >= 2 { stops } else if let Filter::GradientGlow { stops, .. } | Filter::GradientBevel { stops, .. } = d { stops } else { vec![] };
                    if glow {
                        Filter::GradientGlow { blur_x: bx, blur_y: by, strength, quality: q, angle, distance, knockout, kind, stops }
                    } else {
                        Filter::GradientBevel { blur_x: bx, blur_y: by, strength, quality: q, angle, distance, knockout, kind, stops }
                    }
                }
                other => {
                    self.warn.push(format!("{other} not supported"));
                    continue;
                }
            });
        }
        out
    }

    fn text(&mut self, n: Node) -> Option<Element> {
        let mut text = String::new();
        let mut attrs = None;
        for run in child(n, "textRuns").into_iter().flat_map(|r| kids(r, "DOMTextRun")) {
            if let Some(c) = child(run, "characters") {
                text.push_str(&c.text().unwrap_or("").replace("\r\n", "\n").replace('\r', "\n"));
            }
            if attrs.is_none() {
                attrs = child(run, "textAttrs").and_then(|a| child(a, "DOMTextAttrs"));
            }
        }
        if text.is_empty() {
            return None;
        }
        let (size, col, align, ls, face) = match attrs {
            Some(a) => (
                f(a, "size", 12.0),
                color(a, "fillColor", "alpha", Color::rgb(0, 0, 0)),
                match a.attribute("alignment") {
                    Some("center") => ac_core::text::Align::Center,
                    Some("right") => ac_core::text::Align::Right,
                    Some("justify") => ac_core::text::Align::Justify,
                    _ => ac_core::text::Align::Left,
                },
                f(a, "letterSpacing", 0.0),
                a.attribute("face").map(|s| s.to_string()),
            ),
            None => (12.0, Color::rgb(0, 0, 0), ac_core::text::Align::Left, 0.0, None),
        };
        use ac_core::text::{Baseline, Paragraph, TextField, TextKind};
        let kind = match n.tag_name().name() {
            "DOMDynamicText" => TextKind::Dynamic,
            "DOMInputText" => TextKind::Input,
            _ => TextKind::Static,
        };
        // a fixed-width box unless the field grows with its text
        let auto = flag(n, "autoExpand") || n.attribute("lineType").is_some_and(|l| l == "single line") || n.attribute("width").is_none();
        let para = match attrs {
            Some(a) => Paragraph {
                line_spacing: f(a, "lineSpacing", 0.0),
                indent: f(a, "indent", 0.0),
                margin_left: f(a, "leftMargin", 0.0),
                margin_right: f(a, "rightMargin", 0.0),
                width: (!auto && n.attribute("lineType") != Some("multiline no wrap")).then(|| f(n, "width", 0.0)).filter(|w| *w > 1.0),
                baseline: match a.attribute("characterPosition") {
                    Some("superscript") => Baseline::Superscript,
                    Some("subscript") => Baseline::Subscript,
                    _ => Baseline::Normal,
                },
                no_kern: a.attribute("autoKern") == Some("false"),
            },
            None => Paragraph::default(),
        };
        let field = TextField {
            kind,
            max_chars: f(n, "maxCharacters", 0.0) as u32,
            selectable: n.attribute("isSelectable").map(|v| v != "false").unwrap_or(kind != TextKind::Static),
            border: flag(n, "border"),
            password: n.attribute("lineType") == Some("password"),
            link: attrs.and_then(|a| a.attribute("url")).filter(|u| !u.is_empty()).map(|u| u.to_string()),
        };
        let m = matrix_of(n).concat(&Matrix::translate(f(n, "left", 0.0), f(n, "top", 0.0)));
        let mono = face.as_deref().is_some_and(|s| s.to_lowercase().contains("mono") || s.to_lowercase().contains("courier"));
        let (family, weight, italic) = match face.as_deref() {
            Some(f) => split_face(f),
            None => (None, 400, false),
        };
        let t = TextEl { text, size, color: col, font: if mono { ac_core::text::FontFace::Mono } else { ac_core::text::FontFace::Sans }, align, letter_spacing: ls, family, weight, italic, para, field };
        Some(el(n.attribute("name").unwrap_or(""), m, ElementKind::Text(t)))
    }

    /// The elements of a frame (or group members), bottom first.
    fn elements(&mut self, parent: Node, out: &mut Vec<Element>) {
        let mut canvas = MergeShape::default();
        let mark = self.extra.len();
        for n in elems(parent) {
            match n.tag_name().name() {
                "DOMShape" if !flag(n, "isDrawingObject") => self.raw_shape(n, &mut canvas),
                "DOMShape" => out.extend(self.drawing_object(n)),
                "DOMRectangleObject" => out.push(self.primitive(n, false)),
                "DOMOvalObject" => out.push(self.primitive(n, true)),
                "DOMSymbolInstance" | "DOMComponentInstance" => {
                    let lib = n.attribute("libraryItemName").unwrap_or("");
                    let Some(&symbol) = self.symbols.get(lib) else {
                        self.warn.push(format!("instance of missing symbol '{lib}' skipped"));
                        continue;
                    };
                    let loop_mode = match n.attribute("loop") {
                        Some("play once") => LoopMode::PlayOnce,
                        Some("play once reverse") => LoopMode::PlayReverseOnce,
                        Some("loop reverse") => LoopMode::LoopReverse,
                        Some("single frame") => LoopMode::SingleFrame,
                        _ => LoopMode::Loop,
                    };
                    let mut e = el(n.attribute("name").unwrap_or(""), matrix_of(n), ElementKind::Instance(Instance { symbol, loop_mode, first_frame: f(n, "firstFrame", 0.0) as u32 }));
                    self.color_effect(n, &mut e);
                    e.filters = self.filters(n);
                    if let Some(b) = n.attribute("blendMode") {
                        e.blend = BlendMode::from_name(b).unwrap_or_else(|| {
                            self.warn.push(format!("blend mode {b} not supported"));
                            BlendMode::Normal
                        });
                    }
                    out.push(e);
                }
                "DOMBitmapInstance" => {
                    let lib = n.attribute("libraryItemName").unwrap_or("");
                    match self.bitmaps.get(lib) {
                        Some(&b) => out.push(el(n.attribute("name").unwrap_or(""), matrix_of(n), ElementKind::Bitmap(BitmapRef { bitmap: b }))),
                        None => self.warn.push(format!("bitmap '{lib}' not found")),
                    }
                }
                "DOMStaticText" | "DOMDynamicText" | "DOMInputText" => out.extend(self.text(n)),
                "DOMGroup" => {
                    let mut ch = vec![];
                    if let Some(m) = child(n, "members") {
                        self.elements(m, &mut ch);
                    }
                    if !ch.is_empty() {
                        out.push(el("", Matrix::IDENTITY, ElementKind::Group(ch)));
                    }
                }
                other => self.warn.push(format!("{other} not supported, skipped")),
            }
        }
        // art-brush lines of the raw shape sit just above it
        let extra: Vec<Element> = self.extra.drain(mark..).collect();
        for (k, e) in extra.into_iter().enumerate() {
            out.insert(k, e);
        }
        if !canvas.is_empty() {
            out.insert(0, el("", Matrix::IDENTITY, ElementKind::Canvas(canvas)));
        }

    }

    fn custom_ease(n: Node) -> Option<Ease> {
        let ce = child(n, "tweens").and_then(|t| kids(t, "CustomEase").find(|c| matches!(c.attribute("target"), None | Some("all"))))?;
        Self::ease_of(ce)
    }

    /// Per-property eases (`CustomEase target="position"` etc.).
    fn prop_eases(n: Node) -> Option<ac_core::motion::PropEases> {
        let t = child(n, "tweens")?;
        let mut pe = ac_core::motion::PropEases::default();
        let mut any = false;
        for ce in kids(t, "CustomEase") {
            let Some(g) = ce.attribute("target").and_then(ac_core::motion::PropGroup::from_id) else { continue };
            if let Some(e) = Self::ease_of(ce) {
                *pe.get_mut(g) = Some(e);
                any = true;
            }
        }
        any.then_some(pe)
    }

    fn ease_of(ce: Node) -> Option<Ease> {
        let p: Vec<Pt> = kids(ce, "Point").map(|q| pt(f(q, "x", 0.0), f(q, "y", 0.0))).collect();
        // anchor, out-handle, in-handle, anchor, …
        if p.len() < 4 || !(p.len() - 1).is_multiple_of(3) {
            return None;
        }
        let mut pts: Vec<CurvePt> = vec![];
        let mut i = 0;
        while i < p.len() {
            let a = p[i];
            let h_in = if i > 0 { pt(p[i - 1].x - a.x, p[i - 1].y - a.y) } else { Pt::default() };
            let h_out = if i + 1 < p.len() { pt(p[i + 1].x - a.x, p[i + 1].y - a.y) } else { Pt::default() };
            pts.push(CurvePt { x: a.x, y: a.y, h_in, h_out });
            i += 3;
        }
        Some(Ease::Custom { points: pts })
    }

    fn layer(&mut self, n: Node) -> Layer {
        let mut l = Layer::new(n.attribute("name").unwrap_or("Layer"), 0);
        if let Some(c) = n.attribute("color").and_then(Color::from_hex) {
            l.color = c;
        }
        l.visible = n.attribute("visible") != Some("false");
        l.locked = flag(n, "locked");
        l.outline = flag(n, "useOutlineView") || flag(n, "outline");
        l.collapsed = n.attribute("open") == Some("false");
        l.kind = match n.attribute("layerType") {
            Some("folder") => LayerKind::Folder,
            Some("guide") => LayerKind::Guide,
            Some("mask") => LayerKind::Mask,
            Some("camera") => LayerKind::Camera,
            _ => LayerKind::Normal,
        };
        l.parent = n.attribute("parentLayerIndex").and_then(|v| v.parse().ok());
        l.keyframes.clear();
        l.end = 0;
        for fr in child(n, "frames").into_iter().flat_map(|fs| kids(fs, "DOMFrame")) {
            let index = f(fr, "index", 0.0) as u32;
            let dur = f(fr, "duration", 1.0).max(1.0) as u32;
            let mut els = vec![];
            if let Some(es) = child(fr, "elements") {
                self.elements(es, &mut els);
            }
            let mut kf = Keyframe::with(els);
            let ease = f(fr, "acceleration", 0.0);
            kf.tween = match fr.attribute("tweenType") {
                Some("motion") => {
                    let times = f(fr, "motionTweenRotateTimes", 0.0) as i32;
                    let turns = match fr.attribute("motionTweenRotate") {
                        Some("cw") | Some("clockwise") => times.max(1),
                        Some("ccw") | Some("counter-clockwise") => -times.max(1),
                        _ => 0,
                    };
                    Tween::Classic { ease, rotate_turns: turns }
                }
                Some("shape") => Tween::Shape { ease },
                Some("motion object") => match kf.elements.first().and_then(|el| motion_object(fr, el, self.doc.fps)) {
                    Some(m) => {
                        kf.motion = Some(Box::new(m));
                        self.warn.push(format!("motion tween on '{}' read from its motion XML (experimental: check the result)", l.name));
                        Tween::Motion { ease: 0.0 }
                    }
                    None => {
                        self.warn.push(format!("motion tween on '{}' imported without its property keyframes", l.name));
                        Tween::None
                    }
                },
                _ => Tween::None,
            };
            kf.orient_to_path = flag(fr, "motionTweenOrientToPath");
            if flag(fr, "hasCustomEase") || child(fr, "tweens").is_some() {
                kf.ease_curve = Self::custom_ease(fr);
                if matches!(kf.tween, Tween::Classic { .. }) {
                    kf.prop_eases = Self::prop_eases(fr).map(Box::new);
                }
            }
            kf.label = fr.attribute("name").filter(|s| !s.is_empty()).map(|s| s.to_string());
            kf.label_kind = match fr.attribute("labelType") {
                Some("comment") => LabelKind::Comment,
                Some("anchor") => LabelKind::Anchor,
                _ => LabelKind::Name,
            };
            if fr.attribute("motionTweenSync") == Some("true") {
                kf.sync = true;
            }
            if matches!(kf.tween, Tween::Classic { .. }) {
                // Animate writes motionTweenSnap="true" when Snap is on and omits it when off
                kf.snap = flag(fr, "motionTweenSnap");
            }
            if let Some(sc) = child(fr, "Actionscript").and_then(|a| child(a, "script")) {
                let code: String = sc.children().filter_map(|t| t.text()).collect();
                if !code.trim().is_empty() {
                    kf.script = Some(code);
                }
            }
            if let Some(sn) = fr.attribute("soundName") {
                match self.sounds.get(sn) {
                    Some(&id) => {
                        let mut fs = ac_core::sound::FrameSound::new(id);
                        fs.sync = fr.attribute("soundSync").and_then(ac_core::sound::SoundSync::from_name).unwrap_or_default();
                        fs.forever = fr.attribute("soundLoopMode") == Some("loop");
                        fs.repeat = f(fr, "soundLoop", 1.0).max(1.0) as u32;
                        kf.sound = Some(fs);
                    }
                    None => self.warn.push(format!("frame sound '{sn}' not in the library")),
                }
            }
            l.keyframes.insert(index, kf);
            l.end = l.end.max(index + dur);
        }
        if l.keyframes.is_empty() {
            l.keyframes.insert(0, Keyframe::blank());
        }
        l.end = l.end.max(1);
        l
    }

    fn timeline(&mut self, n: Node) -> Timeline {
        let mut tl = Timeline { name: n.attribute("name").unwrap_or("Scene").into(), layers: vec![] };
        for l in child(n, "layers").into_iter().flat_map(|ls| kids(ls, "DOMLayer")) {
            let layer = self.layer(l);
            tl.layers.push(layer);
        }
        let count = tl.layers.len();
        for l in &mut tl.layers {
            if l.parent.is_some_and(|p| p >= count) {
                l.parent = None;
            }
        }
        if tl.layers.is_empty() {
            tl.layers.push(Layer::new("Layer 1", 0));
        }
        tl
    }

    fn media(&mut self, root: Node) {
        let Some(media) = child(root, "media") else { return };
        for b in elems(media) {
            let name = b.attribute("name").unwrap_or("").to_string();
            match b.tag_name().name() {
                "DOMBitmapItem" => {
                    let mut tries = vec![];
                    if let Some(h) = b.attribute("bitmapDataHRef") {
                        tries.push(format!("bin/{h}"));
                    }
                    if let Some(h) = b.attribute("href") {
                        tries.push(format!("LIBRARY/{h}"));
                    }
                    tries.push(format!("LIBRARY/{name}"));
                    let decoded = tries.iter().filter_map(|p| self.pkg.get(p)).find_map(|bytes| crate::dat::decode(bytes).ok());
                    let id = self.doc.bitmaps.iter().map(|b| b.id).max().unwrap_or(0) + 1;
                    let asset = match decoded {
                        Some((w, h, rgba)) => BitmapAsset::from_rgba(id, &name, w, h, &rgba),
                        None => {
                            self.warn.push(format!("bitmap '{name}': data missing or unreadable, using a placeholder"));
                            let (w, h) = ((f(b, "frameRight", 400.0) / 20.0).max(1.0) as u32, (f(b, "frameBottom", 400.0) / 20.0).max(1.0) as u32);
                            let rgba: Vec<u8> = (0..w * h).flat_map(|i| if ((i % w) / 8 + (i / w) / 8) % 2 == 0 { [200, 200, 200, 255] } else { [255, 255, 255, 255] }).collect();
                            BitmapAsset::from_rgba(id, &name, w, h, &rgba)
                        }
                    };
                    if let Ok(mut a) = asset {
                        a.smoothing = b.attribute("allowSmoothing") != Some("false");
                        self.bitmaps.insert(name, id);
                        self.doc.bitmaps.push(a);
                    }
                }
                "DOMSoundItem" => {
                    let mut tries = vec![];
                    if let Some(h) = b.attribute("href") {
                        tries.push(format!("LIBRARY/{h}"));
                    }
                    tries.push(format!("LIBRARY/{name}"));
                    if let Some(h) = b.attribute("soundDataHRef") {
                        tries.push(format!("bin/{h}"));
                    }
                    match tries.iter().find_map(|p| self.pkg.get(p)) {
                        Some(data) => {
                            let id = self.doc.sounds.iter().map(|s| s.id).max().unwrap_or(0) + 1;
                            let asset = ac_core::sound::SoundAsset { id, name: name.clone(), data: data.clone() };
                            if ac_core::sound::has_decoder() && ac_core::sound::seconds(&asset).is_none() {
                                self.warn.push(format!("sound '{name}': kept, but this build can't play its format"));
                            }
                            self.sounds.insert(name, id);
                            self.doc.sounds.push(asset);
                        }
                        None => self.warn.push(format!("sound '{name}': data missing")),
                    }
                }
                other => self.warn.push(format!("{other} '{name}' not supported")),
            }
        }
    }
}

impl Reader<'_> {
    /// The document's swatches: solid ones in `swatchLists`, gradient ones in
    /// `extendedSwatchLists` (only the first list of each, as Animate shows).
    fn swatches(&mut self, root: Node) {
        let mut v = vec![];
        let items = |tag: &'static str| child(root, tag).and_then(|l| kids(l, "swatchList").next()).and_then(|l| child(l, "swatches"));
        if let Some(s) = items("swatchLists") {
            for it in kids(s, "SolidSwatchItem") {
                v.push(Swatch::solid("", color(it, "color", "alpha", Color::rgb(0, 0, 0))));
            }
        }
        if let Some(s) = items("extendedSwatchLists") {
            for it in elems(s) {
                let kind = match it.tag_name().name() {
                    "LinearGradientSwatchItem" => GradientKind::Linear,
                    "RadialGradientSwatchItem" => GradientKind::Radial,
                    "swatchListInfo" => continue,
                    other => {
                        self.warn.push(format!("swatch {other} skipped"));
                        continue;
                    }
                };
                let stops: Vec<(f32, Color)> = kids(it, "GradientEntry").map(|e| (f(e, "ratio", 0.0).clamp(0.0, 1.0), color(e, "color", "alpha", Color::rgb(0, 0, 0)))).collect();
                if stops.is_empty() {
                    continue;
                }
                let mut sw = Swatch::gradient("", kind, stops);
                if let Some(g) = &mut sw.gradient {
                    g.focal = f(it, "focalPointRatio", 0.0).clamp(-1.0, 1.0);
                    g.spread = match it.attribute("spreadMethod") {
                        Some("reflect") => Spread::Reflect,
                        Some("repeat") => Spread::Repeat,
                        _ => Spread::Pad,
                    };
                }
                v.push(sw);
            }
        }
        if !v.is_empty() {
            self.doc.swatches = Some(v);
        }
    }
}

pub(crate) fn read(pkg: &Package) -> Result<crate::Imported, String> {
    let mut r = Reader { pkg, warn: vec![], symbols: HashMap::new(), bitmaps: HashMap::new(), sounds: HashMap::new(), doc: Document::default(), brushes: HashMap::new(), extra: vec![] };
    let text = r.xml("DOMDocument.xml")?;
    let xml = roxmltree::Document::parse(&text).map_err(|e| format!("DOMDocument.xml: {e}"))?;
    let root = xml.root_element();
    if root.tag_name().name() != "DOMDocument" {
        return Err("DOMDocument.xml has no DOMDocument".into());
    }
    r.doc.width = f(root, "width", 550.0);
    r.doc.height = f(root, "height", 400.0);
    r.doc.fps = f(root, "frameRate", 24.0);
    r.doc.background = color(root, "backgroundColor", "", Color::rgb(255, 255, 255));
    r.media(root);
    let brush_file = child(root, "brushdefinitions").and_then(|b| kids(b, "Include").next()).and_then(|i| i.attribute("href")).unwrap_or("PaintBrushDefinitions.xml").to_string();
    if let Ok(t) = r.xml(&brush_file) {
        let (b, notes) = crate::brush::parse_definitions(&t);
        r.brushes = b;
        r.warn.extend(notes);
    }
    r.swatches(root);
    // library folders (empty ones too) and their names
    r.doc.folders = child(root, "folders").into_iter().flat_map(|f| kids(f, "DOMFolderItem")).filter_map(|f| f.attribute("name")).map(|s| s.to_string()).collect();

    // symbols: names → ids first (instances can point at any of them)
    let mut sym_xml: Vec<(String, String)> = vec![];
    for inc in child(root, "symbols").into_iter().flat_map(|s| kids(s, "Include")) {
        let href = inc.attribute("href").unwrap_or("");
        match r.xml(&format!("LIBRARY/{href}")) {
            Ok(t) => sym_xml.push((href.to_string(), t)),
            Err(e) => r.warn.push(e),
        }
    }
    let parsed: Vec<(String, roxmltree::Document)> = sym_xml.iter().filter_map(|(h, t)| roxmltree::Document::parse(t).map(|d| (h.clone(), d)).map_err(|e| format!("{h}: {e}")).ok()).collect();
    if parsed.len() < sym_xml.len() {
        r.warn.push("some library files could not be parsed".into());
    }
    for (i, (href, d)) in parsed.iter().enumerate() {
        let name = d.root_element().attribute("name").map(|s| s.to_string()).unwrap_or_else(|| href.trim_end_matches(".xml").to_string());
        r.symbols.insert(name, i as SymbolId + 1);
    }
    for (i, (href, d)) in parsed.iter().enumerate() {
        let s = d.root_element();
        let name = s.attribute("name").map(|s| s.to_string()).unwrap_or_else(|| href.trim_end_matches(".xml").to_string());
        let kind = match s.attribute("symbolType") {
            Some("graphic") => SymbolKind::Graphic,
            Some("button") => SymbolKind::Button,
            _ => SymbolKind::MovieClip,
        };
        let tl = child(s, "timeline").and_then(|t| child(t, "DOMTimeline")).map(|t| r.timeline(t)).unwrap_or_else(|| Timeline::new(&name));
        if flag(s, "linkageExportForAS") || s.attribute("linkageClassName").is_some() {
            if let Some(c) = s.attribute("linkageClassName").or(s.attribute("linkageIdentifier")).filter(|c| !c.is_empty()) {
                r.doc.linkage.insert(i as SymbolId + 1, c.to_string());
            }
        }
        r.doc.library.push(Symbol { id: i as SymbolId + 1, name, kind, timeline: tl });
    }
    r.doc.next_symbol_id = r.doc.library.len() as SymbolId + 1;

    let mut scenes: Vec<Timeline> = child(root, "timelines").into_iter().flat_map(|t| kids(t, "DOMTimeline")).collect::<Vec<_>>().into_iter().map(|t| r.timeline(t)).collect();
    if scenes.is_empty() {
        scenes.push(Timeline::new("Scene 1"));
    }
    r.doc.scene = scenes.remove(0);
    r.doc.more_scenes = scenes;
    let warn = dedup(r.warn);
    Ok(crate::Imported { doc: r.doc, warnings: warn })
}

/// A motion tween's property keyframes from `<motionObjectXML>`
/// (`<AnimationCore TimeScale=… ><PropertyContainer>…<Property id="Motion_X">
/// <Keyframe anchor="t,v" timevalue=…/>`). Values are offsets from the
/// object's starting pose: pixels, degrees, and percent for scale.
/// Experimental: built from the published structure, without a corpus.
fn motion_object(fr: Node, el: &Element, fps: f32) -> Option<ac_core::motion::Motion> {
    let mo = child(fr, "motionObjectXML")?;
    let owned;
    let core = match mo.descendants().find(|n| n.tag_name().name() == "AnimationCore") {
        Some(c) => c,
        None => {
            let text: String = mo.children().filter_map(|t| t.text()).collect();
            owned = roxmltree::Document::parse(text.trim()).ok()?;
            let r = owned.root_element();
            if r.tag_name().name() != "AnimationCore" {
                return None;
            }
            // re-enter with the parsed tree
            return motion_from_core(r, el, fps);
        }
    };
    motion_from_core(core, el, fps)
}

/// One property's keyframes: (time, value) anchors with Bezier handles
/// (`previous` / `next`: time offset and absolute value), in frames.
#[derive(Clone, Copy, Debug)]
struct MotionKey {
    t: f32,
    v: f32,
    prev: (f32, f32),
    next: (f32, f32),
}

/// Value of a property curve at `t` (frames): each span is a cubic Bezier in
/// (time, value) through the anchors and their handles.
fn curve_at(keys: &[MotionKey], t: f32) -> f32 {
    let Some(first) = keys.first() else { return 0.0 };
    if t <= first.t {
        return first.v;
    }
    let Some(i) = keys.iter().position(|k| k.t >= t) else { return keys.last().unwrap().v };
    let (a, b) = (keys[i - 1], keys[i]);
    if b.t - a.t <= 1e-6 {
        return b.v;
    }
    // control points (handles are clamped inside the span so time stays monotonic)
    let span = b.t - a.t;
    let p0 = (a.t, a.v);
    let p1 = (a.t + a.next.0.clamp(0.0, span), a.next.1);
    let p2 = (b.t + b.prev.0.clamp(-span, 0.0), b.prev.1);
    let p3 = (b.t, b.v);
    let bez = |s: f32, c: [f32; 4]| {
        let u = 1.0 - s;
        u * u * u * c[0] + 3.0 * u * u * s * c[1] + 3.0 * u * s * s * c[2] + s * s * s * c[3]
    };
    let (xs, ys) = ([p0.0, p1.0, p2.0, p3.0], [p0.1, p1.1, p2.1, p3.1]);
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    for _ in 0..40 {
        let mid = (lo + hi) / 2.0;
        if bez(mid, xs) < t {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    bez((lo + hi) / 2.0, ys)
}

fn pair(k: Node, attr: &str, scale: f32, fps: f32, default: (f32, f32)) -> (f32, f32) {
    let Some(s) = k.attribute(attr) else { return default };
    let mut it = s.split(',').map(|x| x.trim().parse::<f32>().ok());
    match (it.next().flatten(), it.next().flatten()) {
        (Some(t), Some(v)) => (t / scale * fps, v),
        _ => default,
    }
}

fn motion_from_core(core: Node, el: &Element, fps: f32) -> Option<ac_core::motion::Motion> {
    use ac_core::motion::{Motion, PathKey, Pose};
    let scale = f(core, "TimeScale", 24000.0).max(1.0);
    let duration = f(core, "duration", 0.0) / scale * fps;
    // the tween's overall ease (TimeMap strength −100…100), applied to time
    let ease = core.descendants().find(|n| n.tag_name().name() == "TimeMap").map(|n| f(n, "strength", 0.0)).unwrap_or(0.0);
    let props: std::collections::HashMap<String, (Vec<MotionKey>, bool)> = core
        .descendants()
        .filter(|n| n.tag_name().name() == "Property" && n.attribute("enabled") != Some("0"))
        .filter_map(|p| {
            let keys: Vec<MotionKey> = kids(p, "Keyframe")
                .filter_map(|k| {
                    let t = f(k, "timevalue", 0.0) / scale * fps;
                    let v = k.attribute("anchor")?.split(',').next_back()?.trim().parse::<f32>().ok()?;
                    Some(MotionKey { t, v, prev: pair(k, "previous", scale, fps, (0.0, v)), next: pair(k, "next", scale, fps, (0.0, v)) })
                })
                .collect();
            let timed = p.attribute("ignoreTimeMap") != Some("1");
            (!keys.is_empty()).then(|| (p.attribute("id").unwrap_or("").to_string(), (keys, timed)))
        })
        .collect();
    if props.is_empty() {
        return None;
    }
    // curved (handles off the straight line between anchors) or eased: sample
    // every frame so the result matches; otherwise keep just the keys
    let straight = |keys: &[MotionKey]| keys.windows(2).all(|w| (w[0].next.0.abs() < 1e-3 || (w[0].next.1 - w[0].v).abs() < 1e-3) && (w[1].prev.0.abs() < 1e-3 || (w[1].prev.1 - w[1].v).abs() < 1e-3));
    let time_of = |fr: f32, timed: bool| -> f32 {
        if !timed || ease == 0.0 || duration <= 0.0 {
            return fr;
        }
        ac_core::model::apply_ease((fr / duration).clamp(0.0, 1.0), ease) * duration
    };
    let frames_for = |keys: &[MotionKey], timed: bool| -> Vec<u32> {
        let last = keys.last().map(|k| k.t.round() as u32).unwrap_or(0);
        if straight(keys) && (ease == 0.0 || !timed) {
            keys.iter().map(|k| k.t.round().max(0.0) as u32).collect()
        } else {
            (0..=last).collect()
        }
    };
    let base = Pose::of(el, pt(0.0, 0.0));
    let value = |id: &str, fr: u32| -> f32 { props.get(id).map(|(k, timed)| curve_at(k, time_of(fr as f32, *timed))).unwrap_or(0.0) };
    let mut m = Motion::new(pt(0.0, 0.0));
    let mut frames: Vec<u32> = ["Motion_X", "Motion_Y"].iter().filter_map(|id| props.get(*id)).flat_map(|(k, timed)| frames_for(k, *timed)).collect();
    frames.sort();
    frames.dedup();
    for fr in frames {
        m.path.push(PathKey { frame: fr, at: pt(base.pos.x + value("Motion_X", fr), base.pos.y + value("Motion_Y", fr)), h_in: Pt::default(), h_out: Pt::default() });
    }
    let track = |id: &str, f: &dyn Fn(f32) -> f32, t: &mut ac_core::motion::Track| {
        if let Some((k, timed)) = props.get(id) {
            for fr in frames_for(k, *timed) {
                t.set(fr, f(value(id, fr)));
            }
        }
    };
    track("Rotation_Z", &|v| base.rotation + v, &mut m.rotation);
    track("Skew_X", &|v| base.skew + v, &mut m.skew);
    track("Scale_X", &|v| base.scale_x * v / 100.0, &mut m.scale_x);
    track("Scale_Y", &|v| base.scale_y * v / 100.0, &mut m.scale_y);
    Some(m)
}

pub(crate) fn dedup(v: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    for w in v {
        if !out.contains(&w) {
            out.push(w);
        }
    }
    out
}

#[allow(dead_code)]
pub(crate) fn path_of(chains: &[(Vec<Piece>, bool)]) -> Path {
    edges::chains_path(chains)
}

/// An XFL text face ("Arial-BoldItalicMT", "TimesNewRomanPSMT", "Ubuntu Light")
/// → (family, weight, italic).
pub(crate) fn split_face(face: &str) -> (Option<String>, u16, bool) {
    let (fam, style) = match face.split_once('-') {
        Some((a, b)) => (a.to_string(), b.to_string()),
        None => (face.to_string(), String::new()),
    };
    let strip = |s: &str| -> String {
        let mut s = s.to_string();
        for suf in ["PSMT", "MT", "PS"] {
            if s.len() > suf.len() + 2 && s.ends_with(suf) {
                s.truncate(s.len() - suf.len());
                break;
            }
        }
        s
    };
    let fam = strip(&fam);
    // "TimesNewRoman" → "Times New Roman" (PostScript names drop the spaces)
    let fam = if fam.contains(' ') {
        fam
    } else {
        let mut out = String::new();
        let cs: Vec<char> = fam.chars().collect();
        for (i, c) in cs.iter().enumerate() {
            if i > 0 && c.is_uppercase() && cs[i - 1].is_lowercase() {
                out.push(' ');
            }
            out.push(*c);
        }
        out
    };
    let st = strip(&style).to_ascii_lowercase();
    let weight = if st.contains("black") || st.contains("heavy") {
        900
    } else if st.contains("extrabold") || st.contains("ultrabold") {
        800
    } else if st.contains("semibold") || st.contains("demi") {
        600
    } else if st.contains("bold") {
        700
    } else if st.contains("medium") {
        500
    } else if st.contains("extralight") || st.contains("ultralight") {
        200
    } else if st.contains("light") {
        300
    } else if st.contains("thin") {
        100
    } else {
        400
    };
    let italic = st.contains("italic") || st.contains("oblique");
    (Some(fam).filter(|f| !f.is_empty()), weight, italic)
}

#[cfg(test)]
mod motion_tests {
    use super::{curve_at, MotionKey};

    #[test]
    fn curve_follows_handles() {
        // flat handles: plain linear ramp
        let lin = [MotionKey { t: 0.0, v: 0.0, prev: (0.0, 0.0), next: (0.0, 0.0) }, MotionKey { t: 10.0, v: 100.0, prev: (0.0, 100.0), next: (0.0, 100.0) }];
        assert!((curve_at(&lin, 0.0) - 0.0).abs() < 1e-3);
        assert!((curve_at(&lin, 10.0) - 100.0).abs() < 1e-3);
        // handles pulling the value up first (an arc): mid value above both ends
        let arc = [MotionKey { t: 0.0, v: 0.0, prev: (0.0, 0.0), next: (3.0, -60.0) }, MotionKey { t: 10.0, v: 0.0, prev: (-3.0, -60.0), next: (0.0, 0.0) }];
        let mid = curve_at(&arc, 5.0);
        assert!(mid < -30.0, "arc midpoint {mid}");
        assert!((curve_at(&arc, 10.0)).abs() < 1e-3);
        // before the first / after the last key
        assert_eq!(curve_at(&arc, -5.0), 0.0);
        assert_eq!(curve_at(&arc, 50.0), 0.0);
    }
}
