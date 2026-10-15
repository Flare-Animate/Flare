//! Rendering back-ends. The raster path is shared by the editor stage and
//! every export so what you see is exactly what you export.

use ac_core::{evaluate_scene, Color, Document, DrawItem, Matrix, Path, Seg};
use tiny_skia as sk;

#[path = "ac_fx.rs"]
mod fx;
#[path = "ac_effectcraft.rs"]
pub mod effectcraft;
#[path = "ac_fonts.rs"]
pub mod fonts;
#[path = "ac_formats.rs"]
pub mod formats;
#[path = "ac_html5.rs"]
pub mod html5;

pub struct RenderOpts {
    /// Extra transform applied on top of document coordinates (zoom/pan).
    pub view: Matrix,
    /// Multiply all alpha (used for onion skin ghosts).
    pub alpha: f32,
    /// Replace colours with this tint (onion skin), amount 0..1.
    pub tint: Option<(Color, f32)>,
}

impl Default for RenderOpts {
    fn default() -> Self {
        RenderOpts { view: Matrix::IDENTITY, alpha: 1.0, tint: None }
    }
}

pub(crate) fn to_sk_path(p: &Path) -> Option<sk::Path> {
    let mut b = sk::PathBuilder::new();
    for s in &p.segs {
        match *s {
            Seg::M(a) => b.move_to(a.x, a.y),
            Seg::L(a) => b.line_to(a.x, a.y),
            Seg::Q(c, a) => b.quad_to(c.x, c.y, a.x, a.y),
            Seg::C(c1, c2, a) => b.cubic_to(c1.x, c1.y, c2.x, c2.y, a.x, a.y),
            Seg::Z => b.close(),
        }
    }
    b.finish()
}

pub(crate) fn to_sk_transform(m: &Matrix) -> sk::Transform {
    sk::Transform::from_row(m.a, m.b, m.c, m.d, m.tx, m.ty)
}

fn shade_cx(c: Color, alpha: f32, tint: Option<(Color, f32)>, cx: Option<ac_core::ColorXform>) -> sk::Color {
    let c = match tint {
        Some((t, amt)) => Color { a: c.a, ..c.lerp(Color { a: c.a, ..t }, amt.clamp(0.0, 1.0)) },
        None => c,
    };
    let c = cx.map(|x| x.apply(c)).unwrap_or(c);
    sk::Color::from_rgba8(c.r, c.g, c.b, ((c.a as f32) * alpha.clamp(0.0, 1.0)).round() as u8)
}

pub fn draw_items(pix: &mut sk::PixmapMut, items: &[DrawItem], opts: &RenderOpts) {
    let mut masks = fx::MaskCache::default();
    draw_level(pix, items, 0, opts, &mut masks);
}

/// Draw items, compositing each run of items that shares an effect group at
/// `level` through an offscreen layer (filters, blend modes).
fn draw_level(pix: &mut sk::PixmapMut, items: &[DrawItem], level: usize, opts: &RenderOpts, masks: &mut fx::MaskCache) {
    let mut i = 0;
    while i < items.len() {
        let Some(g) = items[i].fx.get(level).cloned() else {
            draw_item(pix, &items[i], opts, masks);
            i += 1;
            continue;
        };
        let mut j = i + 1;
        while j < items.len() && items[j].fx.get(level).map(|h| std::sync::Arc::ptr_eq(h, &g)).unwrap_or(false) {
            j += 1;
        }
        fx::draw_group(pix, &items[i..j], level, &g, opts, masks);
        i = j;
    }
}

