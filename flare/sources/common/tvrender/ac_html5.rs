//! Publish ▸ HTML5 Canvas: a page that plays the movie in any browser.
//!
//! Every frame is evaluated with the same engine as the stage (tweens,
//! symbols, masks, camera, effects all resolved), and stored as a display
//! list over shared tables (paths, items, effect groups, masks), so static
//! content costs nothing per frame. A small runtime (`html5_runtime.js`)
//! draws the lists on a `<canvas>`, runs frame scripts with the same
//! CreateJS-style API as the editor (`this.stop()`, `this.ball.x`,
//! `addEventListener("click", …)`), drives buttons and plays sounds with
//! Animate's Event/Start/Stop/Stream rules.

use ac_core::fx::{BevelType, BlendMode, Filter};
use ac_core::model::*;
use ac_core::{Matrix, Path, Seg};
use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::Arc;

pub const RUNTIME: &str = include_str!("html5_runtime.js");

#[derive(Clone, Debug, Default)]
pub struct Html5Opts {
    /// One self-contained .html (images and sounds inlined).
    pub single_file: bool,
    pub loop_play: bool,
    /// Frame range (0-based inclusive) or the whole movie.
    pub range: Option<(u32, u32)>,
}

/// Published files: (relative path, bytes).
pub type Files = Vec<(String, Vec<u8>)>;

fn n(v: f32) -> String {
    if !v.is_finite() {
        return "0".into();
    }
    let r = (v * 100.0).round() / 100.0;
    if r == r.trunc() {
        format!("{}", r as i64)
    } else {
        let s = format!("{r:.2}");
        s.trim_end_matches('0').to_string()
    }
}
fn nm(m: &Matrix) -> String {
    let p = |v: f32| {
        let r = (v * 10000.0).round() / 10000.0;
        let s = format!("{r}");
        if s == "-0" { "0".into() } else { s }
    };
    format!("[{},{},{},{},{},{}]", p(m.a), p(m.b), p(m.c), p(m.d), n(m.tx), n(m.ty))
}
fn js_str(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '<' => o.push_str("\\u003c"),
            c if (c as u32) < 0x20 => {
                let _ = write!(o, "\\u{:04x}", c as u32);
            }
            c => o.push(c),
        }
    }
    o.push('"');
    o
}
fn svg_path(p: &Path) -> String {
    let mut s = String::new();
    for seg in &p.segs {
        match *seg {
            Seg::M(a) => {
                let _ = write!(s, "M{} {}", n(a.x), n(a.y));
            }
            Seg::L(a) => {
                let _ = write!(s, "L{} {}", n(a.x), n(a.y));
            }
            Seg::Q(c, a) => {
                let _ = write!(s, "Q{} {} {} {}", n(c.x), n(c.y), n(a.x), n(a.y));
            }
            Seg::C(c1, c2, a) => {
                let _ = write!(s, "C{} {} {} {} {} {}", n(c1.x), n(c1.y), n(c2.x), n(c2.y), n(a.x), n(a.y));
            }
            Seg::Z => s.push('Z'),
        }
    }
    s
}
fn rgba(c: Color, alpha: f32, tint: Option<(Color, f32)>, cx: Option<ColorXform>) -> String {
    let c = match tint {
        Some((t, amt)) => Color { a: c.a, ..c.lerp(Color { a: c.a, ..t }, amt.clamp(0.0, 1.0)) },
        None => c,
    };
    let c = cx.map(|x| x.apply(c)).unwrap_or(c);
    let a = c.a as f32 / 255.0 * alpha.clamp(0.0, 1.0);
    if a >= 0.999 {
        format!("\"#{:02x}{:02x}{:02x}\"", c.r, c.g, c.b)
    } else {
        format!("\"rgba({},{},{},{})\"", c.r, c.g, c.b, (a * 1000.0).round() / 1000.0)
    }
}

