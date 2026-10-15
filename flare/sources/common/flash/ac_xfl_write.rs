//! Document → XFL package.

use crate::edges::{self, Piece};
use crate::Package;
use ac_core::fx::{BevelType, BlendMode, Filter};
use ac_core::geom::{Matrix, Path, Pt};
use ac_core::merge::MergeShape;
use ac_core::model::*;
use ac_core::motion::Ease;
use std::fmt::Write as _;

const GRAD: f32 = 819.2;
/// Interoperability identifiers. The XFL namespace and the package mimetype
/// are how the authoring tool recognises an XFL document; they are functional
/// names, written only into .fla/XFL exports (never into Flare's own
/// .acx files) and used for nothing else. See CONTRIBUTING.md.
const NS: &str = r#"xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns="http://ns.adobe.com/xfl/2008/""#;

fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\n' => o.push_str("&#xD;"),
            '\r' => {}
            c => o.push(c),
        }
    }
    o
}

fn n(v: f32) -> String {
    if (v - v.round()).abs() < 1e-4 {
        format!("{}", v.round() as i64)
    } else {
        let s = format!("{v:.4}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

fn alpha_attr(c: Color, name: &str) -> String {
    if c.a == 255 {
        String::new()
    } else {
        format!(r#" {name}="{}""#, n(c.a as f32 / 255.0))
    }
}

fn matrix_xml(m: &Matrix, ind: &str) -> String {
    if *m == Matrix::IDENTITY {
        return String::new();
    }
    let mut a = String::new();
    for (k, v, d) in [("a", m.a, 1.0), ("b", m.b, 0.0), ("c", m.c, 0.0), ("d", m.d, 1.0), ("tx", m.tx, 0.0), ("ty", m.ty, 0.0)] {
        if (v - d).abs() > 1e-6 {
            let _ = write!(a, r#" {k}="{}""#, n(v));
        }
    }
    format!("{ind}<matrix>\n{ind}  <Matrix{a}/>\n{ind}</matrix>\n")
}

/// One fill style in a table.
#[derive(Clone, PartialEq)]
enum FillKey {
    Solid(Color),
    Grad(Gradient),
    Bitmap(BitmapFill),
}

struct W<'a> {
    doc: &'a Document,
    warn: Vec<String>,
}

impl W<'_> {
    fn sym_name(&self, id: SymbolId) -> String {
        self.doc.library.iter().find(|s| s.id == id).map(|s| s.name.clone()).unwrap_or_else(|| format!("Symbol {id}"))
    }
    fn bmp_name(&self, id: ac_core::bitmap::BitmapId) -> String {
        self.doc.bitmaps.iter().find(|b| b.id == id).map(|b| b.name.clone()).unwrap_or_else(|| format!("Bitmap {id}"))
    }

    fn fill_xml(&self, f: &FillKey, ind: &str) -> String {
        match f {
            FillKey::Solid(c) => format!("{ind}<SolidColor color=\"{}\"{}/>\n", c.to_hex(), alpha_attr(*c, "alpha")),
            FillKey::Grad(g) => {
                let tag = if g.kind == GradientKind::Linear { "LinearGradient" } else { "RadialGradient" };
                let m = g.matrix.concat(&Matrix::scale(1.0 / GRAD, 1.0 / GRAD));
                let spread = match g.spread {
                    Spread::Pad => "",
                    Spread::Reflect => r#" spreadMethod="reflect""#,
                    Spread::Repeat => r#" spreadMethod="repeat""#,
                };
                let focal = if g.kind == GradientKind::Radial && g.focal != 0.0 { format!(" focalPointRatio=\"{}\"", n(g.focal)) } else { String::new() };
                let mut s = format!("{ind}<{tag}{spread}{focal}>\n{}", matrix_xml(&m, &format!("{ind}  ")));
                for (r, c) in &g.stops {
                    let _ = writeln!(s, "{ind}  <GradientEntry color=\"{}\"{} ratio=\"{}\"/>", c.to_hex(), alpha_attr(*c, "alpha"), n(*r));
                }
                let _ = writeln!(s, "{ind}</{tag}>");
                s
            }
            FillKey::Bitmap(b) => {
                let m = Matrix { a: b.matrix.a * 20.0, b: b.matrix.b * 20.0, c: b.matrix.c * 20.0, d: b.matrix.d * 20.0, ..b.matrix };
                format!("{ind}<BitmapFill bitmapPath=\"{}\">\n{}{ind}</BitmapFill>\n", esc(&self.bmp_name(b.bitmap)), matrix_xml(&m, &format!("{ind}  ")))
            }
        }
    }

    fn stroke_xml(&self, s: &Stroke, st: &StrokeStyle, ind: &str) -> String {
        let caps = match st.cap {
            Cap::None => "none",
            Cap::Round => "round",
            Cap::Square => "square",
        };
        let joints = match st.join {
            Join::Miter => "miter",
            Join::Round => "round",
            Join::Bevel => "bevel",
        };
        let (tag, extra) = match st.dash {
            Dash::Solid => ("SolidStroke", String::new()),
            Dash::Dashed => ("DashedStroke", format!(r#" dash1="{}" dash2="{}""#, n(st.dash_len), n(st.gap))),
            Dash::Dotted => ("DottedStroke", format!(r#" dotSpace="{}""#, n(st.gap))),
            Dash::Ragged => ("RaggedStroke", String::new()),
            Dash::Stipple => ("StippleStroke", String::new()),
            Dash::Hatched => ("HatchedStroke", String::new()),
            // OpenToonz styles have no .fla form (warned by the caller)
            _ => ("SolidStroke", String::new()),
        };
        let mut caps_a = String::new();
        if caps != "round" {
            caps_a = format!(r#" caps="{caps}""#);
        }
        if joints != "round" {
            let _ = write!(caps_a, r#" joints="{joints}""#);
        }
        format!(
            "{ind}<{tag} scaleMode=\"normal\" weight=\"{}\"{caps_a}{extra}>\n{ind}  <fill>\n{ind}    <SolidColor color=\"{}\"{}/>\n{ind}  </fill>\n{ind}</{tag}>\n",
            n(s.width),
            s.color.to_hex(),
            alpha_attr(s.color, "alpha")
        )
    }

    /// `<DOMShape>` from style tables + edge records (fill0, fill1, stroke, pieces).
    fn dom_shape(&self, drawing_object: bool, fills: &[FillKey], strokes: &[(Stroke, StrokeStyle)], recs: &[(u32, u32, u32, Vec<Piece>)], ind: &str) -> String {
        let mut s = format!("{ind}<DOMShape{}>\n", if drawing_object { r#" isDrawingObject="true""# } else { "" });
        if !fills.is_empty() {
            let _ = writeln!(s, "{ind}  <fills>");
            for (i, f) in fills.iter().enumerate() {
                let _ = writeln!(s, "{ind}    <FillStyle index=\"{}\">", i + 1);
                s.push_str(&self.fill_xml(f, &format!("{ind}      ")));
                let _ = writeln!(s, "{ind}    </FillStyle>");
            }
            let _ = writeln!(s, "{ind}  </fills>");
        }
        if !strokes.is_empty() {
            let _ = writeln!(s, "{ind}  <strokes>");
            for (i, (st, sty)) in strokes.iter().enumerate() {
                let _ = writeln!(s, "{ind}    <StrokeStyle index=\"{}\">", i + 1);
                s.push_str(&self.stroke_xml(st, sty, &format!("{ind}      ")));
                let _ = writeln!(s, "{ind}    </StrokeStyle>");
            }
            let _ = writeln!(s, "{ind}  </strokes>");
        }
        let _ = writeln!(s, "{ind}  <edges>");
        for (f0, f1, st, p) in recs {
            if p.is_empty() {
                continue;
            }
            let mut a = String::new();
            if *f0 != 0 {
                let _ = write!(a, r#" fillStyle0="{f0}""#);
            }
            if *f1 != 0 {
                let _ = write!(a, r#" fillStyle1="{f1}""#);
            }
            if *st != 0 {
                let _ = write!(a, r#" strokeStyle="{st}""#);
            }
            let _ = writeln!(s, "{ind}    <Edge{a} edges=\"{}\"/>", edges::write(std::slice::from_ref(p)));
        }
        let _ = writeln!(s, "{ind}  </edges>");
        let _ = writeln!(s, "{ind}</DOMShape>");
        s
    }

    /// Which side a closed contour's fill is on: fillStyle1 (right, y down)
    /// when the region is to its right.
    fn side(pts: &[Pt], hole: bool) -> bool {
        let cw = edges::signed_area(pts) > 0.0;
        cw != hole
    }

    fn canvas(&mut self, c: &MergeShape, m: &Matrix, ind: &str) -> String {
        let mut fills: Vec<FillKey> = vec![];
        let mut strokes: Vec<(Stroke, StrokeStyle)> = vec![];
        let mut recs = vec![];
        let tp = |p: Pt| m.apply(p);
        for r in &c.fills {
            let key = match (&r.bitmap, &r.gradient, r.fill) {
                (Some(b), _, _) => FillKey::Bitmap(BitmapFill { bitmap: b.bitmap, matrix: m.concat(&b.matrix) }),
                (None, Some(g), _) => FillKey::Grad(Gradient { matrix: m.concat(&g.matrix), ..g.clone() }),
                (None, None, Some(c)) => FillKey::Solid(c),
                (None, None, None) => continue,
            };
            let idx = match fills.iter().position(|f| *f == key) {
                Some(i) => i,
                None => {
                    fills.push(key);
                    fills.len() - 1
                }
            } as u32
                + 1;
            for (ci, cont) in r.contours.iter().enumerate() {
                if cont.len() < 3 {
                    continue;
                }
                let pts: Vec<Pt> = cont.iter().map(|p| tp(*p)).collect();
                let right = Self::side(&pts, ci > 0);
                let p = edges::poly_pieces(&pts, true);
                recs.push(if right { (0, idx, 0, p) } else { (idx, 0, 0, p) });
            }
        }
        if c.strokes.iter().any(|e| e.width.is_some()) {
            self.warn.push("variable-width lines exported at their base width".into());
        }
        for e in &c.strokes {
            let key = (e.stroke, StrokeStyle::default());
            let idx = match strokes.iter().position(|s| *s == key) {
                Some(i) => i,
                None => {
                    strokes.push(key);
                    strokes.len() - 1
                }
            } as u32
                + 1;
            let pts: Vec<Pt> = e.pts.iter().map(|p| tp(*p)).collect();
            recs.push((0, 0, idx, edges::poly_pieces(&pts, e.closed)));
        }
        if c.float.is_some() {
            let mut c2 = c.clone();
            c2.commit_float();
            return self.canvas(&c2, m, ind);
        }
        self.dom_shape(false, &fills, &strokes, &share_edges(recs), ind)
    }

    /// A drawing-object shape, baked into parent coordinates by `m`.
    fn shape(&mut self, sh: &Shape, m: &Matrix, ind: &str) -> String {
        if sh.warp.is_some() || sh.width_profile.is_some() {
            self.warn.push("Asset Warp / variable width exported as the plain shape".into());
        }
        let local = match &sh.rig {
            Some(r) => r.deform(&sh.path),
            None => sh.path.clone(),
        };
        let path: Path = local.transformed(m);
        let fill = if let Some(b) = &sh.bitmap_fill {
            Some(FillKey::Bitmap(BitmapFill { bitmap: b.bitmap, matrix: m.concat(&b.matrix) }))
        } else if let Some(g) = &sh.gradient {
            Some(FillKey::Grad(Gradient { matrix: m.concat(&g.matrix), ..g.clone() }))
        } else {
            sh.fill.map(FillKey::Solid)
        };
        let fills: Vec<FillKey> = fill.into_iter().collect();
        if sh.stroke.is_some() && !sh.stroke_style.dash.in_fla() {
            self.warn.push(format!("{} strokes (OpenToonz style) exported as solid lines", sh.stroke_style.dash.name()));
        }
        let strokes: Vec<(Stroke, StrokeStyle)> = sh.stroke.map(|s| (s, sh.stroke_style)).into_iter().collect();
        let subs = edges::path_pieces(&path);
        // which subpaths are holes (inside an odd number of the others)
        let polys: Vec<Vec<Pt>> = subs
            .iter()
            .map(|(p, _)| {
                let mut v = vec![p[0].a];
                for x in p {
                    x.flatten_into(&mut v);
                }
                v
            })
            .collect();
        let mut recs = vec![];
        let has_fill = !fills.is_empty();
        let st = if strokes.is_empty() { 0 } else { 1 };
        for (i, (pieces, closed)) in subs.iter().enumerate() {
            let (mut f0, mut f1) = (0, 0);
            if has_fill && polys[i].len() >= 3 {
                let hole = polys.iter().enumerate().filter(|(j, q)| *j != i && q.len() >= 3 && edges::point_in(q, polys[i][0])).count() % 2 == 1;
                if Self::side(&polys[i], hole) {
                    f1 = 1;
                } else {
                    f0 = 1;
                }
            }
            recs.push((f0, f1, st, pieces.clone()));
            if !*closed && has_fill && f0 + f1 > 0 {
                // the fill closes an open path without a line
                let (a, b) = (pieces.last().unwrap().b, pieces[0].a);
                if a.dist(b) > 1e-3 {
                    recs.push((f0, f1, 0, vec![Piece { a, ctrl: None, b }]));
                }
            }
        }
        self.dom_shape(true, &fills, &strokes, &recs, ind)
    }

    fn color_xml(e: &Element, ind: &str) -> String {
        let mut a = String::new();
        if let Some(cx) = e.color_fx {
            for (i, ch) in ["red", "green", "blue", "alpha"].iter().enumerate() {
                if cx.mul[i] != 1.0 {
                    let _ = write!(a, r#" {ch}Multiplier="{}""#, n(cx.mul[i]));
                }
                if cx.add[i] != 0.0 {
                    let _ = write!(a, r#" {ch}Offset="{}""#, n(cx.add[i]));
                }
            }
        }
        if let Some((c, amt)) = e.tint {
            let _ = write!(a, r#" tintMultiplier="{}" tintColor="{}""#, n(amt), c.to_hex());
        }
        if e.alpha < 1.0 && e.color_fx.is_none() {
            let _ = write!(a, r#" alphaMultiplier="{}""#, n(e.alpha));
        }
        if a.is_empty() {
            return String::new();
        }
        format!("{ind}<color>\n{ind}  <Color{a}/>\n{ind}</color>\n")
    }

    fn filters_xml(fs: &[Filter], ind: &str) -> String {
        if fs.is_empty() {
            return String::new();
        }
        let mut s = format!("{ind}<filters>\n");
        let c = |c: Color, name: &str, al: &str| format!(r#" {name}="{}"{}"#, c.to_hex(), alpha_attr(c, al));
        let b = |name: &str, v: bool| if v { format!(r#" {name}="true""#) } else { String::new() };
        for f in fs {
            let line = match f {
                // no XFL equivalent (warned by the caller)
                Filter::MotionBlur { .. } | Filter::Effect { .. } => continue,
                Filter::DropShadow { blur_x, blur_y, strength, quality, angle, distance, color, knockout, inner, hide_object } => format!(
                    r#"<DropShadowFilter blurX="{}" blurY="{}" strength="{}" quality="{quality}" angle="{}" distance="{}"{}{}{}{}/>"#,
                    n(*blur_x), n(*blur_y), n(strength / 100.0), n(*angle), n(*distance), c(*color, "color", "alpha"), b("knockout", *knockout), b("inner", *inner), b("hideObject", *hide_object)
                ),
                Filter::Blur { blur_x, blur_y, quality } => format!(r#"<BlurFilter blurX="{}" blurY="{}" quality="{quality}"/>"#, n(*blur_x), n(*blur_y)),
                Filter::Glow { blur_x, blur_y, strength, quality, color, knockout, inner } => format!(
                    r#"<GlowFilter blurX="{}" blurY="{}" strength="{}" quality="{quality}"{}{}{}/>"#,
                    n(*blur_x), n(*blur_y), n(strength / 100.0), c(*color, "color", "alpha"), b("knockout", *knockout), b("inner", *inner)
                ),
                Filter::Bevel { blur_x, blur_y, strength, quality, shadow, highlight, angle, distance, knockout, kind } => format!(
                    r#"<BevelFilter blurX="{}" blurY="{}" strength="{}" quality="{quality}" angle="{}" distance="{}"{}{}{} type="{}"/>"#,
                    n(*blur_x), n(*blur_y), n(strength / 100.0), n(*angle), n(*distance), c(*shadow, "shadowColor", "shadowAlpha"), c(*highlight, "highlightColor", "highlightAlpha"), b("knockout", *knockout),
                    match kind { BevelType::Inner => "inner", BevelType::Outer => "outer", BevelType::Full => "full" }
                ),
                Filter::AdjustColor { brightness, contrast, saturation, hue } => format!(r#"<AdjustColorFilter brightness="{}" contrast="{}" saturation="{}" hue="{}"/>"#, n(*brightness), n(*contrast), n(*saturation), n(*hue)),
                Filter::GradientGlow { blur_x, blur_y, strength, quality, angle, distance, knockout, kind, stops } | Filter::GradientBevel { blur_x, blur_y, strength, quality, angle, distance, knockout, kind, stops } => {
                    let tag = if matches!(f, Filter::GradientGlow { .. }) { "GradientGlowFilter" } else { "GradientBevelFilter" };
                    let mut e = format!(
                        r#"<{tag} blurX="{}" blurY="{}" strength="{}" quality="{quality}" angle="{}" distance="{}"{} type="{}">"#,
                        n(*blur_x), n(*blur_y), n(strength / 100.0), n(*angle), n(*distance), b("knockout", *knockout),
                        match kind { BevelType::Inner => "inner", BevelType::Outer => "outer", BevelType::Full => "full" }
                    );
                    for (r, col) in stops {
                        let _ = write!(e, "\n{ind}    <GradientEntry color=\"{}\"{} ratio=\"{}\"/>", col.to_hex(), alpha_attr(*col, "alpha"), n(*r));
                    }
                    let _ = write!(e, "\n{ind}  </{tag}>");
                    e
                }
            };
            let _ = writeln!(s, "{ind}  {line}");
        }
        let _ = writeln!(s, "{ind}</filters>");
        s
    }

    fn blend_name(b: BlendMode) -> String {
        b.name().to_lowercase().replace(' ', "")
    }

    /// Elements, with `parent` folded into shapes (groups have no matrix).
    fn elements(&mut self, els: &[Element], parent: &Matrix, ind: &str) -> String {
        let mut s = String::new();
        for e in els {
            let m = parent.concat(&e.matrix);
            if e.three_d.is_some() {
                self.warn.push("3D rotation/translation not exported".into());
            }
            if e.mesh.as_ref().is_some_and(|w| !w.pins.is_empty()) {
                self.warn.push("Asset Warp on bitmaps/symbols has no .fla form: exported unwarped".into());
            }
            let effects = e.alpha < 1.0 || e.tint.is_some() || e.color_fx.is_some() || !e.filters.is_empty() || e.blend != BlendMode::Normal;
            match &e.kind {
                ElementKind::Particles(_) => self.warn.push("particle emitters have no .fla form: left out (export a video or image sequence to keep them)".into()),
                ElementKind::Canvas(c) => s.push_str(&self.canvas(c, &m, ind)),
                ElementKind::Shape(sh) => {
                    if effects {
                        self.warn.push("colour effects/filters on shapes are not part of the format: dropped (convert to a symbol to keep them)".into());
                    }
                    s.push_str(&self.shape(sh, &m, ind));
                }
                ElementKind::Group(ch) => {
                    let _ = writeln!(s, "{ind}<DOMGroup>\n{ind}  <members>");
                    s.push_str(&self.elements(ch, &m, &format!("{ind}    ")));
                    let _ = writeln!(s, "{ind}  </members>\n{ind}</DOMGroup>");
                }
                ElementKind::Instance(i) => {
                    let sym = self.doc.library.iter().find(|x| x.id == i.symbol);
                    let mut a = format!(r#" libraryItemName="{}""#, esc(&self.sym_name(i.symbol)));
                    if !e.name.is_empty() {
                        let _ = write!(a, r#" name="{}""#, esc(&e.name));
                    }
                    match sym.map(|x| x.kind) {
                        Some(SymbolKind::Graphic) => {
                            let lp = match i.loop_mode {
                                LoopMode::Loop => "loop",
                                LoopMode::PlayOnce => "play once",
                                LoopMode::SingleFrame => "single frame",
                                LoopMode::PlayReverseOnce => "play once reverse",
                                LoopMode::LoopReverse => "loop reverse",
                            };
                            let _ = write!(a, r#" symbolType="graphic" loop="{lp}""#);
                            if i.first_frame > 0 {
                                let _ = write!(a, r#" firstFrame="{}""#, i.first_frame);
                            }
                        }
                        Some(SymbolKind::Button) => a.push_str(r#" symbolType="button""#),
                        _ => {}
                    }
                    if e.blend != BlendMode::Normal {
                        let _ = write!(a, r#" blendMode="{}""#, Self::blend_name(e.blend));
                    }
                    let _ = writeln!(s, "{ind}<DOMSymbolInstance{a}>");
                    let inner = format!("{ind}  ");
                    s.push_str(&matrix_xml(&m, &inner));
                    s.push_str(&Self::color_xml(e, &inner));
                    s.push_str(&Self::filters_xml(&e.filters, &inner));
                    if e.filters.iter().any(|f| matches!(f, Filter::MotionBlur { .. } | Filter::Effect { .. })) {
                        self.warn.push("Motion Blur and EffectCraft effects have no .fla equivalent and were left out".into());
                    }
                    let _ = writeln!(s, "{ind}</DOMSymbolInstance>");
                }
                ElementKind::Bitmap(b) => {
                    if effects {
                        self.warn.push("colour effects/filters on bitmaps dropped".into());
                    }
                    let _ = writeln!(s, "{ind}<DOMBitmapInstance libraryItemName=\"{}\"{}>", esc(&self.bmp_name(b.bitmap)), if e.name.is_empty() { String::new() } else { format!(r#" name="{}""#, esc(&e.name)) });
                    s.push_str(&matrix_xml(&m, &format!("{ind}  ")));
                    let _ = writeln!(s, "{ind}</DOMBitmapInstance>");
                }
                ElementKind::Text(t) => {
                    use ac_core::text::{Baseline, TextKind};
                    let (w, h) = t.box_size();
                    let (w, h) = (w.max(1.0), h.max(1.0));
                    let mut face = t.family.clone().unwrap_or_else(|| if t.font == ac_core::text::FontFace::Mono { "Hack".into() } else { "Ubuntu Light".into() });
                    if t.weight != 400 || t.italic {
                        // PostScript-style face: Family-Bold, Family-Italic, Family-BoldItalic …
                        let w = match t.weight {
                            0..=149 => "Thin",
                            150..=249 => "ExtraLight",
                            250..=349 => "Light",
                            350..=449 => "",
                            450..=549 => "Medium",
                            550..=649 => "Semibold",
                            650..=749 => "Bold",
                            750..=849 => "ExtraBold",
                            _ => "Black",
                        };
                        face = format!("{}-{w}{}", face.replace(' ', ""), if t.italic { "Italic" } else { "" });
                    }
                    let align = match t.align {
                        ac_core::text::Align::Left => "left",
                        ac_core::text::Align::Center => "center",
                        ac_core::text::Align::Right => "right",
                        ac_core::text::Align::Justify => "justify",
                    };
                    let tag = match t.field.kind {
                        TextKind::Static => "DOMStaticText",
                        TextKind::Dynamic => "DOMDynamicText",
                        TextKind::Input => "DOMInputText",
                    };
                    let mut fa = String::new();
                    if t.para.width.is_none() {
                        fa.push_str(r#" autoExpand="true""#);
                    }
                    if t.field.kind != TextKind::Static {
                        let line = if t.field.password { "password" } else if t.para.width.is_some() { "multiline" } else if t.text.contains('\n') { "multiline no wrap" } else { "single line" };
                        let _ = write!(fa, " lineType=\"{line}\"");
                        if !t.field.selectable {
                            fa.push_str(r#" isSelectable="false""#);
                        }
                        if t.field.max_chars > 0 {
                            let _ = write!(fa, " maxCharacters=\"{}\"", t.field.max_chars);
                        }
                    } else if !t.field.selectable {
                        fa.push_str(r#" isSelectable="false""#);
                    }
                    if t.field.border {
                        fa.push_str(r#" border="true""#);
                    }
                    let _ = writeln!(s, "{ind}<{tag}{} width=\"{}\" height=\"{}\"{fa}>", if e.name.is_empty() { String::new() } else { format!(r#" name="{}""#, esc(&e.name)) }, n(w), n(h));
                    let i2 = format!("{ind}  ");
                    s.push_str(&matrix_xml(&m, &i2));
                    let mut ls = if t.letter_spacing != 0.0 { format!(r#" letterSpacing="{}""#, n(t.letter_spacing)) } else { String::new() };
                    let p = &t.para;
                    for (k, v) in [("lineSpacing", p.line_spacing), ("indent", p.indent), ("leftMargin", p.margin_left), ("rightMargin", p.margin_right)] {
                        if v != 0.0 {
                            let _ = write!(ls, " {k}=\"{}\"", n(v));
                        }
                    }
                    match p.baseline {
                        Baseline::Superscript => ls.push_str(r#" characterPosition="superscript""#),
                        Baseline::Subscript => ls.push_str(r#" characterPosition="subscript""#),
                        Baseline::Normal => {}
                    }
                    if p.no_kern {
                        ls.push_str(r#" autoKern="false""#);
                    }
                    if let Some(u) = &t.field.link {
                        let _ = write!(ls, " url=\"{}\"", esc(u));
                    }
                    let _ = writeln!(
                        s,
                        "{i2}<textRuns>\n{i2}  <DOMTextRun>\n{i2}    <characters>{}</characters>\n{i2}    <textAttrs>\n{i2}      <DOMTextAttrs alignment=\"{align}\" size=\"{}\" face=\"{}\" fillColor=\"{}\"{}{ls}/>\n{i2}    </textAttrs>\n{i2}  </DOMTextRun>\n{i2}</textRuns>",
                        esc(&t.text),
                        n(t.size),
                        esc(&face),
                        t.color.to_hex(),
                        alpha_attr(t.color, "alpha")
                    );
                    let _ = writeln!(s, "{ind}</{tag}>");
                }
            }
        }
        s
    }

    /// `<tweens>` with the span's ease ("all") and per-property eases.
    fn eases_xml(kf: &Keyframe, ind: &str) -> String {
        let mut inner = String::new();
        if let Some(e) = &kf.ease_curve {
            inner.push_str(&Self::ease_xml("all", e, ind));
        }
        if let Some(pe) = &kf.prop_eases {
            for g in ac_core::motion::PropGroup::ALL {
                if let Some(e) = pe.get(g) {
                    inner.push_str(&Self::ease_xml(g.id(), e, ind));
                }
            }
        }
        if inner.is_empty() {
            return inner;
        }
        format!("{ind}<tweens>\n{inner}{ind}</tweens>\n")
    }

    fn ease_xml(target: &str, e: &Ease, ind: &str) -> String {
        let Ease::Custom { points } = e.to_custom() else { return String::new() };
        let mut s = format!("{ind}  <CustomEase target=\"{target}\">\n");
        for (i, p) in points.iter().enumerate() {
            if i > 0 {
                let _ = writeln!(s, "{ind}    <Point x=\"{}\" y=\"{}\"/>", n(p.x + p.h_in.x), n(p.y + p.h_in.y));
            }
            let _ = writeln!(s, "{ind}    <Point x=\"{}\" y=\"{}\"/>", n(p.x), n(p.y));
            if i + 1 < points.len() {
                let _ = writeln!(s, "{ind}    <Point x=\"{}\" y=\"{}\"/>", n(p.x + p.h_out.x), n(p.y + p.h_out.y));
            }
        }
        let _ = writeln!(s, "{ind}  </CustomEase>");
        s
    }

    fn layer(&mut self, l: &Layer, ind: &str) -> String {
        let mut l = l.clone();
        // motion tweens have no closing keyframe: bake them for the classic format
        let motion: Vec<(u32, u32)> = l.keyframes.iter().filter(|(_, k)| k.tween.is_motion()).map(|(s, _)| (*s, l.span_end(*s))).collect();
        if !motion.is_empty() {
            self.warn.push("motion tweens exported frame by frame".into());
            for (s, e) in motion {
                ac_core::frames::convert_to_frame_by_frame(&mut l, s, e.saturating_sub(1));
            }
        }
        if !l.bones.is_empty() {
            self.warn.push("IK armatures not exported (the current pose is kept)".into());
        }
        let mut a = format!(r#" name="{}" color="{}""#, esc(&l.name), l.color.to_hex());
        if !l.visible {
            a.push_str(r#" visible="false""#);
        }
        if l.locked {
            a.push_str(r#" locked="true""#);
        }
        if l.outline {
            a.push_str(r#" useOutlineView="true""#);
        }
        match l.kind {
            LayerKind::Normal => {}
            LayerKind::Folder => a.push_str(r#" layerType="folder""#),
            LayerKind::Guide => a.push_str(r#" layerType="guide""#),
            LayerKind::Mask => a.push_str(r#" layerType="mask" locked="true""#),
            LayerKind::Camera => {
                self.warn.push("camera layer exported as an empty guide layer".into());
                a.push_str(r#" layerType="guide""#);
            }
        }
        if l.collapsed {
            a.push_str(r#" open="false""#);
        }
        if let Some(p) = l.parent {
            let _ = write!(a, r#" parentLayerIndex="{p}""#);
        }
        let mut s = format!("{ind}<DOMLayer{a}>\n{ind}  <frames>\n");
        let i3 = format!("{ind}    ");
        for (&start, kf) in &l.keyframes {
            if start >= l.end {
                continue;
            }
            let dur = l.span_end(start) - start;
            let mut fa = format!(r#" index="{start}""#);
            if dur > 1 {
                let _ = write!(fa, r#" duration="{dur}""#);
            }
            match kf.tween {
                Tween::Classic { ease, rotate_turns } => {
                    fa.push_str(r#" tweenType="motion""#);
                    if kf.snap {
                        fa.push_str(r#" motionTweenSnap="true""#);
                    }
                    if kf.sync {
                        fa.push_str(r#" motionTweenSync="true""#);
                    }
                    if rotate_turns != 0 {
                        let _ = write!(fa, r#" motionTweenRotate="{}" motionTweenRotateTimes="{}""#, if rotate_turns > 0 { "cw" } else { "ccw" }, rotate_turns.abs());
                    }
                    if ease != 0.0 {
                        let _ = write!(fa, r#" acceleration="{}""#, n(ease));
                    }
                    if kf.orient_to_path {
                        fa.push_str(r#" motionTweenOrientToPath="true""#);
                    }
                }
                Tween::Shape { ease } => {
                    fa.push_str(r#" tweenType="shape""#);
                    if ease != 0.0 {
                        let _ = write!(fa, r#" acceleration="{}""#, n(ease));
                    }
                }
                _ => {}
            }
            if (kf.ease_curve.is_some() || kf.prop_eases.is_some()) && !matches!(kf.tween, Tween::None) {
                fa.push_str(r#" hasCustomEase="true""#);
                if kf.prop_eases.is_some() {
                    fa.push_str(r#" useSingleEaseCurve="false""#);
                }
            }
            if let Some(fs) = &kf.sound {
                if let Some(snd) = self.doc.sound(fs.sound) {
                    let _ = write!(fa, r#" soundName="{}""#, esc(&snd.name));
                    if fs.sync != ac_core::sound::SoundSync::Event {
                        let _ = write!(fa, r#" soundSync="{}""#, fs.sync.name().to_lowercase());
                    }
                    if fs.effect != ac_core::sound::SoundEffect::None {
                        self.warn.push("sound effects (fades, channels) are not exported to .fla".into());
                    }
                    if fs.forever {
                        fa.push_str(r#" soundLoopMode="loop""#);
                    } else if fs.repeat > 1 {
                        let _ = write!(fa, r#" soundLoopMode="repeat" soundLoop="{}""#, fs.repeat);
                    }
                }
            }
            if let Some(lb) = &kf.label {
                let _ = write!(fa, r#" name="{}" labelType="{}""#, esc(lb), kf.label_kind.id());
            }
            if !kf.shape_hints.is_empty() {
                self.warn.push("shape hints not exported".into());
            }
            let _ = writeln!(s, "{i3}<DOMFrame{fa}>");
            let i4 = format!("{i3}  ");
            if let Some(code) = &kf.script {
                let _ = writeln!(s, "{i4}<Actionscript>\n{i4}  <script><![CDATA[{}]]></script>\n{i4}</Actionscript>", code.replace("]]>", "]]]]><![CDATA[>"));
            }
            if !matches!(kf.tween, Tween::None) {
                s.push_str(&Self::eases_xml(kf, &i4));
            }
            let els = if l.kind == LayerKind::Camera { String::new() } else { self.elements(&kf.elements, &Matrix::IDENTITY, &format!("{i4}  ")) };
            if els.is_empty() {
                let _ = writeln!(s, "{i4}<elements/>");
            } else {
                let _ = write!(s, "{i4}<elements>\n{els}{i4}</elements>\n");
            }
            let _ = writeln!(s, "{i3}</DOMFrame>");
        }
        let _ = writeln!(s, "{ind}  </frames>\n{ind}</DOMLayer>");
        s
    }

    fn timeline(&mut self, tl: &Timeline, ind: &str) -> String {
        if tl.layers.iter().any(|l| l.anim_parent.is_some()) {
            self.warn.push("layer parenting not exported".into());
        }
        let mut s = format!("{ind}<DOMTimeline name=\"{}\">\n{ind}  <layers>\n", esc(&tl.name));
        for l in &tl.layers {
            s.push_str(&self.layer(l, &format!("{ind}    ")));
        }
        let _ = writeln!(s, "{ind}  </layers>\n{ind}</DOMTimeline>");
        s
    }
}

/// Merge identical straight segments of a raw shape into shared edges, the
/// way the authoring tool stores them: a boundary between two fills (and a
/// line drawn along it) becomes one edge carrying fillStyle0, fillStyle1 and
/// strokeStyle, instead of two or three overlapping edges.
fn share_edges(recs: Vec<(u32, u32, u32, Vec<Piece>)>) -> Vec<(u32, u32, u32, Vec<Piece>)> {
    use std::collections::HashMap;
    type Key = ((i64, i64), (i64, i64));
    let tw = |p: Pt| ((p.x * 20.0).round() as i64, (p.y * 20.0).round() as i64);
    let mut order: Vec<Key> = vec![];
    let mut seg: HashMap<Key, (Piece, u32, u32, u32)> = HashMap::new();
    let mut curved = vec![];
    for (f0, f1, st, pieces) in recs {
        for p in pieces {
            if p.ctrl.is_some() {
                curved.push((f0, f1, st, vec![p]));
                continue;
            }
            let (a, b) = (tw(p.a), tw(p.b));
            if a == b {
                continue;
            }
            // canonical direction: reversing a segment swaps its left and right fills
            let (key, piece, l, r) = if a < b { ((a, b), p, f0, f1) } else { ((b, a), p.reversed(), f1, f0) };
            match seg.get_mut(&key) {
                Some(e) => {
                    if e.1 == 0 {
                        e.1 = l;
                    }
                    if e.2 == 0 {
                        e.2 = r;
                    }
                    if e.3 == 0 {
                        e.3 = st;
                    }
                }
                None => {
                    order.push(key);
                    seg.insert(key, (piece, l, r, st));
                }
            }
        }
    }
    // chain segments that share all three styles back into longer edges
    let mut groups: Vec<((u32, u32, u32), Vec<Piece>)> = vec![];
    for k in order {
        let (p, l, r, st) = seg[&k];
        match groups.iter_mut().find(|g| g.0 == (l, r, st)) {
            Some(g) => g.1.push(p),
            None => groups.push(((l, r, st), vec![p])),
        }
    }
    let mut out = vec![];
    for ((l, r, st), pieces) in groups {
        for (ch, _) in edges::chain(&pieces) {
            out.push((l, r, st, ch));
        }
    }
    out.extend(curved);
    out
}

fn file_safe(name: &str) -> String {
    name.chars().map(|c| if matches!(c, ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\\') { '_' } else { c }).collect()
}

pub(crate) fn write(doc: &Document) -> (Package, Vec<String>) {
    let (doc, warn) = ac_core::frames::for_interchange(&doc.with_floats_committed());
    let mut w = W { doc: &doc, warn };
    let mut pkg = Package::new();
    pkg.insert("mimetype".into(), b"application/vnd.adobe.xfl".to_vec());

    // media
    let mut media = String::new();
    for (i, b) in doc.bitmaps.iter().enumerate() {
        let dat = format!("M {} {}.dat", i + 1, b.id);
        let href = file_safe(&b.name);
        if let Some(img) = image::load_from_memory(&b.png).ok().map(|i| i.to_rgba8()) {
            pkg.insert(format!("bin/{dat}"), crate::dat::encode_lossless(img.width(), img.height(), img.as_raw()));
        }
        pkg.insert(format!("LIBRARY/{href}"), b.png.clone());
        let _ = writeln!(
            media,
            "    <DOMBitmapItem name=\"{}\" href=\"{}\" bitmapDataHRef=\"{dat}\" frameRight=\"{}\" frameBottom=\"{}\" allowSmoothing=\"{}\" isJPEG=\"false\" sourceExternalFilepath=\"./LIBRARY/{}\"/>",
            esc(&b.name),
            esc(&href),
            b.width * 20,
            b.height * 20,
            b.smoothing,
            esc(&href)
        );
    }

    for s in &doc.sounds {
        let href = file_safe(&s.name);
        pkg.insert(format!("LIBRARY/{href}"), s.data.clone());
        let _ = writeln!(media, "    <DOMSoundItem name=\"{}\" href=\"{}\" sourceExternalFilepath=\"./LIBRARY/{}\"/>", esc(&s.name), esc(&href), esc(&href));
    }

    // symbols
    let mut includes = String::new();
    let mut folders: Vec<String> = vec![];
    // folders of media items and stored (possibly empty) folders
    for name in doc.bitmaps.iter().map(|b| b.name.as_str()).chain(doc.sounds.iter().map(|s| s.name.as_str())).chain(doc.folders.iter().map(|f| f.as_str())) {
        let mut parts: Vec<&str> = name.split('/').collect();
        if !doc.folders.iter().any(|f| f == name) {
            parts.pop();
        }
        for k in 1..=parts.len() {
            let f = parts[..k].join("/");
            if !folders.contains(&f) {
                folders.push(f);
            }
        }
    }
    for (i, sym) in doc.library.iter().enumerate() {
        let href = format!("{}.xml", file_safe(&sym.name));
        let mut parts: Vec<&str> = sym.name.split('/').collect();
        parts.pop();
        for k in 1..=parts.len() {
            let f = parts[..k].join("/");
            if !folders.contains(&f) {
                folders.push(f);
            }
        }
        let st = match sym.kind {
            SymbolKind::Graphic => r#" symbolType="graphic""#,
            SymbolKind::Button => r#" symbolType="button""#,
            SymbolKind::MovieClip => "",
        };
        let tl = w.timeline(&sym.timeline, "    ");
        let link = match doc.linkage.get(&sym.id) {
            Some(c) => format!(" linkageExportForAS=\"true\" linkageClassName=\"{}\"", esc(c)),
            None => String::new(),
        };
        let xml = format!(
            "<DOMSymbolItem {NS} name=\"{}\" itemID=\"00000000-{:08x}\"{st}{link}>\n  <timeline>\n{tl}  </timeline>\n</DOMSymbolItem>\n",
            esc(&sym.name),
            i + 1
        );
        pkg.insert(format!("LIBRARY/{href}"), xml.into_bytes());
        let _ = writeln!(includes, "    <Include href=\"{}\"/>", esc(&href));
    }

    let mut timelines = w.timeline(&doc.scene, "    ");
    for sc in &doc.more_scenes {
        timelines.push_str(&w.timeline(sc, "    "));
    }

    let mut x = format!(
        "<DOMDocument {NS} width=\"{}\" height=\"{}\" frameRate=\"{}\" backgroundColor=\"{}\" currentTimeline=\"1\" xflVersion=\"2.97\" creatorInfo=\"Flare\" platform=\"{}\">\n",
        n(doc.width),
        n(doc.height),
        n(doc.fps),
        doc.background.to_hex(),
        if cfg!(windows) { "Windows" } else { "Macintosh" }
    );
    if !folders.is_empty() {
        x.push_str("  <folders>\n");
        for f in &folders {
            let _ = writeln!(x, "    <DOMFolderItem name=\"{}\" isExpanded=\"true\"/>", esc(f));
        }
        x.push_str("  </folders>\n");
    }
    if !media.is_empty() {
        let _ = write!(x, "  <media>\n{media}  </media>\n");
    }
    if !includes.is_empty() {
        let _ = write!(x, "  <symbols>\n{includes}  </symbols>\n");
    }
    let _ = write!(x, "  <timelines>\n{timelines}  </timelines>\n");
    if let Some(list) = &doc.swatches {
        x.push_str(&swatch_lists(list));
    }
    x.push_str("</DOMDocument>\n");
    pkg.insert("DOMDocument.xml".into(), x.into_bytes());
    pkg.insert("PublishSettings.xml".into(), b"<flash_profiles>\n  <flash_profile version=\"1.0\" name=\"Default\" current=\"true\"/>\n</flash_profiles>\n".to_vec());
    let warn = crate::read::dedup(w.warn);
    (pkg, warn)
}

/// The document's swatches as `swatchLists` (solid) and `extendedSwatchLists`
/// (gradients), one list each named "Default Swatches" as Animate writes them.
fn swatch_lists(list: &[ac_core::Swatch]) -> String {
    let head = "    <swatchList>\n      <swatchListInfo name=\"Default Swatches\"/>\n      <swatches>\n        <swatchListInfo name=\"Default Swatches\"/>\n";
    let tail = "      </swatches>\n    </swatchList>\n";
    let mut solid = String::new();
    let mut grads = String::new();
    for s in list {
        match &s.gradient {
            None => {
                let _ = writeln!(solid, "        <SolidSwatchItem color=\"{}\"{}/>", s.color.to_hex(), alpha_attr(s.color, "alpha"));
            }
            Some(g) => {
                let tag = if g.kind == GradientKind::Linear { "LinearGradientSwatchItem" } else { "RadialGradientSwatchItem" };
                let spread = match g.spread {
                    Spread::Pad => "",
                    Spread::Reflect => r#" spreadMethod="reflect""#,
                    Spread::Repeat => r#" spreadMethod="repeat""#,
                };
                let focal = if g.focal != 0.0 { format!(" focalPointRatio=\"{}\"", n(g.focal)) } else { String::new() };
                let _ = writeln!(grads, "        <{tag}{spread}{focal}>");
                for (r, c) in &g.stops {
                    let _ = writeln!(grads, "          <GradientEntry color=\"{}\"{} ratio=\"{}\"/>", c.to_hex(), alpha_attr(*c, "alpha"), n(*r));
                }
                let _ = writeln!(grads, "        </{tag}>");
            }
        }
    }
    let mut x = String::new();
    if !solid.is_empty() {
        let _ = write!(x, "  <swatchLists>\n{head}{solid}{tail}  </swatchLists>\n");
    }
    if !grads.is_empty() {
        let _ = write!(x, "  <extendedSwatchLists>\n{head}{grads}{tail}  </extendedSwatchLists>\n");
    }
    x
}
