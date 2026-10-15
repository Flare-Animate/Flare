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

use ac_core::fx::{BevelType, BlendMode, Filter, FxGroup};
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

/// Solid colour as a two-stop ramp: the effect's intensity (0…1) is the alpha
/// (`c.a` scaled by it), exactly how ac-render's `combine` paints a shadow.
fn solid_ramp(c: Color) -> String {
    format!("[[0,{},{},{},0],[1,{},{},{},{}]]", c.r, c.g, c.b, c.r, c.g, c.b, c.a)
}

fn quality(q: u8) -> u8 {
    q.clamp(1, 3)
}

/// How a group's filters reach the browser, in order:
///
/// * `Blur` and a plain outer `DropShadow` use canvas `ctx.filter`
///   (`blur()`, `drop-shadow()`). The browser blurs with a Gaussian where
///   ac-render uses box passes, so edges differ slightly (sigma = (x+y)/4).
/// * `Glow`, `DropShadow` with inner / knockout / hide-object / strength over
///   100, `Bevel`, `GradientGlow`, `GradientBevel` and `AdjustColor` run as
///   per-pixel stages in `html5_runtime.js` (`rampFilter`, `adjustColor`) on an
///   offscreen canvas: a separable box blur of the alpha plane, shifted by
///   (cos a, sin a) * distance, scaled by strength/100 and composited under /
///   over / instead of the object. These are ports of ac-render's formulas.
/// * `MotionBlur` is resolved per occurrence (`"mb"`, see `Tables::list`).
/// * `Effect` (EffectCraft) has no canvas equivalent: `Tables::bake` renders it
///   at publish time and the group becomes a sprite (`"sp"`), so no filter
///   reaches this function for it.
///
/// Things the page can only approximate are pushed on `warn`.
fn fx_json(filters: &[Filter], blend: BlendMode, warn: &mut Vec<String>) -> String {
    let css = |c: Color, strength: f32| format!("rgba({},{},{},{})", c.r, c.g, c.b, (c.a as f32 / 255.0 * (strength / 100.0).min(1.0) * 100.0).round() / 100.0);
    let mut f: Vec<String> = vec![];
    // per-pixel stages; CSS filters before / after them are grouped into their own stages
    let mut stages: Vec<String> = vec![];
    fn flush(f: &mut Vec<String>, stages: &mut Vec<String>) {
        if !f.is_empty() {
            stages.push(format!("{{\"c\":{}}}", js_str(&f.join(" "))));
            f.clear();
        }
    }
    let kind_t = |k: &BevelType| match k {
        BevelType::Inner => 0,
        BevelType::Outer => 1,
        BevelType::Full => 2,
    };
    for x in filters {
        match x {
            Filter::GradientGlow { blur_x, blur_y, strength, quality: q, angle, distance, knockout, kind, stops } | Filter::GradientBevel { blur_x, blur_y, strength, quality: q, angle, distance, knockout, kind, stops } => {
                flush(&mut f, &mut stages);
                let k = if matches!(x, Filter::GradientGlow { .. }) { "glow" } else { "bevel" };
                let r: Vec<String> = stops.iter().map(|(p, c)| format!("[{},{},{},{},{}]", n(*p), c.r, c.g, c.b, c.a)).collect();
                stages.push(format!(
                    "{{\"g\":{{\"k\":\"{k}\",\"bx\":{},\"by\":{},\"s\":{},\"q\":{},\"a\":{},\"d\":{},\"ko\":{},\"t\":{},\"r\":[{}]}}}}",
                    n(*blur_x),
                    n(*blur_y),
                    n(strength / 100.0),
                    quality(*q),
                    n(*angle),
                    n(*distance),
                    knockout,
                    kind_t(kind),
                    r.join(",")
                ));
            }
            Filter::Blur { blur_x, blur_y, .. } => f.push(format!("blur({}px)", n((blur_x + blur_y) / 4.0))),
            Filter::DropShadow { blur_x, blur_y, strength, quality: q, angle, distance, color, inner, knockout, hide_object } => {
                if !*inner && !*knockout && !*hide_object && *strength <= 100.0 {
                    let (dx, dy) = (angle.to_radians().cos() * distance, angle.to_radians().sin() * distance);
                    f.push(format!("drop-shadow({}px {}px {}px {})", n(dx), n(dy), n((blur_x + blur_y) / 4.0), css(*color, *strength)));
                } else {
                    flush(&mut f, &mut stages);
                    let hide = *hide_object && !*inner;
                    stages.push(format!(
                        "{{\"g\":{{\"k\":\"glow\",\"bx\":{},\"by\":{},\"s\":{},\"q\":{},\"a\":{},\"d\":{},\"ko\":{},\"h\":{hide},\"nm\":true,\"t\":{},\"r\":{}}}}}",
                        n(*blur_x),
                        n(*blur_y),
                        n(strength / 100.0),
                        quality(*q),
                        n(*angle),
                        n(*distance),
                        knockout,
                        if *inner { 0 } else { 1 },
                        solid_ramp(*color)
                    ));
                }
            }
            Filter::Glow { blur_x, blur_y, strength, quality: q, color, inner, knockout } => {
                flush(&mut f, &mut stages);
                stages.push(format!(
                    "{{\"g\":{{\"k\":\"glow\",\"bx\":{},\"by\":{},\"s\":{},\"q\":{},\"a\":0,\"d\":0,\"ko\":{},\"h\":false,\"nm\":true,\"t\":{},\"r\":{}}}}}",
                    n(*blur_x),
                    n(*blur_y),
                    n(strength / 100.0),
                    quality(*q),
                    knockout,
                    if *inner { 0 } else { 1 },
                    solid_ramp(*color)
                ));
            }
            Filter::Bevel { blur_x, blur_y, strength, quality: q, shadow, highlight, angle, distance, knockout, kind } => {
                flush(&mut f, &mut stages);
                stages.push(format!(
                    "{{\"g\":{{\"k\":\"sbevel\",\"bx\":{},\"by\":{},\"s\":{},\"q\":{},\"a\":{},\"d\":{},\"ko\":{},\"t\":{},\"sh\":{},\"hl\":{}}}}}",
                    n(*blur_x),
                    n(*blur_y),
                    n(strength / 100.0),
                    quality(*q),
                    n(*angle),
                    n(*distance),
                    knockout,
                    kind_t(kind),
                    solid_ramp(*shadow),
                    solid_ramp(*highlight)
                ));
            }
            Filter::AdjustColor { brightness, contrast, saturation, hue } => {
                flush(&mut f, &mut stages);
                stages.push(format!("{{\"a\":[{},{},{},{}]}}", n(*brightness), n(*contrast), n(*saturation), n(*hue)));
            }
            Filter::MotionBlur { .. } => {} // per group occurrence ("mb"), see Tables::list
            // baked into a sprite before we get here (Tables::bake); never reaches the page as a filter
            Filter::Effect { .. } => {}
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
        BlendMode::Subtract | BlendMode::Difference => "difference",
        BlendMode::Invert => "exclusion",
        BlendMode::Alpha => "destination-in",
        BlendMode::Erase => "destination-out",
    };
    // canvas has no Subtract / Invert composite operation: closest ones used
    let note = match blend {
        BlendMode::Subtract => Some("the Subtract blend mode is drawn as Difference (canvas has no subtract operation)"),
        BlendMode::Invert => Some("the Invert blend mode is drawn as Exclusion (canvas has no invert operation)"),
        _ => None,
    };
    if let Some(w) = note {
        if !warn.iter().any(|x| x == w) {
            warn.push(w.to_string());
        }
    }
    if !stages.is_empty() {
        flush(&mut f, &mut stages);
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
    /// Stage size in pixels (baked effect sprites cover the whole stage).
    w: u32,
    h: u32,
    /// Baked sprites already stored, by content hash → index into `images`.
    baked_ix: HashMap<u64, usize>,
    /// Things the page can't show exactly, for the publish report.
    warnings: Vec<String>,
}

fn intern(v: &mut Vec<String>, ix: &mut HashMap<String, usize>, s: String) -> usize {
    if let Some(&i) = ix.get(&s) {
        return i;
    }
    v.push(s.clone());
    ix.insert(s, v.len() - 1);
    v.len() - 1
}

/// PNG bytes of premultiplied RGBA pixels.
fn png_of_premultiplied(width: u32, height: u32, premul: &[u8]) -> Vec<u8> {
    let mut rgba = Vec::with_capacity(premul.len());
    for px in premul.chunks_exact(4) {
        let a = px[3] as u32;
        let un = |c: u8| (c as u32 * 255 + a / 2).checked_div(a).unwrap_or(0).min(255) as u8;
        rgba.extend_from_slice(&[un(px[0]), un(px[1]), un(px[2]), px[3]]);
    }
    let mut png_bytes = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut png_bytes, width, height);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        if let Ok(mut w) = enc.write_header() {
            let _ = w.write_image_data(&rgba);
        }
    }
    png_bytes
}