/// CSS filter + composite operation for an effect group.
fn fx_json(filters: &[Filter], blend: BlendMode) -> String {
    let css = |c: Color, strength: f32| format!("rgba({},{},{},{})", c.r, c.g, c.b, (c.a as f32 / 255.0 * (strength / 100.0).min(1.0) * 100.0).round() / 100.0);
    let mut f = vec![];
    let mut unsupported = false;
    // gradient filters run as per-pixel stages in the runtime (exact, like the
    // stage); CSS filters before/after them are grouped into their own stages
    let mut stages: Vec<String> = vec![];
    let mut any_ramp = false;
    for x in filters {
        if let Filter::GradientGlow { blur_x, blur_y, strength, quality, angle, distance, knockout, kind, stops } | Filter::GradientBevel { blur_x, blur_y, strength, quality, angle, distance, knockout, kind, stops } = x {
            if !f.is_empty() {
                stages.push(format!("{{\"c\":{}}}", js_str(&f.join(" "))));
                f.clear();
            }
            any_ramp = true;
            let k = if matches!(x, Filter::GradientGlow { .. }) { "glow" } else { "bevel" };
            let t = match kind {
                BevelType::Inner => 0,
                BevelType::Outer => 1,
                BevelType::Full => 2,
            };
            let r: Vec<String> = stops.iter().map(|(p, c)| format!("[{},{},{},{},{}]", n(*p), c.r, c.g, c.b, c.a)).collect();
            stages.push(format!(
                "{{\"g\":{{\"k\":\"{k}\",\"bx\":{},\"by\":{},\"s\":{},\"q\":{},\"a\":{},\"d\":{},\"ko\":{},\"t\":{t},\"r\":[{}]}}}}",
                n(*blur_x),
                n(*blur_y),
                n(strength / 100.0),
                quality,
                n(*angle),
                n(*distance),
                knockout,
                r.join(",")
            ));
            continue;
        }
        match x {
            Filter::Blur { blur_x, blur_y, .. } => f.push(format!("blur({}px)", n((blur_x + blur_y) / 4.0))),
            Filter::DropShadow { blur_x, blur_y, strength, angle, distance, color, inner, knockout, hide_object, .. } => {
                unsupported |= *inner || *knockout || *hide_object;
                let (dx, dy) = (angle.to_radians().cos() * distance, angle.to_radians().sin() * distance);
                f.push(format!("drop-shadow({}px {}px {}px {})", n(dx), n(dy), n((blur_x + blur_y) / 4.0), css(*color, *strength)));
            }
            Filter::Glow { blur_x, blur_y, strength, color, inner, knockout, .. } => {
                unsupported |= *inner || *knockout;
                let k = ((strength / 100.0).ceil() as usize).clamp(1, 3);
                for _ in 0..k {
                    f.push(format!("drop-shadow(0 0 {}px {})", n((blur_x + blur_y) / 4.0), css(*color, 100.0)));
                }
            }
            Filter::Bevel { blur_x, blur_y, strength, shadow, highlight, angle, distance, kind, .. } => {
                unsupported |= *kind != BevelType::Outer;
                let (dx, dy) = (angle.to_radians().cos() * distance, angle.to_radians().sin() * distance);
                let b = n((blur_x + blur_y) / 4.0);
                f.push(format!("drop-shadow({}px {}px {b}px {})", n(-dx), n(-dy), css(*highlight, *strength)));
                f.push(format!("drop-shadow({}px {}px {b}px {})", n(dx), n(dy), css(*shadow, *strength)));
            }
            Filter::GradientGlow { .. } | Filter::GradientBevel { .. } => {} // handled above
            Filter::MotionBlur { .. } => {} // per group occurrence ("mb"), see Tables::list
            // EffectCraft effects run in Rust only; the published page shows the object without them.
            Filter::Effect { .. } => unsupported = true,
            Filter::AdjustColor { brightness, contrast, saturation, hue } => {
                f.push(format!("brightness({}) contrast({}) saturate({}) hue-rotate({}deg)", n(1.0 + brightness / 100.0), n(1.0 + contrast / 100.0), n(1.0 + saturation / 100.0), n(*hue)));
            }
        }
    }
    let op = match blend {
        BlendMode::Normal | BlendMode::Layer => "source-over",
        BlendMode::Darken => "darken",
        BlendMode::Multiply => "multiply",
        BlendMode::Lighten => "lighten",
        BlendMode::Screen => "screen",
        BlendMode::Overlay => "overlay",
        BlendMode::HardLight => "hard-light",
        BlendMode::Add => "lighter",
        BlendMode::Subtract => "difference",
        BlendMode::Difference => "difference",
        BlendMode::Invert => "exclusion",
        BlendMode::Alpha => "destination-in",
        BlendMode::Erase => "destination-out",
    };
    let _ = unsupported;
    if any_ramp {
        if !f.is_empty() {
            stages.push(format!("{{\"c\":{}}}", js_str(&f.join(" "))));
        }
        return format!("{{\"f\":\"\",\"s\":[{}],\"o\":\"{op}\"}}", stages.join(","));
    }
    format!("{{\"f\":{},\"o\":\"{op}\"}}", js_str(&f.join(" ")))
}

