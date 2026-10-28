//! File › Import: bitmaps (PNG/JPG/GIF/BMP/TGA/TIFF/WebP/EXR, animated GIF, numbered
//! sequences) into the library and onto the stage, and SVG as editable shapes.

use crate::App;
use ac_core::{pt, BitmapAsset, Color, Element, Gradient, GradientKind, Keyframe, Matrix, Path, Pt, Seg, Shape, Stroke};
use std::path::{Path as FsPath, PathBuf};

fn file_name(path: &str) -> String {
    FsPath::new(path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "Bitmap".into())
}

/// Decode any supported image into a library bitmap (PNGs are kept as they are;
/// PNG/JPEG/GIF/BMP/TGA/TIFF/WebP/OpenEXR via ac_core::codecs).
fn load_bitmap(path: &str) -> Result<BitmapAsset, String> {
    ac_core::codecs::load_bitmap(FsPath::new(path))
}

/// Where to put something of size w×h: centred on the stage (top-left in a symbol).
fn place_at(app: &App, w: f32, h: f32) -> Pt {
    if app.ed.editing.is_some() {
        return pt(0.0, 0.0);
    }
    let d = &app.ed.doc;
    pt(((d.width - w) / 2.0).max(0.0).round(), ((d.height - h) / 2.0).max(0.0).round())
}

pub(crate) fn import_image(app: &mut App, path: &str) -> Result<String, String> {
    if path.to_lowercase().ends_with(".gif") {
        if let Some(r) = import_animated_gif(app, path) {
            return r;
        }
    }
    if !app.ed.layer_editable(app.ed.layer) {
        return Err("the current layer is locked or hidden".into());
    }
    let asset = load_bitmap(path)?;
    let (w, h, name) = (asset.width, asset.height, asset.name.clone());
    let mark = app.ed.undo_depth();
    let id = app.ed.add_bitmap(asset);
    let at = place_at(app, w as f32, h as f32);
    let s = app.ed.place_bitmap(id, at);
    app.ed.squash_undo(mark, "Import");
    if let Some(s) = s {
        app.ed.selection = vec![s];
    }
    Ok(format!("Imported {name} ({w}×{h}) to the stage and library"))
}

/// File › Import › Import Photoshop (PSD): each pixel layer becomes a library
/// bitmap on its own timeline layer at its position on the canvas (the PSD
/// canvas maps onto the stage at 0,0), keeping its name, visibility and
/// opacity. Groups are flattened into their layers. (psd crate, MIT/Apache.)
pub(crate) fn import_psd(app: &mut App, path: &str) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let psd = psd::Psd::from_bytes(&bytes).map_err(|e| format!("not a PSD this build can read: {e}"))?;
    let (pw, ph) = (psd.width(), psd.height());
    let mark = app.ed.undo_depth();
    let mut made = 0;
    // psd lists layers top first; add bottom first so each goes above the last
    for layer in psd.layers().iter().rev() {
        // bounds: right/bottom are the last pixel (inclusive)
        let (l, t) = (layer.layer_left().max(0), layer.layer_top().max(0));
        let (r, b) = ((layer.layer_right() + 1).min(pw as i32), (layer.layer_bottom() + 1).min(ph as i32));
        if r <= l || b <= t {
            continue; // empty or off-canvas
        }
        let full = layer.rgba();
        if full.len() < (pw * ph * 4) as usize {
            continue;
        }
        let (w, h) = ((r - l) as u32, (b - t) as u32);
        let mut crop = Vec::with_capacity((w * h * 4) as usize);
        for y in t..b {
            let row = (y as u32 * pw + l as u32) as usize * 4;
            crop.extend_from_slice(&full[row..row + w as usize * 4]);
        }
        let name = if layer.name().trim().is_empty() { format!("Layer {}", made + 1) } else { layer.name().to_string() };
        let asset = ac_core::bitmap::BitmapAsset::from_rgba(0, &name, w, h, &crop)?;
        let id = app.ed.add_bitmap(asset);
        app.ed.add_layer();
        let li = app.ed.layer;
        app.ed.rename_layer(li, &name);
        if let Some(s) = app.ed.place_bitmap(id, ac_core::pt(l as f32, t as f32)) {
            let a = layer.opacity() as f32 / 255.0;
            if a < 1.0 {
                let f = app.ed.frame;
                let l = &mut app.ed.timeline_mut().layers[s.layer];
                if let Some(start) = l.span_at(f).map(|(st, _, _)| st) {
                    if let Some(e) = l.keyframes.get_mut(&start).and_then(|k| k.elements.get_mut(s.element)) {
                        e.alpha = a;
                    }
                }
            }
        }
        // psd 0.3 reports the raw flag bit; Photoshop sets it on *hidden* layers
        if layer.visible() {
            app.ed.toggle_layer_flag(li, ac_core::LayerFlag::Visible);
        }
        made += 1;
    }
    app.ed.squash_undo(mark, "Import PSD");
    if made == 0 {
        return Err("the PSD has no pixel layers".into());
    }
    Ok(format!("Imported {made} Photoshop layer{} ({pw}×{ph})", if made == 1 { "" } else { "s" }))
}

