//! More publish formats: sprite sheets (PNG + JSON / XML atlas) and Lottie
//! JSON (one shape layer per distinct frame, so it plays in any Lottie
//! player exactly as the stage shows it).

use crate::{render_frame_opts, unpremultiplied, ExportOpts};
use ac_core::model::*;
use ac_core::{Matrix, Pt, Seg};
use std::fmt::Write as _;

// ---------------------------------------------------------------- sprite sheets

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum AtlasFormat {
    /// TexturePacker-style JSON hash (`frames: { "name0001": {...} }`).
    #[default]
    JsonHash,
    /// JSON array (`frames: [ {filename, frame, …} ]`).
    JsonArray,
    /// Starling / Sparrow XML (`<TextureAtlas><SubTexture …/>`).
    StarlingXml,
}

impl AtlasFormat {
    pub const ALL: [AtlasFormat; 3] = [AtlasFormat::JsonHash, AtlasFormat::JsonArray, AtlasFormat::StarlingXml];
    pub fn name(self) -> &'static str {
        match self {
            AtlasFormat::JsonHash => "JSON (hash)",
            AtlasFormat::JsonArray => "JSON (array)",
            AtlasFormat::StarlingXml => "Starling XML",
        }
    }
    pub fn ext(self) -> &'static str {
        match self {
            AtlasFormat::StarlingXml => "xml",
            _ => "json",
        }
    }
}

#[derive(Clone, Debug)]
pub struct SheetOpts {
    pub format: AtlasFormat,
    /// Crop each frame to its visible pixels (offsets kept in the data).
    pub trim: bool,
    /// Frames that look the same share one cell.
    pub dedupe: bool,
    /// Pixels between cells.
    pub padding: u32,
    /// Widest sheet before wrapping to a new row.
    pub max_width: u32,
}

impl Default for SheetOpts {
    fn default() -> Self {
        SheetOpts { format: AtlasFormat::JsonHash, trim: true, dedupe: true, padding: 2, max_width: 4096 }
    }
}

struct Cell {
    rgba: Vec<u8>,
    w: u32,
    h: u32,
    /// offset of the trimmed cell inside the full frame
    ox: u32,
    oy: u32,
}