fn draw_item(pix: &mut sk::PixmapMut, it: &DrawItem, opts: &RenderOpts, masks: &mut fx::MaskCache) {
    let Some(path) = to_sk_path(&it.path) else { return };
    let m = opts.view.concat(&it.matrix);
    let ts = to_sk_transform(&m);
    let alpha = it.alpha * opts.alpha;
    let tint = opts.tint.or(it.tint);
    // onion skin's tint replaces colour effects
    let cx = if opts.tint.is_some() { None } else { it.cx };
    let shade = |c: Color, a: f32, t: Option<(Color, f32)>| shade_cx(c, a, t, cx);
    let mask = if it.clips.is_empty() { None } else { masks.get(&it.clips, &opts.view, pix.width(), pix.height()) };
    let mask = mask.as_deref();
    if !it.clips.is_empty() && mask.is_none() {
        return; // clipped away entirely
    }
    if let Some(oc) = it.outline {
        // Outline mode: thin hairline in the layer colour.
        let mut paint = sk::Paint::default();
        paint.set_color(shade(oc, alpha, None));
        paint.anti_alias = true;
        let stroke = sk::Stroke { width: 1.0, ..Default::default() };
        if let Some(p) = path.clone().transform(ts) {
            pix.stroke_path(&p, &paint, &stroke, sk::Transform::identity(), mask);
        }
        return;
    }
    if let Some(img) = &it.image {
        if let Some(src) = sk::PixmapRef::from_bytes(&img.rgba, img.width, img.height) {
            let q = if img.smoothing { sk::FilterQuality::Bilinear } else { sk::FilterQuality::Nearest };
            let paint = sk::Paint { shader: sk::Pattern::new(src, sk::SpreadMode::Pad, q, alpha.clamp(0.0, 1.0), sk::Transform::identity()), anti_alias: true, ..Default::default() };
            pix.fill_path(&path, &paint, sk::FillRule::Winding, ts, mask);
        }
    } else if let Some((img, pm)) = it.pattern.as_ref().filter(|_| opts.tint.is_none()) {
        // bitmap fill: the library bitmap tiled across the shape
        if let Some(src) = sk::PixmapRef::from_bytes(&img.rgba, img.width, img.height) {
            let q = if img.smoothing { sk::FilterQuality::Bilinear } else { sk::FilterQuality::Nearest };
            let paint = sk::Paint { shader: sk::Pattern::new(src, sk::SpreadMode::Repeat, q, alpha.clamp(0.0, 1.0), to_sk_transform(pm)), anti_alias: true, ..Default::default() };
            pix.fill_path(&path, &paint, sk::FillRule::EvenOdd, ts, mask);
        }
    } else if let Some(g) = &it.gradient {
        let mut paint = sk::Paint { anti_alias: true, ..Default::default() };
        let stops: Vec<sk::GradientStop> = g.stops.iter().map(|(t, c)| sk::GradientStop::new(t.clamp(0.0, 1.0), shade(*c, alpha, tint))).collect();
        let gts = to_sk_transform(&g.matrix); // tiny-skia applies the fill transform to the shader too
        let spread = match g.spread {
            ac_core::Spread::Pad => sk::SpreadMode::Pad,
            ac_core::Spread::Reflect => sk::SpreadMode::Reflect,
            ac_core::Spread::Repeat => sk::SpreadMode::Repeat,
        };
        let shader = match g.kind {
            ac_core::GradientKind::Linear => sk::LinearGradient::new(sk::Point::from_xy(-1.0, 0.0), sk::Point::from_xy(1.0, 0.0), stops, spread, gts),
            ac_core::GradientKind::Radial => sk::RadialGradient::new(sk::Point::from_xy(g.focal.clamp(-0.99, 0.99), 0.0), sk::Point::from_xy(0.0, 0.0), 1.0, stops, spread, gts),
        };
        match shader {
            Some(sh) => {
                paint.shader = sh;
                pix.fill_path(&path, &paint, sk::FillRule::EvenOdd, ts, mask);
            }
            None => {
                if let Some(c) = g.stops.first().map(|s| s.1).or(it.fill) {
                    paint.set_color(shade(c, alpha, tint));
                    pix.fill_path(&path, &paint, sk::FillRule::EvenOdd, ts, mask);
                }
            }
        }
    } else if let Some(fill) = it.fill {
        let mut paint = sk::Paint::default();
        paint.set_color(shade(fill, alpha, tint));
        paint.anti_alias = true;
        pix.fill_path(&path, &paint, sk::FillRule::EvenOdd, ts, mask);
    }
    if let Some(st) = it.stroke {
        let mut paint = sk::Paint::default();
        paint.set_color(shade(st.color, alpha, tint));
        paint.anti_alias = true;
        pix.stroke_path(&path, &paint, &sk_stroke(st.width, &it.stroke_style), ts, mask);
    }
}

/// tiny-skia stroke for a width + Animate stroke style (dashes, caps, joins).
fn sk_stroke(width: f32, st: &ac_core::StrokeStyle) -> sk::Stroke {
    use ac_core::{Cap, Dash, Join};
    let w = width.max(0.1);
    let line_cap = match (st.dash, st.cap) {
        (Dash::Dotted, _) | (_, Cap::Round) => sk::LineCap::Round,
        (_, Cap::None) => sk::LineCap::Butt,
        (_, Cap::Square) => sk::LineCap::Square,
    };
    let line_join = match st.join {
        Join::Miter => sk::LineJoin::Miter,
        Join::Round => sk::LineJoin::Round,
        Join::Bevel => sk::LineJoin::Bevel,
    };
    let dash = match st.dash {
        Dash::Dashed => sk::StrokeDash::new(vec![(st.dash_len * w).max(0.5), (st.gap * w).max(0.5)], 0.0),
        Dash::Dotted => sk::StrokeDash::new(vec![0.01, (st.gap * w).max(w * 1.2)], 0.0),
        _ => None,
    };
    sk::Stroke { width: w, line_cap, line_join, dash, ..Default::default() }
}

/// Render a scene frame at `scale` into an RGBA pixmap (stage-sized, clipped).
pub fn render_frame(doc: &Document, frame: u32, scale: f32) -> sk::Pixmap {
    render_frame_opts(doc, frame, scale, false)
}

/// Render a scene frame; `transparent` leaves the stage colour out (alpha output).
pub fn render_frame_opts(doc: &Document, frame: u32, scale: f32, transparent: bool) -> sk::Pixmap {
    let w = (doc.width * scale).round().max(1.0) as u32;
    let h = (doc.height * scale).round().max(1.0) as u32;
    let mut pix = sk::Pixmap::new(w, h).expect("pixmap");
    if !transparent {
        let bg = doc.background;
        pix.fill(sk::Color::from_rgba8(bg.r, bg.g, bg.b, 255));
    }
    let items = evaluate_scene(doc, frame);
    draw_items(&mut pix.as_mut(), &items, &RenderOpts { view: Matrix::scale(scale, scale), ..Default::default() });
    pix
}