struct Tables {
    paths: Vec<String>,
    path_ix: HashMap<String, usize>,
    items: Vec<String>,
    item_ix: HashMap<String, usize>,
    fx: Vec<String>,
    fx_ix: HashMap<String, usize>,
    clips: Vec<String>,
    clip_ix: HashMap<String, usize>,
    images: Vec<(String, Vec<u8>, bool)>,
    image_ix: HashMap<usize, usize>,
}

fn intern(v: &mut Vec<String>, ix: &mut HashMap<String, usize>, s: String) -> usize {
    if let Some(&i) = ix.get(&s) {
        return i;
    }
    v.push(s.clone());
    ix.insert(s, v.len() - 1);
    v.len() - 1
}

impl Tables {
    fn path(&mut self, p: &Path) -> usize {
        intern(&mut self.paths, &mut self.path_ix, svg_path(p))
    }
    fn image(&mut self, img: &Arc<ac_core::bitmap::Image>) -> usize {
        let key = Arc::as_ptr(img) as usize;
        if let Some(&i) = self.image_ix.get(&key) {
            return i;
        }
        // re-encode the decoded (premultiplied) pixels as PNG
        let mut rgba = Vec::with_capacity(img.rgba.len());
        for px in img.rgba.chunks_exact(4) {
            let a = px[3] as u32;
            let un = |c: u8| (c as u32 * 255 + a / 2).checked_div(a).unwrap_or(0).min(255) as u8;
            rgba.extend_from_slice(&[un(px[0]), un(px[1]), un(px[2]), px[3]]);
        }
        let mut png_bytes = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut png_bytes, img.width, img.height);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            if let Ok(mut w) = enc.write_header() {
                let _ = w.write_image_data(&rgba);
            }
        }
        let name = format!("images/img{}.png", self.images.len() + 1);
        self.images.push((name, png_bytes, img.smoothing));
        self.image_ix.insert(key, self.images.len() - 1);
        self.images.len() - 1
    }
    fn clip(&mut self, c: &ac_core::Clip) -> usize {
        let parts: Vec<String> = c.shapes.iter().map(|(p, m)| format!("[{},{}]", self.path(p), nm(m))).collect();
        intern(&mut self.clips, &mut self.clip_ix, format!("[{}]", parts.join(",")))
    }
    fn item(&mut self, it: &DrawItem) -> usize {
        let mut s = format!("{{\"p\":{},\"m\":{}", self.path(&it.path), nm(&it.matrix));
        let a = it.alpha;
        if let Some(oc) = it.outline {
            let _ = write!(s, ",\"ol\":{}", rgba(oc, a, None, None));
        } else if let Some(img) = &it.image {
            let _ = write!(s, ",\"img\":{}", self.image(img));
            if a < 0.999 {
                let _ = write!(s, ",\"a\":{}", n(a));
            }
        } else if let Some((img, pm)) = &it.pattern {
            let _ = write!(s, ",\"pat\":{},\"pm\":{}", self.image(img), nm(pm));
            if a < 0.999 {
                let _ = write!(s, ",\"a\":{}", n(a));
            }
        } else if let Some(g) = &it.gradient {
            // canvas gradients only extend: repeat/reflect are unrolled over a wide range
            let (stops, extent) = match g.spread {
                Spread::Pad => (g.stops.iter().map(|(t, c)| format!("[{},{}]", n(*t), rgba(*c, a, it.tint, it.cx))).collect::<Vec<_>>(), 1.0),
                spread => {
                    let periods: i32 = 12;
                    let linear = g.kind == GradientKind::Linear;
                    let extent = if linear { 1.0 + 2.0 * periods as f32 } else { 1.0 + periods as f32 };
                    let mut v = vec![];
                    let ks: Vec<i32> = if linear { (-periods..=periods).collect() } else { (0..=periods).collect() };
                    for k in ks {
                        let flip = spread == Spread::Reflect && k.rem_euclid(2) == 1;
                        let mut st: Vec<(f32, Color)> = g.stops.iter().map(|(t, c)| (if flip { 1.0 - t } else { *t }, *c)).collect();
                        st.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap());
                        for (t, c) in st {
                            let x = if linear { -1.0 + 2.0 * k as f32 + 2.0 * t } else { k as f32 + t };
                            let u = if linear { (x + extent) / (2.0 * extent) } else { x / extent };
                            v.push(format!("[{},{}]", (u.clamp(0.0, 1.0) * 1e5).round() / 1e5, rgba(c, a, it.tint, it.cx)));
                        }
                    }
                    (v, extent)
                }
            };
            let mut ext = if extent != 1.0 { format!(",\"x\":{}", n(extent)) } else { String::new() };
            if g.kind == GradientKind::Radial && g.focal != 0.0 {
                let _ = write!(ext, ",\"fx\":{}", n(g.focal.clamp(-0.99, 0.99)));
            }
            let _ = write!(s, ",\"g\":{{\"k\":{},\"m\":{},\"s\":[{}]{ext}}}", if g.kind == GradientKind::Linear { 0 } else { 1 }, nm(&g.matrix), stops.join(","));
        } else if let Some(f) = it.fill {
            let _ = write!(s, ",\"f\":{}", rgba(f, a, it.tint, it.cx));
        }
        if it.outline.is_none() {
            if let Some(st) = it.stroke {
                let _ = write!(s, ",\"s\":{},\"w\":{}", rgba(st.color, a, it.tint, it.cx), n(st.width.max(0.1)));
                let ss = &it.stroke_style;
                let cap = match (ss.dash, ss.cap) {
                    (Dash::Dotted, _) | (_, Cap::Round) => "round",
                    (_, Cap::None) => "butt",
                    (_, Cap::Square) => "square",
                };
                let join = match ss.join {
                    Join::Miter => "miter",
                    Join::Round => "round",
                    Join::Bevel => "bevel",
                };
                if cap != "round" || join != "round" {
                    let _ = write!(s, ",\"cj\":\"{cap} {join}\"");
                }
                let w = st.width.max(0.1);
                match ss.dash {
                    Dash::Dashed => {
                        let _ = write!(s, ",\"d\":[{},{}]", n((ss.dash_len * w).max(0.5)), n((ss.gap * w).max(0.5)));
                    }
                    Dash::Dotted => {
                        let _ = write!(s, ",\"d\":[0.01,{}]", n((ss.gap * w).max(w * 1.2)));
                    }
                    _ => {}
                }
            }
        }
        if !it.clips.is_empty() {
            let c: Vec<String> = it.clips.iter().map(|c| self.clip(c).to_string()).collect();
            let _ = write!(s, ",\"c\":[{}]", c.join(","));
        }
        if let Some((l, e)) = it.src {
            let _ = write!(s, ",\"src\":\"{l}:{e}\"");
        }
        s.push('}');
        intern(&mut self.items, &mut self.item_ix, s)
    }

    /// A frame's items as a display-list tree: item indices, and effect
    /// groups `{"x": fx, "c": [...]}` drawn through an offscreen canvas.
    fn list(&mut self, items: &[DrawItem], level: usize) -> String {
        let mut out = vec![];
        let mut i = 0;
        while i < items.len() {
            let Some(g) = items[i].fx.get(level).cloned() else {
                out.push(self.item(&items[i]).to_string());
                i += 1;
                continue;
            };
            let mut j = i + 1;
            while j < items.len() && items[j].fx.get(level).is_some_and(|h| Arc::ptr_eq(h, &g)) {
                j += 1;
            }
            let fx = intern(&mut self.fx, &mut self.fx_ix, fx_json(&g.filters, g.blend));
            let inner = self.list(&items[i..j], level + 1);
            let mb = g
                .motion
                .zip(g.filters.iter().find_map(|f| match f {
                    Filter::MotionBlur { shutter, samples } if *shutter > 0.0 => Some(((shutter / 360.0).min(2.0), (*samples).clamp(2, 64))),
                    _ => None,
                }))
                .map(|(m, (f, count))| format!(",\"mb\":{{\"m\":[{},{},{},{},{},{}],\"f\":{},\"n\":{count}}}", n(m.a), n(m.b), n(m.c), n(m.d), n(m.tx), n(m.ty), n(f)))
                .unwrap_or_default();
            // Asset Warp on a bitmap / symbol: the group's grid, before and after (stage space)
            let mw = g
                .mesh
                .as_ref()
                .map(|m| {
                    let flat = |v: &[ac_core::geom::Pt]| v.iter().map(|p| format!("{},{}", n(p.x), n(p.y))).collect::<Vec<_>>().join(",");
                    format!(",\"mw\":{{\"c\":{},\"r\":{},\"s\":[{}],\"d\":[{}]}}", m.cols, m.rows, flat(&m.src), flat(&m.dst))
                })
                .unwrap_or_default();
            out.push(format!("{{\"x\":{fx},\"c\":{inner}{mb}{mw}}}"));
            i = j;
        }
        format!("[{}]", out.join(","))
    }
}