/// Render the frames into one sheet. Returns (PNG bytes, atlas text).
/// `image_name` is the PNG's file name as the atlas should refer to it.
pub fn sprite_sheet(doc: &Document, o: &ExportOpts, so: &SheetOpts, image_name: &str) -> Result<(Vec<u8>, String), String> {
    let frames: Vec<u32> = o.frames(doc).collect();
    let (fw, fh) = ((doc.width * o.scale).round().max(1.0) as u32, (doc.height * o.scale).round().max(1.0) as u32);
    let mut cells: Vec<Cell> = vec![];
    let mut frame_cell: Vec<usize> = vec![];
    for &f in &frames {
        let pix = render_frame_opts(doc, f, o.scale, o.transparent);
        let rgba = unpremultiplied(&pix);
        let (mut x0, mut y0, mut x1, mut y1) = (0, 0, fw, fh);
        if so.trim {
            let opaque = |x: u32, y: u32| rgba[((y * fw + x) * 4 + 3) as usize] > 0;
            let rows: Vec<u32> = (0..fh).filter(|&y| (0..fw).any(|x| opaque(x, y))).collect();
            if let (Some(&a), Some(&b)) = (rows.first(), rows.last()) {
                let cols: Vec<u32> = (0..fw).filter(|&x| (a..=b).any(|y| opaque(x, y))).collect();
                (x0, y0, x1, y1) = (cols[0], a, cols[cols.len() - 1] + 1, b + 1);
            } else {
                (x0, y0, x1, y1) = (0, 0, 1, 1);
            }
        }
        let (w, h) = (x1 - x0, y1 - y0);
        let mut cut = Vec::with_capacity((w * h * 4) as usize);
        for y in y0..y1 {
            let s = ((y * fw + x0) * 4) as usize;
            cut.extend_from_slice(&rgba[s..s + (w * 4) as usize]);
        }
        let cell = Cell { rgba: cut, w, h, ox: x0, oy: y0 };
        let found = if so.dedupe { cells.iter().position(|c| c.w == cell.w && c.h == cell.h && c.ox == cell.ox && c.oy == cell.oy && c.rgba == cell.rgba) } else { None };
        match found {
            Some(i) => frame_cell.push(i),
            None => {
                cells.push(cell);
                frame_cell.push(cells.len() - 1);
            }
        }
    }
    // shelf packing, tallest first
    let pad = so.padding;
    let mut order: Vec<usize> = (0..cells.len()).collect();
    order.sort_by_key(|&i| std::cmp::Reverse(cells[i].h));
    let maxw = so.max_width.max(cells.iter().map(|c| c.w + pad).max().unwrap_or(1));
    let mut pos = vec![(0u32, 0u32); cells.len()];
    let (mut x, mut y, mut shelf, mut sheet_w) = (0u32, 0u32, 0u32, 0u32);
    for &i in &order {
        let c = &cells[i];
        if x > 0 && x + c.w > maxw {
            x = 0;
            y += shelf + pad;
            shelf = 0;
        }
        pos[i] = (x, y);
        x += c.w + pad;
        shelf = shelf.max(c.h);
        sheet_w = sheet_w.max(x - pad);
    }
    let sheet_h = (y + shelf).max(1);
    let sheet_w = sheet_w.max(1);
    let mut sheet = vec![0u8; (sheet_w * sheet_h * 4) as usize];
    for (i, c) in cells.iter().enumerate() {
        let (px, py) = pos[i];
        for row in 0..c.h {
            let d = (((py + row) * sheet_w + px) * 4) as usize;
            let s = (row * c.w * 4) as usize;
            sheet[d..d + (c.w * 4) as usize].copy_from_slice(&c.rgba[s..s + (c.w * 4) as usize]);
        }
    }
    let mut png_bytes = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut png_bytes, sheet_w, sheet_h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut wr = enc.write_header().map_err(|e| e.to_string())?;
        wr.write_image_data(&sheet).map_err(|e| e.to_string())?;
    }
    let stem = image_name.rsplit_once('.').map(|(a, _)| a).unwrap_or(image_name);
    let name = |k: usize| format!("{stem}{:04}", k);
    let mut atlas = String::new();
    match so.format {
        AtlasFormat::JsonHash | AtlasFormat::JsonArray => {
            let entries: Vec<String> = frame_cell
                .iter()
                .enumerate()
                .map(|(k, &ci)| {
                    let c = &cells[ci];
                    let (px, py) = pos[ci];
                    let body = format!(
                        "\"frame\":{{\"x\":{px},\"y\":{py},\"w\":{},\"h\":{}}},\"rotated\":false,\"trimmed\":{},\"spriteSourceSize\":{{\"x\":{},\"y\":{},\"w\":{},\"h\":{}}},\"sourceSize\":{{\"w\":{fw},\"h\":{fh}}},\"duration\":{}",
                        c.w,
                        c.h,
                        c.w != fw || c.h != fh,
                        c.ox,
                        c.oy,
                        c.w,
                        c.h,
                        (1000.0 / doc.fps.max(1.0)).round()
                    );
                    if so.format == AtlasFormat::JsonHash {
                        format!("\n  \"{}\": {{{body}}}", name(k))
                    } else {
                        format!("\n  {{\"filename\":\"{}\",{body}}}", name(k))
                    }
                })
                .collect();
            let (open, close) = if so.format == AtlasFormat::JsonHash { ("{", "}") } else { ("[", "]") };
            let _ = write!(
                atlas,
                "{{\"frames\": {open}{}\n{close},\n\"meta\": {{\"app\":\"Flare\",\"version\":\"1.0\",\"image\":\"{image_name}\",\"format\":\"RGBA8888\",\"size\":{{\"w\":{sheet_w},\"h\":{sheet_h}}},\"scale\":\"{}\",\"frameRate\":{}}}\n}}\n",
                entries.join(","),
                o.scale,
                doc.fps
            );
        }
        AtlasFormat::StarlingXml => {
            let _ = writeln!(atlas, "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<TextureAtlas imagePath=\"{image_name}\">");
            for (k, &ci) in frame_cell.iter().enumerate() {
                let c = &cells[ci];
                let (px, py) = pos[ci];
                let _ = writeln!(atlas, "  <SubTexture name=\"{}\" x=\"{px}\" y=\"{py}\" width=\"{}\" height=\"{}\" frameX=\"-{}\" frameY=\"-{}\" frameWidth=\"{fw}\" frameHeight=\"{fh}\"/>", name(k), c.w, c.h, c.ox, c.oy);
            }
            atlas.push_str("</TextureAtlas>\n");
        }
    }
    Ok((png_bytes, atlas))
}