/// Straight (un-premultiplied) RGBA bytes.
pub fn unpremultiplied(pix: &sk::Pixmap) -> Vec<u8> {
    pix.pixels().iter().flat_map(|p| {
        let c = p.demultiply();
        [c.red(), c.green(), c.blue(), c.alpha()]
    }).collect()
}

pub fn export_png(doc: &Document, frame: u32, scale: f32, path: &std::path::Path) -> Result<(), String> {
    render_frame(doc, frame, scale).save_png(path).map_err(|e| e.to_string())
}

/// File ▸ Export ▸ Export Image: the format of a single-frame export.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StillFormat {
    /// `transparent`: keep the stage transparent.
    Png { transparent: bool },
    /// Quality 1–100; `progressive` is accepted for parity but written baseline.
    Jpeg { quality: u8 },
    /// `colors`: palette size 2–256; `transparent`: stage colour becomes transparent.
    Gif { colors: u16, transparent: bool },
}

/// Export one frame as PNG, JPEG or GIF (Export Image).
pub fn export_still(doc: &Document, frame: u32, scale: f32, format: StillFormat, path: &std::path::Path) -> Result<(), String> {
    let bytes = still_bytes(doc, frame, scale, format)?;
    std::fs::write(path, bytes).map_err(|e| e.to_string())
}

/// The encoded bytes of `export_still`.
pub fn still_bytes(doc: &Document, frame: u32, scale: f32, format: StillFormat) -> Result<Vec<u8>, String> {
    match format {
        StillFormat::Png { transparent } => render_frame_opts(doc, frame, scale, transparent).encode_png().map_err(|e| e.to_string()),
        StillFormat::Jpeg { quality } => {
            let pix = render_frame_opts(doc, frame, scale, false);
            let rgb: Vec<u8> = unpremultiplied(&pix).chunks(4).flat_map(|c| [c[0], c[1], c[2]]).collect();
            let mut out = Vec::new();
            let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality.clamp(1, 100));
            use image::ImageEncoder;
            enc.write_image(&rgb, pix.width(), pix.height(), image::ExtendedColorType::Rgb8).map_err(|e| e.to_string())?;
            Ok(out)
        }
        StillFormat::Gif { colors, transparent } => {
            let pix = render_frame_opts(doc, frame, scale, transparent);
            let rgba = unpremultiplied(&pix);
            let (w, h) = (pix.width() as u16, pix.height() as u16);
            let n = colors.clamp(2, 256) as usize;
            // NeuQuant on the opaque pixels; transparent pixels get their own index
            let opaque: Vec<u8> = rgba.chunks(4).filter(|c| c[3] >= 128).flatten().copied().collect();
            let slots = if transparent { n - 1 } else { n };
            let nq = color_quant::NeuQuant::new(10, slots.max(2), if opaque.is_empty() { &[0, 0, 0, 255] } else { &opaque });
            let mut palette: Vec<u8> = nq.color_map_rgb();
            palette.truncate(slots.max(2) * 3);
            let tindex = transparent.then_some((palette.len() / 3) as u8);
            if transparent {
                palette.extend_from_slice(&[0, 0, 0]);
            }
            let indices: Vec<u8> = rgba.chunks(4).map(|c| if c[3] < 128 { tindex.unwrap_or(nq.index_of(c) as u8) } else { nq.index_of(c) as u8 }).collect();
            let mut out = Vec::new();
            {
                let mut enc = gif::Encoder::new(&mut out, w, h, &palette).map_err(|e| e.to_string())?;
                let mut fr = gif::Frame::from_indexed_pixels(w, h, indices, tindex);
                fr.palette = None;
                enc.write_frame(&fr).map_err(|e| e.to_string())?;
            }
            Ok(out)
        }
    }
}

/// Options shared by the multi-frame exporters (Publish Settings).
#[derive(Clone, Debug)]
pub struct ExportOpts {
    pub scale: f32,
    /// Frame range, 0-based inclusive; `None` = whole timeline.
    pub range: Option<(u32, u32)>,
    pub transparent: bool,
    /// GIF: 0 = loop forever, n = play n times.
    pub loops: u16,
    /// GIF quantiser speed 1 (best) … 30 (fastest).
    pub gif_speed: i32,
}

impl Default for ExportOpts {
    fn default() -> Self {
        ExportOpts { scale: 1.0, range: None, transparent: false, loops: 0, gif_speed: 10 }
    }
}

impl ExportOpts {
    pub fn frames(&self, doc: &Document) -> std::ops::RangeInclusive<u32> {
        let n = doc.total_frames();
        let (a, b) = self.range.unwrap_or((0, n - 1));
        a.min(n - 1)..=b.min(n - 1).max(a.min(n - 1))
    }
}

/// Progress callback: (frames done, total). Return false to cancel.
pub type Progress<'a> = &'a mut dyn FnMut(u32, u32) -> bool;