impl Tables {
    fn warn(&mut self, w: String) {
        if !self.warnings.contains(&w) {
            self.warnings.push(w);
        }
    }
    /// Render an effect group that has EffectCraft effects through ac-render's
    /// rasteriser (the same code the stage uses: its whole filter chain, motion
    /// blur and warp included, blend left to the page) into a stage-sized PNG
    /// sprite. Identical sprites across frames are stored once. None when it
    /// can't be done (reported in the warnings).
    fn bake(&mut self, items: &[DrawItem], level: usize, g: &FxGroup) -> Option<usize> {
        for f in &g.filters {
            if let Filter::Effect { id, .. } = f {
                if crate::effectcraft::info(id).is_none() {
                    self.warn(format!("EffectCraft effect \"{id}\" is not in the effect catalog: the page shows the object without it (the stage does too)"));
                }
            }
        }
        if self.w == 0 || self.h == 0 || self.w > 8192 || self.h > 8192 {
            self.warn(format!("EffectCraft effects not baked: a {}×{} px stage is outside the 1–8192 px range; the page shows the objects without them", self.w, self.h));
            return None;
        }
        let Some(mut pm) = tiny_skia::Pixmap::new(self.w, self.h) else {
            self.warn("EffectCraft effects not baked: couldn't allocate the sprite; the page shows the objects without them".into());
            return None;
        };
        let mut baked = g.clone();
        baked.blend = BlendMode::Normal;
        crate::fx::draw_group(&mut pm.as_mut(), items, level, &baked, &crate::RenderOpts::default(), &mut crate::fx::MaskCache::default());
        let bytes = png_of_premultiplied(self.w, self.h, pm.data());
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        bytes.hash(&mut h);
        let key = h.finish();
        if let Some(&i) = self.baked_ix.get(&key) {
            if self.images[i].1 == bytes {
                return Some(i);
            }
        }
        let name = format!("images/fx{}.png", self.images.len() + 1);
        self.images.push((name, bytes, true));
        self.baked_ix.insert(key, self.images.len() - 1);
        Some(self.images.len() - 1)
    }
    fn path(&mut self, p: &Path) -> usize {
        intern(&mut self.paths, &mut self.path_ix, svg_path(p))
    }
    fn image(&mut self, img: &Arc<ac_core::bitmap::Image>) -> usize {
        let key = Arc::as_ptr(img) as usize;
        if let Some(&i) = self.image_ix.get(&key) {
            return i;
        }
        // re-encode the decoded (premultiplied) pixels as PNG
        let png_bytes = png_of_premultiplied(img.width, img.height, &img.rgba);
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
            if g.filters.iter().any(|f| matches!(f, Filter::Effect { .. })) {
                if let Some(sp) = self.bake(&items[i..j], level, &g) {
                    let mut w = std::mem::take(&mut self.warnings);
                    let fx = intern(&mut self.fx, &mut self.fx_ix, fx_json(&[], g.blend, &mut w));
                    self.warnings = w;
                    out.push(format!("{{\"x\":{fx},\"sp\":{sp}}}"));
                    i = j;
                    continue;
                }
            }
            let mut w = std::mem::take(&mut self.warnings);
            let fx = intern(&mut self.fx, &mut self.fx_ix, fx_json(&g.filters, g.blend, &mut w));
            self.warnings = w;
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

/// A published page plus everything it can't show exactly.
#[derive(Clone, Debug, Default)]
pub struct Report {
    pub files: Files,
    /// Human-readable notes (approximated blend modes, effects that couldn't be
    /// baked…); nothing is dropped without an entry here.
    pub warnings: Vec<String>,
}

/// Build the published files.
pub fn publish(doc: &Document, o: &Html5Opts) -> Files {
    publish_report(doc, o).files
}

/// Build the published files and the list of things the page can't show exactly.
pub fn publish_report(doc: &Document, o: &Html5Opts) -> Report {
    let total = doc.total_frames();
    let (f0, f1) = o.range.unwrap_or((0, total - 1));
    let (f0, f1) = (f0.min(total - 1), f1.min(total - 1).max(f0.min(total - 1)));
    let mut t = Tables { paths: vec![], path_ix: HashMap::new(), items: vec![], item_ix: HashMap::new(), fx: vec![], fx_ix: HashMap::new(), clips: vec![], clip_ix: HashMap::new(), images: vec![], image_ix: HashMap::new(), w: doc.width.round().max(1.0) as u32, h: doc.height.round().max(1.0) as u32, baked_ix: HashMap::new(), warnings: vec![] };
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
    let warnings = t.warnings.clone();
    let mut data = String::with_capacity(1 << 16);
    let _ = write!(
        data,
        "var AC_MOVIE={{\"w\":{},\"h\":{},\"fps\":{},\"bg\":\"#{:02x}{:02x}{:02x}\",\"loop\":{},\"scenes\":[{}],\"labels\":[{}],\"paths\":[{}],\"items\":[{}],\"fx\":[{}],\"clips\":[{}],\"images\":[{}],\"sounds\":[{}],\"warnings\":[{}],\"frames\":[{}],\"extra\":[{}]}};",
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
        t.warnings.iter().map(|w| js_str(w)).collect::<Vec<_>>().join(","),
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
    Report { files, warnings }
}

/// Publish to a folder (index.html + movie.js + runtime + images/) or, for
/// a path ending in .html, one self-contained page.
pub fn publish_to(doc: &Document, o: &Html5Opts, path: &std::path::Path) -> Result<std::path::PathBuf, String> {
    publish_to_report(doc, o, path).map(|(p, _)| p)
}

/// `publish_to`, also returning the publish report's warnings.
pub fn publish_to_report(doc: &Document, o: &Html5Opts, path: &std::path::Path) -> Result<(std::path::PathBuf, Vec<String>), String> {
    let single = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("html") || e.eq_ignore_ascii_case("htm"));
    let o = Html5Opts { single_file: single || o.single_file, ..o.clone() };
    let Report { files, warnings } = publish_report(doc, &o);
    if single {
        std::fs::write(path, &files[0].1).map_err(|e| format!("{}: {e}", path.display()))?;
        return Ok((path.to_path_buf(), warnings));
    }
    for (name, bytes) in files {
        let p = path.join(&name);
        if let Some(d) = p.parent() {
            std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
        }
        std::fs::write(&p, bytes).map_err(|e| format!("{}: {e}", p.display()))?;
    }
    Ok((path.join("index.html"), warnings))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fx(filters: &[Filter], blend: BlendMode) -> (serde_json::Value, Vec<String>) {
        let mut w = vec![];
        let s = fx_json(filters, blend, &mut w);
        (serde_json::from_str(&s).unwrap_or_else(|e| panic!("{e}: {s}")), w)
    }
    fn red() -> Color {
        Color::rgb(255, 0, 0)
    }

    #[test]
    fn blur_and_plain_drop_shadow_use_canvas_filter_strings() {
        let f = [
            Filter::Blur { blur_x: 8.0, blur_y: 4.0, quality: 1 },
            Filter::DropShadow { blur_x: 4.0, blur_y: 4.0, strength: 100.0, quality: 1, angle: 0.0, distance: 6.0, color: Color::rgb(0, 0, 0), knockout: false, inner: false, hide_object: false },
        ];
        let (j, w) = fx(&f, BlendMode::Normal);
        assert_eq!(j["f"], "blur(3px) drop-shadow(6px 0px 2px rgba(0,0,0,1))");
        assert!(j.get("s").is_none(), "no per-pixel stages: {j}");
        assert_eq!(j["o"], "source-over");
        assert!(w.is_empty());
    }

    #[test]
    fn shadow_variants_canvas_cannot_do_run_as_pixel_stages() {
        for (inner, knockout, hide, strength, t) in [(true, false, false, 100.0, 0), (false, true, false, 100.0, 1), (false, false, true, 100.0, 1), (false, false, false, 250.0, 1)] {
            let f = [Filter::DropShadow { blur_x: 4.0, blur_y: 4.0, strength, quality: 2, angle: 90.0, distance: 3.0, color: red(), knockout, inner, hide_object: hide }];
            let (j, _) = fx(&f, BlendMode::Normal);
            let g = &j["s"][0]["g"];
            assert_eq!(j["f"], "", "{j}");
            assert_eq!(g["k"], "glow", "{j}");
            assert_eq!((g["t"].as_i64(), g["ko"].as_bool(), g["h"].as_bool(), g["nm"].as_bool(), g["q"].as_i64()), (Some(t), Some(knockout), Some(hide), Some(true), Some(2)), "{j}");
            assert_eq!(g["s"], strength / 100.0);
            assert_eq!(g["r"], serde_json::json!([[0, 255, 0, 0, 0], [1, 255, 0, 0, 255]]), "intensity is the alpha, colour constant");
        }
    }

    #[test]
    fn glow_bevel_and_adjust_color_are_stages_with_their_parameters() {
        let f = [
            Filter::Glow { blur_x: 6.0, blur_y: 6.0, strength: 200.0, quality: 3, color: Color { r: 1, g: 2, b: 3, a: 128 }, knockout: true, inner: true },
            Filter::Bevel { blur_x: 5.0, blur_y: 5.0, strength: 100.0, quality: 1, shadow: Color::rgb(0, 0, 0), highlight: Color::rgb(255, 255, 255), angle: 45.0, distance: 5.0, knockout: false, kind: BevelType::Full },
            Filter::AdjustColor { brightness: 10.0, contrast: -20.0, saturation: 30.0, hue: 90.0 },
        ];
        let (j, w) = fx(&f, BlendMode::Multiply);
        let s = j["s"].as_array().unwrap();
        assert_eq!(s.len(), 3, "{j}");
        let g = &s[0]["g"];
        assert_eq!((g["k"].as_str(), g["a"].as_i64(), g["d"].as_i64(), g["t"].as_i64(), g["ko"].as_bool(), g["s"].as_f64()), (Some("glow"), Some(0), Some(0), Some(0), Some(true), Some(2.0)));
        assert_eq!(g["r"][1], serde_json::json!([1, 1, 2, 3, 128]));
        let b = &s[1]["g"];
        assert_eq!((b["k"].as_str(), b["t"].as_i64(), b["a"].as_f64(), b["d"].as_f64()), (Some("sbevel"), Some(2), Some(45.0), Some(5.0)));
        assert_eq!(b["sh"][1], serde_json::json!([1, 0, 0, 0, 255]));
        assert_eq!(b["hl"][1], serde_json::json!([1, 255, 255, 255, 255]));
        assert_eq!(s[2]["a"], serde_json::json!([10, -20, 30, 90]));
        assert_eq!(j["o"], "multiply");
        assert!(w.is_empty(), "{w:?}");
    }

    #[test]
    fn css_filters_between_pixel_stages_keep_their_order() {
        let f = [
            Filter::Blur { blur_x: 4.0, blur_y: 4.0, quality: 1 },
            Filter::Glow { blur_x: 2.0, blur_y: 2.0, strength: 100.0, quality: 1, color: red(), knockout: false, inner: false },
            Filter::Blur { blur_x: 8.0, blur_y: 8.0, quality: 1 },
        ];
        let (j, _) = fx(&f, BlendMode::Normal);
        let s = j["s"].as_array().unwrap();
        assert_eq!(s.len(), 3);
        assert_eq!(s[0]["c"], "blur(2px)");
        assert!(s[1].get("g").is_some());
        assert_eq!(s[2]["c"], "blur(4px)");
    }

    #[test]
    fn approximated_blend_modes_are_reported() {
        let (j, w) = fx(&[], BlendMode::Subtract);
        assert_eq!(j["o"], "difference");
        assert_eq!(w.len(), 1);
        assert!(w[0].contains("Subtract"));
        let (_, w) = fx(&[], BlendMode::Invert);
        assert!(w[0].contains("Invert"));
        let (_, w) = fx(&[], BlendMode::Screen);
        assert!(w.is_empty());
    }

    #[test]
    fn runtime_defines_every_stage_the_generator_emits() {
        for needle in ["function adjustColor", "g.k === \"sbevel\"", "g.k === \"glow\"", "g.nm", "g.h ?", "e.sp !== undefined", "st.a)", "function css("] {
            assert!(RUNTIME.contains(needle), "runtime lacks `{needle}`");
        }
    }

    #[test]
    fn runtime_is_valid_javascript() {
        let Ok(out) = std::process::Command::new("node").arg("--version").output() else { return };
        if !out.status.success() {
            return;
        }
        let dir = std::env::temp_dir().join(format!("flare-rt-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("runtime.js");
        std::fs::write(&f, RUNTIME).unwrap();
        let r = std::process::Command::new("node").arg("--check").arg(&f).output().unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    }
}