fn b64(data: &[u8]) -> String {
    crate::base64_lite::encode(data)
}

fn mime_of(data: &[u8]) -> &'static str {
    if data.starts_with(b"RIFF") {
        "audio/wav"
    } else if data.starts_with(b"OggS") {
        "audio/ogg"
    } else if data.starts_with(b"fLaC") {
        "audio/flac"
    } else if data.starts_with(b"FORM") {
        "audio/aiff"
    } else {
        "audio/mpeg"
    }
}

/// A text field's runtime description: kind, text and how to draw it again
/// when a script changes it (dynamic), as an HTML input (input), its link.
fn text_json(t: &ac_core::TextEl) -> String {
    use ac_core::text::{Align, TextKind};
    let k = match t.field.kind {
        TextKind::Static => 0,
        TextKind::Dynamic => 1,
        TextKind::Input => 2,
    };
    let family = match &t.family {
        Some(f) => format!("\"{}\", sans-serif", f.replace('"', "")),
        None if t.font == ac_core::text::FontFace::Mono => "Hack, Consolas, monospace".into(),
        None => "Ubuntu, \"Segoe UI\", sans-serif".into(),
    };
    let font = format!("{}{} {}px {}", if t.italic { "italic " } else { "" }, t.weight, n(t.size), family);
    let al = match t.align {
        Align::Left => "left",
        Align::Center => "center",
        Align::Right => "right",
        Align::Justify => "justify",
    };
    let (bw, bh) = t.box_size();
    let c = t.color;
    let mut s = format!(
        ",\"tx\":{{\"k\":{k},\"s\":{},\"f\":{},\"sz\":{},\"c\":\"rgba({},{},{},{})\",\"al\":\"{al}\",\"w\":{},\"bw\":{},\"bh\":{},\"lh\":{},\"ls\":{},\"ml\":{},\"in\":{}",
        js_str(&t.text),
        js_str(&font),
        n(t.size),
        c.r,
        c.g,
        c.b,
        n(c.a as f32 / 255.0),
        n(t.para.width.unwrap_or(0.0)),
        n(bw),
        n(bh),
        n(t.size * 1.2 + t.para.line_spacing),
        n(t.letter_spacing),
        n(t.para.margin_left),
        n(t.para.indent)
    );
    if t.field.password {
        s.push_str(",\"pw\":1");
    }
    if t.field.max_chars > 0 {
        let _ = write!(s, ",\"mc\":{}", t.field.max_chars);
    }
    if t.field.border {
        s.push_str(",\"bd\":1");
    }
    if let Some(u) = &t.field.link {
        let _ = write!(s, ",\"url\":{}", js_str(u));
    }
    s.push('}');
    s
}