pub fn export_gif(doc: &Document, scale: f32, path: &std::path::Path) -> Result<(), String> {
    export_gif_opts(doc, &ExportOpts { scale, ..Default::default() }, path, &mut |_, _| true)
}

pub fn export_gif_opts(doc: &Document, o: &ExportOpts, path: &std::path::Path, progress: Progress) -> Result<(), String> {
    let first = render_frame_opts(doc, 0, o.scale, o.transparent);
    let (w, h) = (first.width() as u16, first.height() as u16);
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut enc = gif::Encoder::new(file, w, h, &[]).map_err(|e| e.to_string())?;
    enc.set_repeat(if o.loops == 0 { gif::Repeat::Infinite } else { gif::Repeat::Finite(o.loops.saturating_sub(1)) }).map_err(|e| e.to_string())?;
    let delay = (100.0 / doc.fps).round() as u16;
    let frames = o.frames(doc);
    let total = frames.end() - frames.start() + 1;
    for (i, f) in frames.enumerate() {
        let pix = render_frame_opts(doc, f, o.scale, o.transparent);
        let mut rgba = unpremultiplied(&pix);
        let mut frame = gif::Frame::from_rgba_speed(w, h, &mut rgba, o.gif_speed.clamp(1, 30));
        frame.delay = delay;
        if o.transparent {
            frame.dispose = gif::DisposalMethod::Background;
        }
        enc.write_frame(&frame).map_err(|e| e.to_string())?;
        if !progress(i as u32 + 1, total) {
            return Err("cancelled".into());
        }
    }
    Ok(())
}

/// One PNG per frame: `<dir>/<prefix>0001.png` … Returns the number written.
pub fn export_png_sequence(doc: &Document, o: &ExportOpts, dir: &std::path::Path, prefix: &str, progress: Progress) -> Result<u32, String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let frames = o.frames(doc);
    let total = frames.end() - frames.start() + 1;
    for (i, f) in frames.enumerate() {
        let p = dir.join(format!("{prefix}{:04}.png", f + 1));
        render_frame_opts(doc, f, o.scale, o.transparent).save_png(&p).map_err(|e| format!("{}: {e}", p.display()))?;
        if !progress(i as u32 + 1, total) {
            return Err("cancelled".into());
        }
    }
    Ok(total)
}

/// Is an ffmpeg binary available (`ffmpeg` on PATH, or `exe`)?
pub fn ffmpeg_version(exe: &str) -> Option<String> {
    let out = std::process::Command::new(exe).arg("-version").output().ok()?;
    String::from_utf8_lossy(&out.stdout).lines().next().map(|l| l.to_string())
}

/// Encode a video with ffmpeg (MP4/H.264, WebM/VP9, MOV/ProRes 4444 with
/// alpha, or GIF). Frames are piped as raw RGBA, so nothing touches disk.
pub fn export_video(doc: &Document, o: &ExportOpts, path: &std::path::Path, ffmpeg: &str, progress: Progress) -> Result<(), String> {
    use std::io::Write;
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let first = render_frame_opts(doc, 0, o.scale, o.transparent);
    let (w, h) = (first.width(), first.height());
    let codec: Vec<&str> = match ext.as_str() {
        "webm" => vec!["-c:v", "libvpx-vp9", "-b:v", "0", "-crf", "30", "-pix_fmt", if o.transparent { "yuva420p" } else { "yuv420p" }],
        "mov" => vec!["-c:v", "prores_ks", "-profile:v", "4444", "-pix_fmt", "yuva444p10le"],
        "gif" => vec![],
        // H.264 needs even dimensions and no alpha
        _ => vec!["-c:v", "libx264", "-preset", "medium", "-crf", "18", "-pix_fmt", "yuv420p", "-vf", "pad=ceil(iw/2)*2:ceil(ih/2)*2", "-movflags", "+faststart"],
    };
    let size = format!("{w}x{h}");
    let fps = format!("{}", doc.fps);
    let frames = o.frames(doc);
    // the movie's sound (keyframe sounds mixed as Animate plays them)
    let wav = if ac_audio::has_sound(doc) && ext != "gif" {
        let samples = ac_audio::mix(doc, *frames.start(), 48000);
        let n = ((frames.end() - frames.start() + 1) as f64 / doc.fps.max(1.0) as f64 * 48000.0) as usize * 2;
        let p = std::env::temp_dir().join(format!("flare-audio-{}.wav", std::process::id()));
        std::fs::write(&p, ac_audio::wav(&samples[..n.min(samples.len())], 48000)).map_err(|e| e.to_string())?;
        Some(p)
    } else {
        None
    };
    let mut cmd = std::process::Command::new(ffmpeg);
    cmd.args(["-y", "-hide_banner", "-loglevel", "error", "-f", "rawvideo", "-pix_fmt", "rgba", "-s", &size, "-r", &fps, "-i", "-"]);
    if let Some(w) = &wav {
        cmd.arg("-i").arg(w);
    }
    cmd.args(&codec);
    if wav.is_some() {
        let acodec: &[&str] = match ext.as_str() {
            "webm" => &["-c:a", "libopus", "-b:a", "160k"],
            "mov" => &["-c:a", "pcm_s16le"],
            _ => &["-c:a", "aac", "-b:a", "192k"],
        };
        cmd.args(acodec).args(["-map", "0:v", "-map", "1:a"]);
    }
    cmd.arg(path);
    cmd.stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| format!("couldn't start ffmpeg ({e}). Install ffmpeg or set its path in Publish Settings"))?;
    let total = frames.end() - frames.start() + 1;
    {
        let stdin = child.stdin.as_mut().ok_or("no ffmpeg stdin")?;
        for (i, f) in frames.enumerate() {
            let pix = render_frame_opts(doc, f, o.scale, o.transparent);
            if stdin.write_all(&unpremultiplied(&pix)).is_err() {
                break; // ffmpeg exited; its stderr says why
            }
            if !progress(i as u32 + 1, total) {
                let _ = child.kill();
                return Err("cancelled".into());
            }
        }
    }
    drop(child.stdin.take());
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if let Some(w) = wav {
        let _ = std::fs::remove_file(w);
    }
    if out.status.success() {
        Ok(())
    } else {
        Err(format!("ffmpeg failed: {}", String::from_utf8_lossy(&out.stderr).trim()))
    }
}

