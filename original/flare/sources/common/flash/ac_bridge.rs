//! Bridge between the AnimationCraft crates (ac-core document model, ac-xfl,
//! ac-render, ac-audio) and the Flare C++ app. Pure Rust core (`import_to_dir`,
//! `doc_from_manifest`, `export_xfl`, `export_html5`, ...) plus the `extern "C"`
//! wrappers in flare_formats.rs. Everything crossing the C ABI goes through
//! files and plain strings so the C++ side needs no knowledge of ac-core.
//!
//! Import (XFL/FLA -> Flare scene): `import_to_dir` writes into `out_dir`
//!   frames.tsv  `layer \t name \t row \t file`  (one SVG per drawing/tween frame)
//!   sound.tsv   `row \t file \t startOffsetFrames \t endOffsetFrames`
//!   meta.tsv    `width \t height \t fps \t frames`
//! Export (Flare scene -> XFL / HTML5): the app writes a manifest
//!   doc \t W \t H \t FPS
//!   cell \t col \t name \t row \t count \t path
//!   sound \t row \t path \t inFrames \t outFrames
//! and `export_xfl` / `export_html5` rebuild an ac Document from it.

use ac_core::model::*;
use ac_core::sound::{FrameSound, SoundAsset, SoundSync};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// Hard caps so a corrupt or hostile document cannot fill the disk.
const MAX_FILES: usize = 30_000;
const MAX_ROWS: u32 = 100_000;

fn clean(s: &str) -> String {
    s.chars().map(|c| if c == '\t' || c == '\n' || c == '\r' { ' ' } else { c }).collect()
}

fn strip_background(svg: &str) -> String {
    // export_svg paints an opaque stage rect first; Flare levels must be transparent.
    match svg.find("<rect width=\"100%\" height=\"100%\"") {
        Some(a) => match svg[a..].find("/>") {
            Some(b) => format!("{}{}", &svg[..a], svg[a + b + 2..].trim_start_matches('\n')),
            None => svg.to_string(),
        },
        None => svg.to_string(),
    }
}

fn is_animated(kf: &Keyframe) -> bool {
    !matches!(kf.tween, Tween::None) || kf.motion.is_some()
}