/// Animated GIF → one keyframe per frame on a new layer. None = not animated.
fn import_animated_gif(app: &mut App, path: &str) -> Option<Result<String, String>> {
    use image::AnimationDecoder;
    let f = std::fs::File::open(path).ok()?;
    let dec = image::codecs::gif::GifDecoder::new(std::io::BufReader::new(f)).ok()?;
    let frames: Vec<image::Frame> = dec.into_frames().collect_frames().ok()?;
    if frames.len() < 2 {
        return None;
    }
    let name = file_name(path);
    let mut assets = Vec::new();
    for (i, fr) in frames.iter().enumerate() {
        let buf = fr.buffer();
        match BitmapAsset::from_rgba(0, &format!("{name} {:03}", i + 1), buf.width(), buf.height(), buf.as_raw()) {
            Ok(a) => assets.push(a),
            Err(e) => return Some(Err(e)),
        }
    }
    Some(import_frames(app, &name, assets))
}

/// Put a list of bitmaps on consecutive keyframes of a new layer.
fn import_frames(app: &mut App, name: &str, assets: Vec<BitmapAsset>) -> Result<String, String> {
    let n = assets.len();
    let (w, h) = assets.first().map(|a| (a.width as f32, a.height as f32)).unwrap_or((0.0, 0.0));
    let at = place_at(app, w, h);
    let mark = app.ed.undo_depth();
    app.ed.add_layer();
    let li = app.ed.layer;
    let start = app.ed.frame;
    let ids: Vec<u32> = assets.into_iter().map(|a| app.ed.add_bitmap(a)).collect();
    {
        let l = &mut app.ed.timeline_mut().layers[li];
        l.name = name.to_string();
        l.keyframes.clear();
        if start > 0 {
            l.keyframes.insert(0, Keyframe::blank());
        }
        for (k, id) in ids.iter().enumerate() {
            l.keyframes.insert(start + k as u32, Keyframe::with(vec![Element::bitmap("", *id, Matrix::translate(at.x, at.y))]));
        }
        l.end = start + n as u32;
    }
    app.ed.squash_undo(mark, "Import");
    Ok(format!("Imported {n} frames of {name} onto a new layer"))
}