fn svg_path(p: &Path) -> String {
    let mut s = String::new();
    for seg in &p.segs {
        use std::fmt::Write;
        let _ = match *seg {
            Seg::M(a) => write!(s, "M{:.2} {:.2}", a.x, a.y),
            Seg::L(a) => write!(s, "L{:.2} {:.2}", a.x, a.y),
            Seg::Q(c, a) => write!(s, "Q{:.2} {:.2} {:.2} {:.2}", c.x, c.y, a.x, a.y),
            Seg::C(c1, c2, a) => write!(s, "C{:.2} {:.2} {:.2} {:.2} {:.2} {:.2}", c1.x, c1.y, c2.x, c2.y, a.x, a.y),
            Seg::Z => write!(s, "Z"),
        };
    }
    s
}

/// Export one frame as a standalone SVG.
pub fn export_svg(doc: &Document, frame: u32) -> String {
    let mut out = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:xlink=\"http://www.w3.org/1999/xlink\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\">\n<rect width=\"100%\" height=\"100%\" fill=\"{bg}\"/>\n",
        w = doc.width,
        h = doc.height,
        bg = doc.background.to_hex()
    );
    let items = evaluate_scene(doc, frame);
    let mut w = SvgWriter { doc, defs: String::new(), next: 0, clip_ids: Default::default(), image_ids: Default::default() };
    let body = w.level(&items, 0);
    if !w.defs.is_empty() {
        out += &format!("<defs>\n{}</defs>\n", w.defs);
    }
    out + &body + "</svg>\n"
}

/// Animated SVG: every frame as a group shown for its time with SMIL
/// (plays in browsers without script). Identical frames share one group.
pub fn export_animated_svg(doc: &Document, o: &ExportOpts) -> String {
    let frames = o.frames(doc);
    let n = frames.end() - frames.start() + 1;
    let dur = n as f32 / doc.fps.max(1.0);
    // keep every frame's items alive: the writer keys clips/images by pointer
    let all: Vec<Vec<DrawItem>> = frames.clone().map(|f| evaluate_scene(doc, f)).collect();
    let mut w = SvgWriter { doc, defs: String::new(), next: 0, clip_ids: Default::default(), image_ids: Default::default() };
    let mut bodies: Vec<String> = vec![];
    let mut shown: Vec<Vec<u32>> = vec![];
    for (i, items) in all.iter().enumerate() {
        let body = w.level(items, 0);
        match bodies.iter().position(|b| *b == body) {
            Some(k) => shown[k].push(i as u32),
            None => {
                bodies.push(body);
                shown.push(vec![i as u32]);
            }
        }
    }
    let mut out = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:xlink=\"http://www.w3.org/1999/xlink\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\">\n",
        w = doc.width,
        h = doc.height
    );
    if !o.transparent {
        out += &format!("<rect width=\"100%\" height=\"100%\" fill=\"{}\"/>\n", doc.background.to_hex());
    }
    if !w.defs.is_empty() {
        out += &format!("<defs>\n{}</defs>\n", w.defs);
    }
    let repeat = if o.loops == 0 { "indefinite".to_string() } else { o.loops.to_string() };
    for (body, fr) in bodies.iter().zip(&shown) {
        if fr.len() as u32 == n {
            out += &format!("<g>\n{body}</g>\n");
            continue;
        }
        // discrete visibility: on for each frame listed, off otherwise
        let mut times = vec![];
        let mut vals = vec![];
        let mut on = false;
        for i in 0..n {
            let want = fr.contains(&i);
            if i == 0 || want != on {
                times.push(format!("{:.5}", i as f32 / n as f32));
                vals.push(if want { "inline" } else { "none" });
                on = want;
            }
        }
        let first = if fr.contains(&0) { "inline" } else { "none" };
        out += &format!(
            "<g display=\"{first}\"><animate attributeName=\"display\" calcMode=\"discrete\" dur=\"{dur:.4}s\" repeatCount=\"{repeat}\" keyTimes=\"{}\" values=\"{}\" fill=\"freeze\"/>\n{body}</g>\n",
            times.join(";"),
            vals.join(";")
        );
    }
    out + "</svg>\n"
}