/// Load an XFL/FLA (zipped, folder or DOMDocument/.xfl marker) and convert it to
/// per-layer SVG frames plus manifests in `out_dir`. Returns a log ("ok ...").
pub fn import_to_dir(path: &Path, out_dir: &Path) -> Result<String, String> {
    let imp = ac_xfl::open(path)?;
    let doc = imp.doc;
    std::fs::create_dir_all(out_dir).map_err(|e| format!("{}: {e}", out_dir.display()))?;
    let mut tsv = String::new();
    let (mut files, mut layers_out, mut layers_empty) = (0usize, 0usize, 0usize);
    let mut log = String::new();
    let mut col = 0usize;
    for (si, tl) in doc.scenes().enumerate() {
        let base = doc.scene_start(si);
        for (li, layer) in tl.layers.iter().enumerate() {
            if matches!(layer.kind, LayerKind::Guide | LayerKind::Folder | LayerKind::Camera) {
                continue;
            }
            if layer.keyframes.values().all(|k| k.elements.is_empty()) {
                layers_empty += 1;
                continue;
            }
            // Render the layer on its own: everything else hidden (a mask or folder
            // above it stays so masking still applies).
            let mut solo = doc.clone();
            solo.background.a = 0;
            for (sj, t) in solo.scenes_mut().enumerate() {
                for (lj, l) in t.layers.iter_mut().enumerate() {
                    let keep = sj == si && (lj == li || tl.layers[li].parent == Some(lj));
                    if !keep {
                        l.visible = false;
                    }
                }
            }
            let mut wrote = false;
            for (&start, kf) in &layer.keyframes {
                let end = layer.span_end(start).min(layer.end).min(start + MAX_ROWS);
                if kf.elements.is_empty() || end <= start {
                    continue;
                }
                let frames: Vec<u32> = if is_animated(kf) { (start..end).collect() } else { vec![start] };
                for f in frames {
                    files += 1;
                    if files > MAX_FILES {
                        return Err(format!("too many frames to convert (limit {MAX_FILES})"));
                    }
                    let svg = strip_background(&ac_render::export_svg(&solo, base + f));
                    let name = format!("L{col}_F{}.svg", base + f);
                    std::fs::write(out_dir.join(&name), svg).map_err(|e| format!("{name}: {e}"))?;
                    // a static drawing holds for its whole span; a tween frame for one row
                    let _ = writeln!(tsv, "{col}\t{}\t{}\t{name}", clean(&layer.name), base + f);
                    if !is_animated(kf) {
                        for r in start + 1..end {
                            let _ = writeln!(tsv, "{col}\t{}\t{}\t{name}", clean(&layer.name), base + r);
                        }
                    }
                    wrote = true;
                }
            }
            if wrote {
                layers_out += 1;
                col += 1;
            }
        }
    }
    std::fs::write(out_dir.join("frames.tsv"), tsv).map_err(|e| e.to_string())?;

    // Sounds: the decoded asset as WAV once, plus each voice with its in/out cut.
    let fps = doc.fps.max(1.0) as f64;
    let mut stsv = String::new();
    let mut sounds = 0usize;
    if ac_audio::has_sound(&doc) {
        let pcms: std::collections::HashMap<_, _> = doc.sounds.iter().filter_map(|s| ac_audio::pcm(s).map(|p| (s.id, p))).collect();
        let voices = ac_audio::schedule(&doc, &|id| pcms.get(&id).map(|p| p.seconds() as f64));
        for v in voices.iter() {
            let Some(p) = pcms.get(&v.sound) else { continue };
            let name = format!("sound_{}.wav", v.sound);
            if !out_dir.join(&name).exists() {
                std::fs::write(out_dir.join(&name), ac_audio::wav(&p.samples, p.rate)).map_err(|e| e.to_string())?;
            }
            let row = (v.start * fps).round() as i64;
            // a voice shorter than the asset is cut at the end (Stream span end / Stop key)
            let cut = ((p.seconds() as f64 - (v.end - v.start)).max(0.0) * fps).round() as i64;
            let _ = writeln!(stsv, "{row}\t{name}\t0\t{}", if v.planned.is_finite() && v.planned > p.seconds() as f64 { 0 } else { cut });
            sounds += 1;
        }
    }
    std::fs::write(out_dir.join("sound.tsv"), stsv).map_err(|e| e.to_string())?;
    std::fs::write(out_dir.join("meta.tsv"), format!("{}\t{}\t{}\t{}\n", doc.width, doc.height, doc.fps, doc.total_frames())).map_err(|e| e.to_string())?;
    log += &format!("ok {layers_out} layers {files} frames {sounds} sounds {}x{} @{}\n", doc.width, doc.height, doc.fps);
    if layers_empty > 0 {
        // Honest report (D21): the document does have layers, they just hold no drawable content.
        let _ = writeln!(log, "note: {layers_empty} layer(s) in the document contain only empty keyframes (no shapes, symbols or bitmaps), so no columns were created for them");
    }
    for w in imp.warnings.iter().chain(doc.warnings.iter()) {
        let _ = writeln!(log, "warning: {w}");
    }
    Ok(log)
}

fn rasterize_svg(bytes: &[u8], w: u32, h: u32) -> Result<Vec<u8>, String> {
    use resvg::{tiny_skia, usvg};
    let tree = usvg::Tree::from_data(bytes, &usvg::Options::default()).map_err(|e| e.to_string())?;
    let (w, h) = (w.clamp(1, 8192), h.clamp(1, 8192));
    let mut pm = tiny_skia::Pixmap::new(w, h).ok_or("bad size")?;
    let sz = tree.size();
    let t = tiny_skia::Transform::from_scale(w as f32 / sz.width().max(1.0), h as f32 / sz.height().max(1.0));
    resvg::render(&tree, t, &mut pm.as_mut());
    pm.encode_png().map_err(|e| e.to_string())
}