/// Build the published files.
pub fn publish(doc: &Document, o: &Html5Opts) -> Files {
    let total = doc.total_frames();
    let (f0, f1) = o.range.unwrap_or((0, total - 1));
    let (f0, f1) = (f0.min(total - 1), f1.min(total - 1).max(f0.min(total - 1)));
    let mut t = Tables { paths: vec![], path_ix: HashMap::new(), items: vec![], item_ix: HashMap::new(), fx: vec![], fx_ix: HashMap::new(), clips: vec![], clip_ix: HashMap::new(), images: vec![], image_ix: HashMap::new() };
    let mut frames = vec![];
    let mut extra = vec![]; // per frame: buttons + named instances + scripts + sounds
    // A script can hold the main timeline while movie clips keep playing:
    // frames that can be held store one list per movie-clip clock phase.
    let can_hold = doc.scenes().chain(doc.library.iter().map(|s| &s.timeline)).flat_map(|t| &t.layers).flat_map(|l| l.keyframes.values()).any(|k| k.script.as_deref().is_some_and(|s| s.contains("stop") || s.contains("gotoAndStop")));
    let mut variant_budget: usize = 20_000;
    fn gcd(a: u32, b: u32) -> u32 {
        if b == 0 { a } else { gcd(b, a % b) }
    }
    for f in f0..=f1 {
        let items = evaluate_scene(doc, f);
        frames.push(t.list(&items, 0));
        let mut variants = String::new();
        if can_hold {
            let lens = doc.movie_clip_lengths(f);
            let period = lens.iter().fold(1u32, |a, &b| (a / gcd(a, b)).saturating_mul(b));
            if period > 1 && period <= 240 && (period as usize) <= variant_budget {
                variant_budget -= period as usize;
                let lists: Vec<String> = (0..period).map(|p| t.list(&evaluate_scene_clock(doc, f, p), 0)).collect();
                variants = format!("\"v\":[{}]", lists.join(","));
            }
        }
        let (si, local) = doc.locate_frame(f);
        let scene = doc.scene_at(si).unwrap_or(&doc.scene);
        let view = camera_view(doc, scene, local);
        let mut parts = vec![];
        if !variants.is_empty() {
            parts.push(variants);
        }
        // buttons: Over / Down states and the hit area
        let mut buttons = vec![];
        let mut named = vec![];
        for (li, layer) in scene.layers.iter().enumerate() {
            if layer.kind != LayerKind::Normal || !layer_shown(scene, li) {
                continue;
            }
            let Some((els, _)) = layer_elements_at(layer, local) else { continue };
            for (ei, el) in els.iter().enumerate() {
                // text with a link but no instance name still needs an entry (for its click)
                let link_name = match &el.kind {
                    ElementKind::Text(t) if el.name.is_empty() && t.field.link.is_some() => Some(format!("__link{li}_{ei}")),
                    _ => None,
                };
                let name = link_name.clone().unwrap_or_else(|| el.name.clone());
                if !name.is_empty() && !matches!(el.kind, ElementKind::Shape(_) | ElementKind::Canvas(_)) && !named.iter().any(|(nn, _): &(String, String)| *nn == name) {
                    let d = el.matrix.decompose();
                    let tx = match &el.kind {
                        ElementKind::Text(t) => text_json(t),
                        _ => String::new(),
                    };
                    named.push((name, format!("{{\"src\":\"{li}:{ei}\",\"x\":{},\"y\":{},\"r\":{},\"sx\":{},\"sy\":{},\"a\":{},\"m\":{}{tx}}}", n(el.matrix.tx), n(el.matrix.ty), n(d.skew_y.to_degrees()), n(d.scale_x), n(d.scale_y), n(el.alpha), nm(&el.matrix))));
                }
                let ElementKind::Instance(inst) = &el.kind else { continue };
                let Some(sym) = doc.symbol(inst.symbol) else { continue };
                if sym.kind != SymbolKind::Button {
                    continue;
                }
                let state = |s: u32| {
                    let ctx = EvalCtx { button: Some(((li, ei), s)), ..EvalCtx::new(doc, f) };
                    let mut out = vec![];
                    evaluate(&ctx, scene, local, &Matrix::IDENTITY, 1.0, 0, &mut out);
                    if let Some(v) = &view {
                        apply_view(&mut out, v);
                    }
                    out.retain(|it| it.src == Some((li, ei)));
                    out
                };
                let over = state(1);
                let down = state(2);
                let dur = sym.timeline.frame_count();
                let m = view.map(|v| v.concat(&el.matrix)).unwrap_or(el.matrix);
                let mut hit = vec![];
                let ctx = EvalCtx::new(doc, f);
                evaluate(&ctx, &sym.timeline, if dur >= 4 { 3 } else { 0 }, &m, 1.0, 1, &mut hit);
                let hits: Vec<String> = hit.iter().map(|h| format!("[{},{}]", t.path(&h.path), nm(&h.matrix))).collect();
                let ov = t.list(&over, 0);
                let dn = t.list(&down, 0);
                buttons.push(format!("{{\"src\":\"{li}:{ei}\",\"name\":{},\"over\":{ov},\"down\":{dn},\"hit\":[{}]}}", js_str(&el.name), hits.join(",")));
            }
        }
        if !buttons.is_empty() {
            parts.push(format!("\"b\":[{}]", buttons.join(",")));
        }
        if !named.is_empty() {
            let v: Vec<String> = named.iter().map(|(k, v)| format!("{}:{v}", js_str(k))).collect();
            parts.push(format!("\"n\":{{{}}}", v.join(",")));
        }
        // keyframe scripts and sounds that start here
        let mut scripts = vec![];
        let sounds: Vec<String> = ac_core::sound::frame_sounds(doc, f)
            .into_iter()
            .filter_map(|cue| {
                let fs = cue.sound;
                let ix = doc.sounds.iter().position(|s| s.id == fs.sound)?;
                let fx = if fs.effect == ac_core::sound::SoundEffect::None { String::new() } else { format!(",\"fx\":\"{:?}\"", fs.effect) };
                Some(format!("{{\"s\":{ix},\"y\":\"{}\",\"r\":{},\"l\":{},\"e\":{}{fx}}}", fs.sync.name().to_lowercase(), fs.repeat.max(1), fs.forever, cue.stream_end.saturating_sub(f0)))
            })
            .collect();
        for l in &scene.layers {
            if let Some(k) = l.keyframes.get(&local) {
                if let Some(sc) = k.script.as_ref().filter(|s| !s.trim().is_empty()) {
                    scripts.push(js_str(sc));
                }
            }
        }
        if !scripts.is_empty() {
            parts.push(format!("\"js\":[{}]", scripts.join(",")));
        }
        if !sounds.is_empty() {
            parts.push(format!("\"snd\":[{}]", sounds.join(",")));
        }
        extra.push(if parts.is_empty() { "0".to_string() } else { format!("{{{}}}", parts.join(",")) });
    }
    // scenes (for currentFrame / gotoAndPlay inside a scene) and labels
    let mut scenes = vec![];
    let mut labels = vec![];
    for (i, sc) in doc.scenes().enumerate() {
        let st = doc.scene_start(i) as i64 - f0 as i64;
        scenes.push(format!("{{\"name\":{},\"start\":{st},\"count\":{}}}", js_str(&sc.name), sc.frame_count()));
        for l in &sc.layers {
            for (fr, k) in &l.keyframes {
                if let Some(lb) = k.target_label() {
                    labels.push(format!("[{},{},{}]", js_str(lb), i, fr));
                }
            }
        }
    }
    let images: Vec<String> = t
        .images
        .iter()
        .map(|(name, bytes, smooth)| {
            let src = if o.single_file { format!("data:image/png;base64,{}", b64(bytes)) } else { name.clone() };
            format!("[{},{}]", js_str(&src), smooth)
        })
        .collect();
    let sounds: Vec<String> = doc.sounds.iter().map(|s| format!("[{},{}]", js_str(&s.name), js_str(&format!("data:{};base64,{}", mime_of(&s.data), b64(&s.data))))).collect();
    let bg = doc.background;
    let mut data = String::with_capacity(1 << 16);
    let _ = write!(
        data,
        "var AC_MOVIE={{\"w\":{},\"h\":{},\"fps\":{},\"bg\":\"#{:02x}{:02x}{:02x}\",\"loop\":{},\"scenes\":[{}],\"labels\":[{}],\"paths\":[{}],\"items\":[{}],\"fx\":[{}],\"clips\":[{}],\"images\":[{}],\"sounds\":[{}],\"frames\":[{}],\"extra\":[{}]}};",
        n(doc.width),
        n(doc.height),
        n(doc.fps),
        bg.r,
        bg.g,
        bg.b,
        o.loop_play,
        scenes.join(","),
        labels.join(","),
        t.paths.iter().map(|p| js_str(p)).collect::<Vec<_>>().join(","),
        t.items.join(","),
        t.fx.join(","),
        t.clips.join(","),
        images.join(","),
        sounds.join(","),
        frames.join(",\n"),
        extra.join(",")
    );
    data.push('\n');
    let title = "Flare movie";
    let mut files: Files = vec![];
    let html = |scripts: &str| {
        format!(
            "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<meta name=\"generator\" content=\"Flare\">\n<title>{title}</title>\n<style>html,body{{margin:0;height:100%;background:#1e1e1e;display:flex;align-items:center;justify-content:center;overflow:hidden}}canvas{{display:block;max-width:100vw;max-height:100vh;width:auto;height:auto;image-rendering:auto}}</style>\n</head>\n<body>\n<canvas id=\"ac-stage\" width=\"{}\" height=\"{}\"></canvas>\n{scripts}</body>\n</html>\n",
            doc.width.round() as u32,
            doc.height.round() as u32
        )
    };
    if o.single_file {
        files.push(("index.html".into(), html(&format!("<script>\n{data}</script>\n<script>\n{RUNTIME}</script>\n")).into_bytes()));
    } else {
        files.push(("index.html".into(), html("<script src=\"movie.js\"></script>\n<script src=\"flare-runtime.js\"></script>\n").into_bytes()));
        files.push(("movie.js".into(), data.into_bytes()));
        files.push(("flare-runtime.js".into(), RUNTIME.as_bytes().to_vec()));
        for (name, bytes, _) in t.images {
            files.push((name, bytes));
        }
    }
    files
}

/// Publish to a folder (index.html + movie.js + runtime + images/) or, for
/// a path ending in .html, one self-contained page.
pub fn publish_to(doc: &Document, o: &Html5Opts, path: &std::path::Path) -> Result<std::path::PathBuf, String> {
    let single = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("html") || e.eq_ignore_ascii_case("htm"));
    let o = Html5Opts { single_file: single || o.single_file, ..o.clone() };
    let files = publish(doc, &o);
    if single {
        std::fs::write(path, &files[0].1).map_err(|e| format!("{}: {e}", path.display()))?;
        return Ok(path.to_path_buf());
    }
    for (name, bytes) in files {
        let p = path.join(&name);
        if let Some(d) = p.parent() {
            std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
        }
        std::fs::write(&p, bytes).map_err(|e| format!("{}: {e}", p.display()))?;
    }
    Ok(path.join("index.html"))
}