struct SvgWriter<'a> {
    doc: &'a Document,
    defs: String,
    next: usize,
    clip_ids: std::collections::HashMap<usize, String>,
    image_ids: std::collections::HashMap<usize, String>,
}

fn svg_matrix(m: &Matrix) -> String {
    format!("matrix({:.4} {:.4} {:.4} {:.4} {:.2} {:.2})", m.a, m.b, m.c, m.d, m.tx, m.ty)
}

impl SvgWriter<'_> {
    fn id(&mut self, p: &str) -> String {
        self.next += 1;
        format!("{p}{}", self.next)
    }
    fn level(&mut self, items: &[DrawItem], level: usize) -> String {
        let mut body = String::new();
        let mut i = 0;
        while i < items.len() {
            match items[i].fx.get(level).cloned() {
                None => {
                    body += &self.item(&items[i]);
                    i += 1;
                }
                Some(g) => {
                    let mut j = i + 1;
                    while j < items.len() && items[j].fx.get(level).map(|h| std::sync::Arc::ptr_eq(h, &g)).unwrap_or(false) {
                        j += 1;
                    }
                    if g.mesh.is_some() {
                        // Asset Warp has no vector form: the warped group goes in as a picture
                        body += &self.raster_group(&items[i..j], level);
                        i = j;
                        continue;
                    }
                    let inner = self.level(&items[i..j], level + 1);
                    let mut attrs = String::new();
                    if let Some(f) = self.filter(&g.filters) {
                        attrs += &format!(" filter=\"url(#{f})\"");
                    }
                    let css = match g.blend {
                        ac_core::BlendMode::Multiply => "multiply",
                        ac_core::BlendMode::Screen => "screen",
                        ac_core::BlendMode::Overlay => "overlay",
                        ac_core::BlendMode::Darken => "darken",
                        ac_core::BlendMode::Lighten => "lighten",
                        ac_core::BlendMode::HardLight => "hard-light",
                        ac_core::BlendMode::Difference => "difference",
                        _ => "",
                    };
                    if !css.is_empty() {
                        attrs += &format!(" style=\"mix-blend-mode:{css}\"");
                    }
                    body += &format!("<g{attrs}>\n{inner}</g>\n");
                    i = j;
                }
            }
        }
        body
    }
    /// Items drawn through the rasteriser (from effect level `level` down) and
    /// embedded as a PNG at 2× over the area they cover.
    fn raster_group(&mut self, items: &[DrawItem], level: usize) -> String {
        const S: f32 = 2.0;
        let (w, h) = ((self.doc.width * S).round().max(1.0) as u32, (self.doc.height * S).round().max(1.0) as u32);
        let Some(mut pix) = sk::Pixmap::new(w, h) else { return String::new() };
        let own: Vec<DrawItem> = items.iter().map(|it| DrawItem { fx: it.fx[level.min(it.fx.len())..].to_vec(), ..it.clone() }).collect();
        draw_items(&mut pix.as_mut(), &own, &RenderOpts { view: Matrix::scale(S, S), ..Default::default() });
        // crop to the drawn pixels
        let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0u32, 0u32);
        for (k, p) in pix.pixels().iter().enumerate() {
            if p.alpha() > 0 {
                let (x, y) = (k as u32 % w, k as u32 / w);
                (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x + 1), y1.max(y + 1));
            }
        }
        if x1 <= x0 || y1 <= y0 {
            return String::new();
        }
        let Some(crop) = pix.clone_rect(sk::IntRect::from_xywh(x0 as i32, y0 as i32, x1 - x0, y1 - y0).unwrap()) else { return String::new() };
        let Ok(png) = crop.encode_png() else { return String::new() };
        format!(
            "<image x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" xlink:href=\"data:image/png;base64,{}\"/>\n",
            x0 as f32 / S,
            y0 as f32 / S,
            (x1 - x0) as f32 / S,
            (y1 - y0) as f32 / S,
            base64_lite::encode(&png)
        )
    }
    /// SVG filter for blur / drop shadow / glow (others render without effect).
    fn filter(&mut self, fs: &[ac_core::Filter]) -> Option<String> {
        let mut prims = String::new();
        for f in fs {
            match f {
                ac_core::Filter::Blur { blur_x, blur_y, .. } => prims += &format!("<feGaussianBlur stdDeviation=\"{:.2} {:.2}\"/>", blur_x / 2.0, blur_y / 2.0),
                ac_core::Filter::DropShadow { blur_x, blur_y, strength, angle, distance, color, .. } => {
                    let (dx, dy) = (angle.to_radians().cos() * distance, angle.to_radians().sin() * distance);
                    prims += &format!("<feDropShadow dx=\"{dx:.2}\" dy=\"{dy:.2}\" stdDeviation=\"{:.2} {:.2}\" flood-color=\"{}\" flood-opacity=\"{:.2}\"/>", blur_x / 2.0, blur_y / 2.0, color.to_hex(), (strength / 100.0).min(1.0));
                }
                ac_core::Filter::Glow { blur_x, blur_y, strength, color, .. } => {
                    prims += &format!("<feDropShadow dx=\"0\" dy=\"0\" stdDeviation=\"{:.2} {:.2}\" flood-color=\"{}\" flood-opacity=\"{:.2}\"/>", blur_x / 2.0, blur_y / 2.0, color.to_hex(), (strength / 100.0).min(1.0));
                }
                _ => {}
            }
        }
        if prims.is_empty() {
            return None;
        }
        let id = self.id("f");
        self.defs += &format!("<filter id=\"{id}\" x=\"-50%\" y=\"-50%\" width=\"200%\" height=\"200%\">{prims}</filter>\n");
        Some(id)
    }
    fn clip(&mut self, c: &std::sync::Arc<ac_core::Clip>) -> String {
        let k = std::sync::Arc::as_ptr(c) as usize;
        if let Some(id) = self.clip_ids.get(&k) {
            return id.clone();
        }
        let id = self.id("c");
        self.defs += &format!("<clipPath id=\"{id}\">");
        for (p, m) in &c.shapes {
            self.defs += &format!("<path transform=\"{}\" d=\"{}\"/>", svg_matrix(m), svg_path(p));
        }
        self.defs += "</clipPath>\n";
        self.clip_ids.insert(k, id.clone());
        id
    }
    fn item(&mut self, it: &DrawItem) -> String {
        let m = it.matrix;
        let col = |c: Color| {
            let c = match it.tint { Some((t, a)) => c.lerp(t, a), None => c };
            let c = it.cx.map(|x| x.apply(c)).unwrap_or(c);
            (c.to_hex(), c.a as f32 / 255.0 * it.alpha)
        };
        let shape = if let (Some(img), Some((lo, hi))) = (&it.image, it.path.bounds()) {
            // find the asset with these pixels to embed its PNG
            let key = std::sync::Arc::as_ptr(img) as usize;
            let png = self.doc.bitmaps.iter().find(|b| b.image().map(|i| std::sync::Arc::ptr_eq(&i, img)).unwrap_or(false)).map(|b| b.png.clone());
            let id = match (self.image_ids.get(&key).cloned(), png) {
                (Some(id), _) => Some(id),
                (None, Some(png)) => {
                    use base64_lite::encode;
                    let id = self.id("i");
                    self.defs += &format!("<image id=\"{id}\" width=\"{}\" height=\"{}\" xlink:href=\"data:image/png;base64,{}\"/>\n", hi.x - lo.x, hi.y - lo.y, encode(&png));
                    self.image_ids.insert(key, id.clone());
                    Some(id)
                }
                _ => None,
            };
            match id {
                Some(id) => format!("<use transform=\"{}\" xlink:href=\"#{id}\" opacity=\"{:.3}\"", svg_matrix(&m), it.alpha),
                None => return String::new(),
            }
        } else {
            let fill = match (&it.gradient, it.fill) {
                (Some(g), _) => {
                    let gid = self.id("g");
                    let tag = if g.kind == ac_core::GradientKind::Linear { "linearGradient" } else { "radialGradient" };
                    let geo = if g.kind == ac_core::GradientKind::Linear { "x1=\"-1\" y1=\"0\" x2=\"1\" y2=\"0\"".to_string() } else if g.focal != 0.0 { format!("cx=\"0\" cy=\"0\" r=\"1\" fx=\"{:.4}\" fy=\"0\"", g.focal.clamp(-0.99, 0.99)) } else { "cx=\"0\" cy=\"0\" r=\"1\"".to_string() };
                    let spread = match g.spread {
                        ac_core::Spread::Pad => "",
                        ac_core::Spread::Reflect => " spreadMethod=\"reflect\"",
                        ac_core::Spread::Repeat => " spreadMethod=\"repeat\"",
                    };
                    self.defs += &format!("<{tag} id=\"{gid}\" gradientUnits=\"userSpaceOnUse\" {geo}{spread} gradientTransform=\"{}\">", svg_matrix(&g.matrix));
                    for (t, c) in &g.stops {
                        let (h, a) = col(*c);
                        self.defs += &format!("<stop offset=\"{t:.3}\" stop-color=\"{h}\" stop-opacity=\"{a:.3}\"/>");
                    }
                    self.defs += &format!("</{tag}>\n");
                    format!("fill=\"url(#{gid})\" fill-rule=\"evenodd\"")
                }
                (None, Some(c)) => {
                    let (h, a) = col(c);
                    format!("fill=\"{h}\" fill-opacity=\"{a:.3}\" fill-rule=\"evenodd\"")
                }
                (None, None) => "fill=\"none\"".into(),
            };
            let stroke = match it.stroke {
                Some(s) => {
                    let (h, a) = col(s.color);
                    {
                        use ac_core::{Cap, Dash, Join};
                        let st = &it.stroke_style;
                        let cap = match (st.dash, st.cap) {
                            (Dash::Dotted, _) | (_, Cap::Round) => "round",
                            (_, Cap::None) => "butt",
                            (_, Cap::Square) => "square",
                        };
                        let join = match st.join {
                            Join::Miter => "miter",
                            Join::Round => "round",
                            Join::Bevel => "bevel",
                        };
                        let dash = match st.dash {
                            Dash::Dashed => format!(" stroke-dasharray=\"{} {}\"", st.dash_len * s.width, st.gap * s.width),
                            Dash::Dotted => format!(" stroke-dasharray=\"0.01 {}\"", (st.gap * s.width).max(s.width * 1.2)),
                            _ => String::new(),
                        };
                        format!(" stroke=\"{h}\" stroke-opacity=\"{a:.3}\" stroke-width=\"{}\" stroke-linecap=\"{cap}\" stroke-linejoin=\"{join}\"{dash}", s.width)
                    }
                }
                None => String::new(),
            };
            format!("<path transform=\"{}\" d=\"{}\" {}{}", svg_matrix(&m), svg_path(&it.path), fill, stroke)
        };
        let mut s = shape + "/>\n";
        // nested clip groups (clip paths are in the parent space, so wrap)
        for c in it.clips.clone() {
            let id = self.clip(&c);
            s = format!("<g clip-path=\"url(#{id})\">{s}</g>\n");
        }
        s
    }
}