/// Rebuild a Document from the app's scene manifest (see module docs).
pub fn doc_from_manifest(text: &str) -> Result<Document, String> {
    let mut doc = Document {
        width: 1920.0, height: 1080.0, fps: 24.0, background: ac_core::model::Color::rgb(255, 255, 255),
        scene: Timeline { name: "Scene 1".into(), layers: vec![] }, library: vec![], next_symbol_id: 1,
        perspective_angle: 55.0, vanishing_point: None, bitmaps: vec![], guides: Default::default(),
        more_scenes: vec![], sounds: vec![], camera_mode: Default::default(), swatches: None, folders: vec![],
        linkage: Default::default(), swf_char_ids: Default::default(), assistants: vec![], warnings: vec![],
    };
    let mut cols: Vec<(usize, Layer)> = vec![];
    let mut by_path: std::collections::HashMap<String, u32> = Default::default();
    let mut sound_rows: Vec<(u32, String, u32, u32)> = vec![];
    // pass 1: doc line
    for line in text.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        if f.first() == Some(&"doc") && f.len() >= 4 {
            doc.width = f[1].parse().map_err(|_| "bad width")?;
            doc.height = f[2].parse().map_err(|_| "bad height")?;
            doc.fps = f[3].parse().map_err(|_| "bad fps")?;
        }
    }
    if !(doc.width >= 1.0 && doc.height >= 1.0 && doc.fps >= 1.0 && doc.width <= 16384.0 && doc.height <= 16384.0 && doc.fps <= 1000.0) {
        return Err("manifest: bad document size or frame rate".into());
    }
    for line in text.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        match f.first().copied() {
            Some("cell") if f.len() >= 6 => {
                let col: usize = f[1].parse().map_err(|_| "bad column")?;
                let row: u32 = f[3].parse().map_err(|_| "bad row")?;
                let count: u32 = f[4].parse().map_err(|_| "bad count")?;
                if row > MAX_ROWS || count == 0 || count > MAX_ROWS {
                    return Err("manifest: row/count out of range".into());
                }
                let path = f[5];
                let id = match by_path.get(path) {
                    Some(&id) => id,
                    None => {
                        let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
                        let name = Path::new(path).file_name().and_then(|s| s.to_str()).unwrap_or("image").to_string();
                        let ext = Path::new(path).extension().and_then(|s| s.to_str()).unwrap_or("").to_ascii_lowercase();
                        let mut asset = if ext == "svg" {
                            let png = rasterize_svg(&bytes, doc.width as u32, doc.height as u32)?;
                            BitmapAsset::from_png(&name, png)?
                        } else {
                            ac_core::codecs::bitmap_from_bytes(&name, bytes)?
                        };
                        asset.id = doc.next_bitmap_id();
                        let id = asset.id;
                        doc.bitmaps.push(asset);
                        by_path.insert(path.to_string(), id);
                        id
                    }
                };
                let idx = match cols.iter().position(|(c, _)| *c == col) {
                    Some(i) => i,
                    None => {
                        let mut nl = Layer::new(&clean(f[2]), cols.len());
                        nl.keyframes.clear();
                        cols.push((col, nl));
                        cols.len() - 1
                    }
                };
                let l = &mut cols[idx].1;
                let (w, h) = doc.bitmap(id).map(|b| (b.width as f32, b.height as f32)).unwrap_or((doc.width, doc.height));
                // bitmaps are drawn at 0,0..w,h; centre-fit is the app's job, keep 1:1
                let _ = (w, h);
                let el = Element { name: String::new(), matrix: ac_core::geom::Matrix::IDENTITY, alpha: 1.0, tint: None, kind: ElementKind::Bitmap(BitmapRef { bitmap: id }), three_d: None, filters: vec![], blend: Default::default(), color_fx: None, mesh: None, visible: true, cache_as_bitmap: false, export_as_bitmap: false };
                l.keyframes.insert(row, Keyframe::with(vec![el]));
                // an empty keyframe after the run ends it
                l.keyframes.entry(row + count).or_insert_with(Keyframe::blank);
                l.end = l.end.max(row + count);
            }
            Some("sound") if f.len() >= 5 => {
                sound_rows.push((f[1].parse().map_err(|_| "bad sound row")?, f[2].to_string(), f[3].parse().unwrap_or(0), f[4].parse().unwrap_or(0)));
            }
            _ => {}
        }
    }
    // Flare's first column is leftmost = Animate's top layer: keep order.
    cols.sort_by_key(|(c, _)| *c);
    doc.scene.layers = cols.into_iter().map(|(_, l)| l).collect();
    if !sound_rows.is_empty() {
        let mut layer = Layer::new("Sound", doc.scene.layers.len());
        layer.keyframes.clear();
        for (i, (row, path, inp, outp)) in sound_rows.iter().enumerate() {
            let data = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
            let pcm = ac_audio::decode(&data)?;
            let id = i as u32 + 1;
            let name = Path::new(path).file_name().and_then(|s| s.to_str()).unwrap_or("sound").to_string();
            doc.sounds.push(SoundAsset { id, name, data });
            let len_frames = (pcm.seconds() as f64 * doc.fps as f64).round() as u32;
            // ac-core has no in-point; the out cut becomes a Stop key, the in cut a warning.
            if *inp > 0 {
                doc.warnings.push(format!("sound {path}: in-point of {inp} frames is not representable in XFL; kept at full start"));
            }
            let mut k = Keyframe::blank();
            k.sound = Some(FrameSound { sync: SoundSync::Event, ..FrameSound::new(id) });
            layer.keyframes.insert(*row, k);
            let end = row + len_frames.saturating_sub(*outp).max(1);
            let mut stop = Keyframe::blank();
            stop.sound = Some(FrameSound { sync: SoundSync::Stop, ..FrameSound::new(id) });
            layer.keyframes.entry(end).or_insert(stop);
            layer.end = layer.end.max(end + 1);
        }
        doc.scene.layers.push(layer);
    }
    if doc.scene.layers.is_empty() {
        return Err("manifest has no cells".into());
    }
    Ok(doc)
}