/// Write `<path>.png` and its atlas next to it; returns both paths.
pub fn export_sprite_sheet(doc: &Document, o: &ExportOpts, so: &SheetOpts, path: &std::path::Path) -> Result<(std::path::PathBuf, std::path::PathBuf), String> {
    let png_path = path.with_extension("png");
    let atlas_path = path.with_extension(so.format.ext());
    let name = png_path.file_name().and_then(|s| s.to_str()).unwrap_or("sheet.png").to_string();
    let (png_bytes, atlas) = sprite_sheet(doc, o, so, &name)?;
    std::fs::write(&png_path, png_bytes).map_err(|e| format!("{}: {e}", png_path.display()))?;
    std::fs::write(&atlas_path, atlas).map_err(|e| format!("{}: {e}", atlas_path.display()))?;
    Ok((png_path, atlas_path))
}

// ---------------------------------------------------------------- Lottie

fn shade(c: Color, alpha: f32, tint: Option<(Color, f32)>, cx: Option<ColorXform>) -> ([f32; 3], f32) {
    let c = match tint {
        Some((t, amt)) => Color { a: c.a, ..c.lerp(Color { a: c.a, ..t }, amt.clamp(0.0, 1.0)) },
        None => c,
    };
    let c = cx.map(|x| x.apply(c)).unwrap_or(c);
    ([c.r as f32 / 255.0, c.g as f32 / 255.0, c.b as f32 / 255.0], c.a as f32 / 255.0 * alpha.clamp(0.0, 1.0) * 100.0)
}

fn f3(v: f32) -> String {
    let r = (v * 1000.0).round() / 1000.0;
    if r == r.trunc() { format!("{}", r as i64) } else { format!("{r}") }
}