mod base64_lite {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    pub fn encode(b: &[u8]) -> String {
        let mut o = String::with_capacity(b.len().div_ceil(3) * 4);
        for c in b.chunks(3) {
            let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
            o.push(T[(n >> 18) as usize & 63] as char);
            o.push(T[(n >> 12) as usize & 63] as char);
            o.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
            o.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
        }
        o
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_demo_frames() {
        let doc = ac_core::demo::demo_document();
        let a = render_frame(&doc, 0, 1.0);
        let b = render_frame(&doc, 9, 1.0);
        assert_eq!((a.width(), a.height()), (550, 400));
        assert_ne!(a.data(), b.data(), "animation changes pixels");
        // ground is green at the bottom-left
        let p = a.pixel(5, 395).unwrap().demultiply();
        assert!(p.green() > p.red() && p.green() > p.blue());
    }

    #[test]
    fn gradient_follows_object_and_view() {
        use ac_core::{Element, Gradient, GradientKind, Shape};
        let mut doc = Document { width: 200.0, height: 200.0, ..Default::default() };
        let mut sh = Shape::new(Path::rect(-40.0, -40.0, 80.0, 80.0), Some(Color::rgb(0, 0, 255)), None);
        sh.gradient = Some(Gradient::fit(GradientKind::Radial, vec![(0.0, Color::rgb(0, 0, 255)), (1.0, Color::rgb(255, 255, 255))], ac_core::pt(-40.0, -40.0), ac_core::pt(40.0, 40.0)));
        let mut el = Element::shape("g", sh);
        el.matrix = Matrix::translate(100.0, 100.0);
        doc.scene.layers[0].keyframes.insert(0, ac_core::Keyframe::with(vec![el]));
        for scale in [1.0, 2.0] {
            let pix = render_frame(&doc, 0, scale);
            let c = pix.pixel((100.0 * scale) as u32, (100.0 * scale) as u32).unwrap().demultiply();
            assert!(c.blue() > 200 && c.red() < 60, "centre is the first stop at scale {scale}: {c:?}");
            let e = pix.pixel((139.0 * scale) as u32, (139.0 * scale) as u32).unwrap().demultiply();
            assert!(e.red() > 150, "corner is near the last stop at scale {scale}: {e:?}");
        }
    }