/// File › Import › Image Sequence: `walk0001.png`, `walk0002.png`, … next to the picked file.
pub(crate) fn import_sequence(app: &mut App, path: &str) -> Result<String, String> {
    let p = PathBuf::from(path);
    let stem = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let ext = p.extension().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let digits = stem.chars().rev().take_while(|c| c.is_ascii_digit()).count();
    if digits == 0 {
        return import_image(app, path);
    }
    let prefix = &stem[..stem.len() - digits];
    let dir = p.parent().map(|d| d.to_path_buf()).unwrap_or_default();
    let mut files: Vec<(u64, PathBuf)> = std::fs::read_dir(&dir)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter_map(|f| {
            let s = f.file_stem()?.to_string_lossy().to_string();
            let e = f.extension()?.to_string_lossy().to_string();
            if !e.eq_ignore_ascii_case(&ext) || !s.starts_with(prefix) {
                return None;
            }
            let num = &s[prefix.len()..];
            (!num.is_empty() && num.chars().all(|c| c.is_ascii_digit())).then(|| (num.parse().unwrap_or(0), f))
        })
        .collect();
    files.sort();
    let assets: Result<Vec<BitmapAsset>, String> = files.iter().map(|(_, f)| load_bitmap(&f.display().to_string())).collect();
    import_frames(app, prefix.trim_end_matches(['_', '-', ' ']), assets?)
}

// ------------------------------------------------------------------ SVG

fn color(c: usvg::Color, opacity: f32) -> Color {
    Color { r: c.red, g: c.green, b: c.blue, a: (opacity.clamp(0.0, 1.0) * 255.0).round() as u8 }
}

fn convert_path(p: &usvg::tiny_skia_path::Path) -> Path {
    use usvg::tiny_skia_path::PathSegment as S;
    let mut segs = Vec::new();
    for s in p.segments() {
        segs.push(match s {
            S::MoveTo(a) => Seg::M(pt(a.x, a.y)),
            S::LineTo(a) => Seg::L(pt(a.x, a.y)),
            S::QuadTo(c, a) => Seg::Q(pt(c.x, c.y), pt(a.x, a.y)),
            S::CubicTo(c1, c2, a) => Seg::C(pt(c1.x, c1.y), pt(c2.x, c2.y), pt(a.x, a.y)),
            S::Close => Seg::Z,
        });
    }
    Path { segs }
}

fn tf(t: usvg::Transform) -> Matrix {
    Matrix { a: t.sx, b: t.ky, c: t.kx, d: t.sy, tx: t.tx, ty: t.ty }
}

/// Fill paint → (solid colour, gradient in the path's local space).
fn paint(p: &usvg::Paint, opacity: f32) -> (Option<Color>, Option<Gradient>) {
    let stops = |s: &[usvg::Stop]| -> Vec<(f32, Color)> { s.iter().map(|st| (st.offset().get(), color(st.color(), st.opacity().get() * opacity))).collect() };
    match p {
        usvg::Paint::Color(c) => (Some(color(*c, opacity)), None),
        usvg::Paint::LinearGradient(g) => {
            let st = stops(g.stops());
            let (a, b) = (pt(g.x1(), g.y1()), pt(g.x2(), g.y2()));
            let len = a.dist(b).max(1e-3);
            let m = Matrix::translate((a.x + b.x) / 2.0, (a.y + b.y) / 2.0).concat(&Matrix::rotate((b.y - a.y).atan2(b.x - a.x))).concat(&Matrix::scale(len / 2.0, len / 2.0));
            (st.first().map(|s| s.1), Some(Gradient { kind: GradientKind::Linear, stops: st, matrix: tf(g.transform()).concat(&m), spread: match g.spread_method() { usvg::SpreadMethod::Reflect => ac_core::Spread::Reflect, usvg::SpreadMethod::Repeat => ac_core::Spread::Repeat, _ => ac_core::Spread::Pad }, focal: 0.0 }))
        }
        usvg::Paint::RadialGradient(g) => {
            let st = stops(g.stops());
            let r = g.r().get();
            // the focal point (fx, fy) becomes Animate's focal ratio along a rotated x axis
            let (dx, dy) = ((g.fx() - g.cx()) / r.max(1e-6), (g.fy() - g.cy()) / r.max(1e-6));
            let focal = (dx * dx + dy * dy).sqrt().min(0.99);
            let m = Matrix::translate(g.cx(), g.cy()).concat(&Matrix::rotate(if focal > 1e-4 { dy.atan2(dx) } else { 0.0 })).concat(&Matrix::scale(r, r));
            (st.first().map(|s| s.1), Some(Gradient { kind: GradientKind::Radial, stops: st, matrix: tf(g.transform()).concat(&m), spread: match g.spread_method() { usvg::SpreadMethod::Reflect => ac_core::Spread::Reflect, usvg::SpreadMethod::Repeat => ac_core::Spread::Repeat, _ => ac_core::Spread::Pad }, focal }))
        }
        usvg::Paint::Pattern(_) => (Some(Color::rgb(128, 128, 128)), None),
    }
}