/// Lottie bezier shapes (one per subpath), points baked through `m`.
fn lottie_paths(p: &ac_core::Path, m: &Matrix) -> Vec<String> {
    let mut out = vec![];
    let mut v: Vec<Pt> = vec![];
    let mut ins: Vec<Pt> = vec![];
    let mut outs: Vec<Pt> = vec![];
    let mut closed = false;
    let flush = |v: &mut Vec<Pt>, ins: &mut Vec<Pt>, outs: &mut Vec<Pt>, closed: bool, out: &mut Vec<String>| {
        if v.len() >= 2 {
            // a closing point equal to the first merges into it
            if closed && v.len() > 2 && v[0].dist(*v.last().unwrap()) < 1e-3 {
                let last_in = ins.pop().unwrap();
                v.pop();
                outs.pop();
                ins[0] = last_in;
            }
            let fmt = |a: &[Pt]| a.iter().map(|p| format!("[{},{}]", f3(p.x), f3(p.y))).collect::<Vec<_>>().join(",");
            out.push(format!("{{\"ty\":\"sh\",\"ks\":{{\"a\":0,\"k\":{{\"c\":{closed},\"v\":[{}],\"i\":[{}],\"o\":[{}]}}}}}}", fmt(v), fmt(ins), fmt(outs)));
        }
        v.clear();
        ins.clear();
        outs.clear();
    };
    let zero = Pt::default();
    let rel = |a: Pt, b: Pt| Pt { x: a.x - b.x, y: a.y - b.y };
    for s in &p.segs {
        match *s {
            Seg::M(a) => {
                flush(&mut v, &mut ins, &mut outs, closed, &mut out);
                closed = false;
                v.push(m.apply(a));
                ins.push(zero);
                outs.push(zero);
            }
            Seg::L(a) => {
                v.push(m.apply(a));
                ins.push(zero);
                outs.push(zero);
            }
            Seg::Q(c, a) => {
                let p0 = *v.last().unwrap_or(&zero);
                let (c, a) = (m.apply(c), m.apply(a));
                let c1 = p0.lerp(c, 2.0 / 3.0);
                let c2 = a.lerp(c, 2.0 / 3.0);
                if let Some(o) = outs.last_mut() {
                    *o = rel(c1, p0);
                }
                v.push(a);
                ins.push(rel(c2, a));
                outs.push(zero);
            }
            Seg::C(c1, c2, a) => {
                let p0 = *v.last().unwrap_or(&zero);
                let (c1, c2, a) = (m.apply(c1), m.apply(c2), m.apply(a));
                if let Some(o) = outs.last_mut() {
                    *o = rel(c1, p0);
                }
                v.push(a);
                ins.push(rel(c2, a));
                outs.push(zero);
            }
            Seg::Z => {
                closed = true;
                flush(&mut v, &mut ins, &mut outs, closed, &mut out);
                closed = false;
            }
        }
    }
    flush(&mut v, &mut ins, &mut outs, closed, &mut out);
    out
}

const IDENT_KS: &str = "{\"o\":{\"a\":0,\"k\":100},\"r\":{\"a\":0,\"k\":0},\"p\":{\"a\":0,\"k\":[0,0,0]},\"a\":{\"a\":0,\"k\":[0,0,0]},\"s\":{\"a\":0,\"k\":[100,100,100]}}";
const TR: &str = "{\"ty\":\"tr\",\"p\":{\"a\":0,\"k\":[0,0]},\"a\":{\"a\":0,\"k\":[0,0]},\"s\":{\"a\":0,\"k\":[100,100]},\"r\":{\"a\":0,\"k\":0},\"o\":{\"a\":0,\"k\":100}}";