    #[test]
    fn png_sequence_and_ranges() {
        let doc = ac_core::demo::demo_document();
        let dir = std::env::temp_dir().join(format!("ac-seq-{}", std::process::id()));
        let o = ExportOpts { scale: 0.25, range: Some((2, 4)), transparent: true, ..Default::default() };
        let n = export_png_sequence(&doc, &o, &dir, "f", &mut |_, _| true).unwrap();
        assert_eq!(n, 3);
        assert!(dir.join("f0003.png").exists() && dir.join("f0005.png").exists() && !dir.join("f0006.png").exists());
        let img = sk::Pixmap::load_png(dir.join("f0003.png")).unwrap();
        assert!(img.pixels().iter().any(|p| p.alpha() == 0), "transparent background");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn video_via_ffmpeg_when_available() {
        if ffmpeg_version("ffmpeg").is_none() {
            return; // ffmpeg isn't installed here
        }
        let doc = ac_core::demo::demo_document();
        let p = std::env::temp_dir().join(format!("ac-video-{}.mp4", std::process::id()));
        let o = ExportOpts { scale: 0.25, range: Some((0, 5)), ..Default::default() };
        let mut calls = 0;
        export_video(&doc, &o, &p, "ffmpeg", &mut |_, _| { calls += 1; true }).unwrap();
        assert_eq!(calls, 6);
        assert!(std::fs::metadata(&p).unwrap().len() > 500);
        let _ = std::fs::remove_file(p);
    }

    #[test]
    fn svg_is_well_formed_ish() {
        let doc = ac_core::demo::demo_document();
        let s = export_svg(&doc, 12);
        assert!(s.starts_with("<svg") && s.trim_end().ends_with("</svg>"));
        assert!(s.matches("<path").count() > 10);
    }
}