fn walk(g: &usvg::Group, out: &mut Vec<Element>) {
    for node in g.children() {
        match node {
            usvg::Node::Group(c) => walk(c, out),
            usvg::Node::Path(p) => {
                if !p.is_visible() {
                    continue;
                }
                let (fill, gradient) = p.fill().map(|f| paint(f.paint(), f.opacity().get())).unwrap_or((None, None));
                let stroke = p.stroke().and_then(|s| match s.paint() {
                    usvg::Paint::Color(c) => Some(Stroke { color: color(*c, s.opacity().get()), width: s.width().get() }),
                    other => paint(other, s.opacity().get()).0.map(|c| Stroke { color: c, width: s.width().get() }),
                });
                if fill.is_none() && stroke.is_none() {
                    continue;
                }
                let mut shape = Shape::new(convert_path(p.data()), fill, stroke);
                shape.gradient = gradient;
                let mut el = Element::shape(p.id(), shape);
                el.matrix = tf(p.abs_transform());
                out.push(el);
            }
            usvg::Node::Text(t) => walk(t.flattened(), out),
            usvg::Node::Image(_) => {}
        }
    }
}

pub(crate) fn import_svg(app: &mut App, path: &str) -> Result<String, String> {
    if !app.ed.layer_editable(app.ed.layer) {
        return Err("the current layer is locked or hidden".into());
    }
    let data = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    let mut opt = usvg::Options::default();
    opt.fontdb_mut().load_system_fonts();
    let tree = usvg::Tree::from_data(&data, &opt).map_err(|e| format!("not a valid SVG ({e})"))?;
    let mut els = Vec::new();
    walk(tree.root(), &mut els);
    if els.is_empty() {
        return Err("the SVG has no visible shapes".into());
    }
    let n = els.len();
    let s = tree.size();
    let at = place_at(app, s.width(), s.height());
    let mut g = Element::group(els);
    g.name = file_name(path);
    g.matrix = Matrix::translate(at.x, at.y);
    match app.ed.add_element(g) {
        Some(sel) => {
            app.ed.selection = vec![sel];
            Ok(format!("Imported {} as a group of {n} shapes (Ctrl+Shift+G to ungroup)", file_name(path)))
        }
        None => Err("the current layer is locked or hidden".into()),
    }
}

/// Files dropped onto the window.
pub(crate) fn dropped(app: &mut App, path: &str) -> Result<String, String> {
    let l = path.to_lowercase();
    if l.ends_with(".svg") {
        import_svg(app, path)
    } else if l.ends_with(".psd") {
        import_psd(app, path)
    } else if l.ends_with(".xdts") {
        ac_core::xdts::import_file(&mut app.ed, FsPath::new(path)).map(|r| r.summary())
    } else if [".wav", ".mp3", ".ogg", ".oga", ".flac", ".aif", ".aiff"].iter().any(|e| l.ends_with(e)) {
        app.import_sound(path)
    } else if l.ends_with(".json") || l.ends_with(".acx") || l.ends_with(".fla") || l.ends_with(".xfl") {
        app.guarded(crate::files::Pending::OpenPath(path.to_string()));
        Ok(String::new())
    } else {
        import_image(app, path)
    }
}