/// One draw item as a Lottie group (None for images, handled as layers).
fn lottie_item(it: &DrawItem, warn: &mut Vec<&'static str>) -> Option<String> {
    if it.image.is_some() {
        return None;
    }
    if it.clips.len() > 1 {
        warn.push("nested masks: Lottie keeps only the innermost one");
    }
    if !it.fx.is_empty() {
        warn.push("filters, blend modes and Asset Warp on symbols are not exported to Lottie");
    }
    let mut parts = lottie_paths(&it.path, &it.matrix);
    if parts.is_empty() {
        return None;
    }
    let sc = it.matrix.decompose();
    let line_scale = (sc.scale_x.abs() * sc.scale_y.abs()).sqrt().max(1e-3);
    if let Some(oc) = it.outline {
        let (c, o) = shade(oc, it.alpha, None, None);
        parts.push(format!("{{\"ty\":\"st\",\"c\":{{\"a\":0,\"k\":[{},{},{},1]}},\"o\":{{\"a\":0,\"k\":{}}},\"w\":{{\"a\":0,\"k\":1}},\"lc\":2,\"lj\":2}}", f3(c[0]), f3(c[1]), f3(c[2]), f3(o)));
    } else {
        if let Some(st) = it.stroke {
            let (c, o) = shade(st.color, it.alpha, it.tint, it.cx);
            let ss = &it.stroke_style;
            let lc = match (ss.dash, ss.cap) {
                (Dash::Dotted, _) | (_, Cap::Round) => 2,
                (_, Cap::None) => 1,
                (_, Cap::Square) => 3,
            };
            let lj = match ss.join {
                Join::Miter => 1,
                Join::Round => 2,
                Join::Bevel => 3,
            };
            let w = st.width.max(0.1) * line_scale;
            let dashes = match ss.dash {
                Dash::Dashed => format!(",\"d\":[{{\"n\":\"d\",\"nm\":\"dash\",\"v\":{{\"a\":0,\"k\":{}}}}},{{\"n\":\"g\",\"nm\":\"gap\",\"v\":{{\"a\":0,\"k\":{}}}}}]", f3((ss.dash_len * w).max(0.5)), f3((ss.gap * w).max(0.5))),
                Dash::Dotted => format!(",\"d\":[{{\"n\":\"d\",\"nm\":\"dash\",\"v\":{{\"a\":0,\"k\":0.01}}}},{{\"n\":\"g\",\"nm\":\"gap\",\"v\":{{\"a\":0,\"k\":{}}}}}]", f3((ss.gap * w).max(w * 1.2))),
                _ => String::new(),
            };
            parts.push(format!("{{\"ty\":\"st\",\"c\":{{\"a\":0,\"k\":[{},{},{},1]}},\"o\":{{\"a\":0,\"k\":{}}},\"w\":{{\"a\":0,\"k\":{}}},\"lc\":{lc},\"lj\":{lj},\"ml\":4{dashes}}}", f3(c[0]), f3(c[1]), f3(c[2]), f3(o), f3(w)));
        }
        if let Some(g) = &it.gradient {
            let gm = it.matrix.concat(&g.matrix);
            let s = gm.apply(ac_core::pt(if g.kind == GradientKind::Linear { -1.0 } else { 0.0 }, 0.0));
            let e = gm.apply(ac_core::pt(1.0, 0.0));
            let mut k = vec![];
            let mut alphas = vec![];
            for (t, col) in &g.stops {
                let (c, o) = shade(*col, it.alpha, it.tint, it.cx);
                k.extend([f3(*t), f3(c[0]), f3(c[1]), f3(c[2])]);
                alphas.extend([f3(*t), f3(o / 100.0)]);
            }
            k.extend(alphas);
            // radial focal point: Lottie's highlight length (% of the radius) along s → e
            let hl = if g.kind == GradientKind::Radial && g.focal != 0.0 { format!(",\"h\":{{\"a\":0,\"k\":{}}},\"a\":{{\"a\":0,\"k\":0}}", f3(g.focal.clamp(-0.99, 0.99) * 100.0)) } else { String::new() };
            parts.push(format!(
                "{{\"ty\":\"gf\",\"t\":{},\"s\":{{\"a\":0,\"k\":[{},{}]}},\"e\":{{\"a\":0,\"k\":[{},{}]}},\"g\":{{\"p\":{},\"k\":{{\"a\":0,\"k\":[{}]}}}},\"o\":{{\"a\":0,\"k\":100}},\"r\":2{hl}}}",
                if g.kind == GradientKind::Linear { 1 } else { 2 },
                f3(s.x),
                f3(s.y),
                f3(e.x),
                f3(e.y),
                g.stops.len(),
                k.join(",")
            ));
        } else if let Some(fill) = it.fill.or(it.pattern.as_ref().map(|_| Color::rgb(128, 128, 128))) {
            if it.pattern.is_some() {
                warn.push("bitmap fills are exported to Lottie as grey");
            }
            let (c, o) = shade(fill, it.alpha, it.tint, it.cx);
            parts.push(format!("{{\"ty\":\"fl\",\"c\":{{\"a\":0,\"k\":[{},{},{},1]}},\"o\":{{\"a\":0,\"k\":{}}},\"r\":2}}", f3(c[0]), f3(c[1]), f3(c[2]), f3(o)));
        }
    }
    parts.push(TR.into());
    Some(format!("{{\"ty\":\"gr\",\"it\":[{}]}}", parts.join(",")))
}