/// Write an XFL (`.fla` zip, or folder/`.xfl`) from a manifest file.
pub fn export_xfl(manifest: &Path, out: &Path) -> Result<String, String> {
    let text = std::fs::read_to_string(manifest).map_err(|e| format!("{}: {e}", manifest.display()))?;
    let doc = doc_from_manifest(&text)?;
    let mut warnings = ac_xfl::save(&doc, out)?;
    warnings.extend(doc.warnings.iter().cloned());
    let mut log = format!("ok {} layers {} bitmaps {} sounds\n", doc.scene.layers.len(), doc.bitmaps.len(), doc.sounds.len());
    for w in warnings {
        let _ = writeln!(log, "warning: {w}");
    }
    Ok(log)
}

fn html5_into(doc: &Document, out: &Path, single_file: bool) -> Result<String, String> {
    let o = ac_render::html5::Html5Opts { single_file, loop_play: true, range: None };
    let (p, warns) = ac_render::html5::publish_to_report(doc, &o, out)?;
    let mut log = format!("ok {}\n", p.display());
    for w in warns {
        let _ = writeln!(log, "warning: {w}");
    }
    Ok(log)
}

/// HTML5 publish of a Flare scene manifest.
pub fn export_html5_manifest(manifest: &Path, out: &Path, single_file: bool) -> Result<String, String> {
    let text = std::fs::read_to_string(manifest).map_err(|e| format!("{}: {e}", manifest.display()))?;
    html5_into(&doc_from_manifest(&text)?, out, single_file)
}

/// HTML5 publish straight from an XFL/FLA.
pub fn export_html5_xfl(xfl: &Path, out: &Path, single_file: bool) -> Result<String, String> {
    html5_into(&ac_xfl::open(xfl)?.doc, out, single_file)
}

pub fn path_of(s: &str) -> PathBuf {
    PathBuf::from(s)
}

#[cfg(test)]
#[path = "ac_bridge_tests.rs"]
mod tests;