/// Lottie (Bodymovin) JSON of the movie. Each run of identical frames is
/// one layer; bitmaps are embedded image layers. Returns (json, warnings).
pub fn lottie(doc: &Document, o: &ExportOpts) -> (String, Vec<String>) {
    let frames: Vec<u32> = o.frames(doc).collect();
    let mut warn: Vec<&'static str> = vec![];
    let mut assets: Vec<String> = vec![];
    let mut asset_ix: std::collections::HashMap<usize, usize> = Default::default();
    // per frame: the layer bodies (top first), as (kind, json-without-ip/op)
    let mut runs: Vec<(Vec<String>, u32, u32)> = vec![];
    let mut keep: Vec<Vec<DrawItem>> = vec![];
    for (fi, &f) in frames.iter().enumerate() {
        let items = evaluate_scene(doc, f);
        let mut layers: Vec<String> = vec![];
        let mut groups: Vec<String> = vec![];
        // the mask (if any) the current run of shape groups is clipped by
        let mut run_clip: Option<std::sync::Arc<ac_core::Clip>> = None;
        let flush = |groups: &mut Vec<String>, layers: &mut Vec<String>, clip: &Option<std::sync::Arc<ac_core::Clip>>| {
            if !groups.is_empty() {
                // Lottie draws a layer's first group on top
                groups.reverse();
                let matte = if clip.is_some() { ",\"tt\":1" } else { "" };
                layers.push(format!("\"ty\":4{matte},\"ks\":{IDENT_KS},\"shapes\":[{}]", groups.join(",")));
                groups.clear();
                // the mask as an alpha matte layer just above its content
                if let Some(c) = clip {
                    let shapes: Vec<String> = c.shapes.iter().flat_map(|(p, m)| lottie_paths(p, m)).collect();
                    layers.push(format!("\"ty\":4,\"td\":1,\"ks\":{IDENT_KS},\"shapes\":[{{\"ty\":\"gr\",\"it\":[{},{{\"ty\":\"fl\",\"c\":{{\"a\":0,\"k\":[1,1,1,1]}},\"o\":{{\"a\":0,\"k\":100}},\"r\":1}},{TR}]}}]", shapes.join(",")));
                }
            }
        };
        for it in &items {
            match &it.image {
                Some(img) => {
                    flush(&mut groups, &mut layers, &run_clip);
                    let key = std::sync::Arc::as_ptr(img) as usize;
                    let ix = *asset_ix.entry(key).or_insert_with(|| {
                        let mut rgba = Vec::with_capacity(img.rgba.len());
                        for px in img.rgba.chunks_exact(4) {
                            let a = px[3] as u32;
                            let un = |c: u8| (c as u32 * 255 + a / 2).checked_div(a).unwrap_or(0).min(255) as u8;
                            rgba.extend_from_slice(&[un(px[0]), un(px[1]), un(px[2]), px[3]]);
                        }
                        let mut bytes = Vec::new();
                        {
                            let mut enc = png::Encoder::new(&mut bytes, img.width, img.height);
                            enc.set_color(png::ColorType::Rgba);
                            enc.set_depth(png::BitDepth::Eight);
                            if let Ok(mut w) = enc.write_header() {
                                let _ = w.write_image_data(&rgba);
                            }
                        }
                        assets.push(format!("{{\"id\":\"img_{}\",\"w\":{},\"h\":{},\"u\":\"\",\"p\":\"data:image/png;base64,{}\",\"e\":1}}", assets.len(), img.width, img.height, crate::base64_lite::encode(&bytes)));
                        assets.len() - 1
                    });
                    let d = it.matrix.decompose();
                    let skew = (d.skew_x - d.skew_y).to_degrees();
                    if skew.abs() > 0.01 {
                        warn.push("skewed bitmaps lose their skew in Lottie");
                    }
                    layers.push(format!(
                        "\"ty\":2,\"refId\":\"img_{ix}\",\"ks\":{{\"o\":{{\"a\":0,\"k\":{}}},\"r\":{{\"a\":0,\"k\":{}}},\"p\":{{\"a\":0,\"k\":[{},{},0]}},\"a\":{{\"a\":0,\"k\":[0,0,0]}},\"s\":{{\"a\":0,\"k\":[{},{},100]}}}}",
                        f3(it.alpha * 100.0),
                        f3(d.skew_y.to_degrees()),
                        f3(d.tx),
                        f3(d.ty),
                        f3(d.scale_x * 100.0),
                        f3(d.scale_y * 100.0)
                    ));
                }
                None => {
                    let clip = it.clips.last().cloned();
                    let same = match (&clip, &run_clip) {
                        (Some(a), Some(b)) => std::sync::Arc::ptr_eq(a, b),
                        (None, None) => true,
                        _ => false,
                    };
                    if !same {
                        flush(&mut groups, &mut layers, &run_clip);
                        run_clip = clip;
                    }
                    groups.extend(lottie_item(it, &mut warn));
                }
            }
        }
        flush(&mut groups, &mut layers, &run_clip);
        layers.reverse(); // Lottie lists the top layer first
        keep.push(items);
        match runs.last_mut() {
            Some((l, _, end)) if *l == layers => *end = fi as u32 + 1,
            _ => runs.push((layers, fi as u32, fi as u32 + 1)),
        }
    }
    drop(keep);
    // all layers, top first: later runs never overlap earlier ones in time
    let mut all = vec![];
    let mut ind = 1;
    for (layers, ip, op) in &runs {
        for l in layers {
            all.push(format!("{{\"ddd\":0,\"ind\":{ind},\"nm\":\"f{ip}\",\"ip\":{ip},\"op\":{op},\"st\":0,\"sr\":1,\"bm\":0,{l}}}"));
            ind += 1;
        }
    }
    let bg = doc.background;
    if !o.transparent {
        all.push(format!(
            "{{\"ddd\":0,\"ind\":{ind},\"ty\":1,\"nm\":\"Stage\",\"sc\":\"#{:02x}{:02x}{:02x}\",\"sw\":{},\"sh\":{},\"ip\":0,\"op\":{},\"st\":0,\"sr\":1,\"bm\":0,\"ks\":{{\"o\":{{\"a\":0,\"k\":100}},\"r\":{{\"a\":0,\"k\":0}},\"p\":{{\"a\":0,\"k\":[0,0,0]}},\"a\":{{\"a\":0,\"k\":[0,0,0]}},\"s\":{{\"a\":0,\"k\":[100,100,100]}}}}}}",
            bg.r,
            bg.g,
            bg.b,
            doc.width.round(),
            doc.height.round(),
            frames.len()
        ));
    }
    let json = format!(
        "{{\"v\":\"5.7.0\",\"fr\":{},\"ip\":0,\"op\":{},\"w\":{},\"h\":{},\"nm\":\"Flare\",\"ddd\":0,\"assets\":[{}],\"layers\":[{}],\"meta\":{{\"g\":\"Flare\"}}}}\n",
        f3(doc.fps),
        frames.len(),
        doc.width.round(),
        doc.height.round(),
        assets.join(","),
        all.join(",\n")
    );
    let mut w: Vec<String> = vec![];
    for x in warn {
        if !w.iter().any(|y| y == x) {
            w.push(x.to_string());
        }
    }
    (json, w)
}
