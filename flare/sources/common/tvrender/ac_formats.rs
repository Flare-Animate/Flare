//! More publish formats: sprite sheets (PNG + JSON / XML atlas) and Lottie
//! JSON (one shape layer per distinct frame, so it plays in any Lottie
//! player exactly as the stage shows it).

use crate::{render_frame_opts, unpremultiplied, ExportOpts};
use ac_core::model::*;
use ac_core::{Matrix, Pt, Seg};
use std::fmt::Write as _;

// ------------------------------------------------------------- texture atlas

/// An image to pack: straight (non-premultiplied) RGBA.
#[derive(Clone, Debug)]
pub struct AtlasInput {
    pub name: String,
    pub w: u32,
    pub h: u32,
    pub rgba: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct AtlasOpts {
    /// Largest atlas side in pixels.
    pub max_size: u32,
    /// Pixels kept clear between images.
    pub padding: u32,
    /// Allow turning images 90 degrees clockwise to pack tighter.
    pub allow_rotate: bool,
    /// Round the atlas size up to powers of two.
    pub power_of_two: bool,
}

impl Default for AtlasOpts {
    fn default() -> Self {
        AtlasOpts { max_size: 2048, padding: 1, allow_rotate: false, power_of_two: false }
    }
}

/// Where one image sits. `w`/`h` are the size in the atlas (swapped when `rotated`).
#[derive(Clone, Debug, PartialEq)]
pub struct AtlasEntry {
    pub name: String,
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub rotated: bool,
    pub source_w: u32,
    pub source_h: u32,
}

#[derive(Clone, Debug)]
pub struct Atlas {
    pub width: u32,
    pub height: u32,
    pub entries: Vec<AtlasEntry>,
    pub rgba: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Rc {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
}

impl Rc {
    fn overlaps(&self, o: &Rc) -> bool {
        self.x < o.x + o.w && o.x < self.x + self.w && self.y < o.y + o.h && o.y < self.y + self.h
    }
    fn contains(&self, o: &Rc) -> bool {
        o.x >= self.x && o.y >= self.y && o.x + o.w <= self.x + self.w && o.y + o.h <= self.y + self.h
    }
}

/// A placement from `max_rects`: top-left corner and whether the rectangle
/// was turned (its footprint is then `h` wide and `w` tall).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placement {
    pub x: u32,
    pub y: u32,
    pub rotated: bool,
}

/// MaxRects bin packing (best short side fit): place every `(w, h)` in a
/// `bin_w` x `bin_h` bin, or `None` if they don't all fit. Placements keep the
/// order of `sizes`; rectangles never overlap and stay inside the bin.
pub fn max_rects(sizes: &[(u32, u32)], bin_w: u32, bin_h: u32, allow_rotate: bool) -> Option<Vec<Placement>> {
    let mut free = vec![Rc { x: 0, y: 0, w: bin_w, h: bin_h }];
    let mut out = vec![Placement { x: 0, y: 0, rotated: false }; sizes.len()];
    // large first
    let mut order: Vec<usize> = (0..sizes.len()).collect();
    order.sort_by_key(|&i| std::cmp::Reverse((sizes[i].0.max(sizes[i].1), sizes[i].0 as u64 * sizes[i].1 as u64)));
    for i in order {
        let (w, h) = sizes[i];
        // best short side fit, then best long side fit
        let mut best: Option<(u32, u32, Rc, bool)> = None;
        for f in &free {
            for rot in [false, true] {
                if rot && (!allow_rotate || w == h) {
                    continue;
                }
                let (rw, rh) = if rot { (h, w) } else { (w, h) };
                if rw <= f.w && rh <= f.h {
                    let (lw, lh) = (f.w - rw, f.h - rh);
                    let key = (lw.min(lh), lw.max(lh));
                    if best.as_ref().map_or(true, |b| key < (b.0, b.1)) {
                        best = Some((key.0, key.1, Rc { x: f.x, y: f.y, w: rw, h: rh }, rot));
                    }
                }
            }
        }
        let (_, _, placed, rot) = best?;
        out[i] = Placement { x: placed.x, y: placed.y, rotated: rot };
        // split every free rect the placement overlaps
        let mut next: Vec<Rc> = Vec::with_capacity(free.len() + 4);
        for f in free {
            if !f.overlaps(&placed) {
                next.push(f);
                continue;
            }
            if placed.x > f.x {
                next.push(Rc { x: f.x, y: f.y, w: placed.x - f.x, h: f.h });
            }
            if placed.x + placed.w < f.x + f.w {
                next.push(Rc { x: placed.x + placed.w, y: f.y, w: f.x + f.w - placed.x - placed.w, h: f.h });
            }
            if placed.y > f.y {
                next.push(Rc { x: f.x, y: f.y, w: f.w, h: placed.y - f.y });
            }
            if placed.y + placed.h < f.y + f.h {
                next.push(Rc { x: f.x, y: placed.y + placed.h, w: f.w, h: f.y + f.h - placed.y - placed.h });
            }
        }
        // drop free rects contained in another
        let mut keep = vec![true; next.len()];
        for a in 0..next.len() {
            for b in 0..next.len() {
                if a != b && keep[a] && keep[b] && next[b].contains(&next[a]) && (next[a] != next[b] || a > b) {
                    keep[a] = false;
                }
            }
        }
        free = next.into_iter().zip(keep).filter(|(_, k)| *k).map(|(r, _)| r).collect();
    }
    Some(out)
}

fn next_pow2(v: u32) -> u32 {
    v.max(1).next_power_of_two()
}

/// Pack `inputs` into the smallest atlas (up to `max_size` on a side) and
/// compose its pixels. Entries keep the input order.
pub fn pack_atlas(inputs: &[AtlasInput], o: &AtlasOpts) -> Result<Atlas, String> {
    if inputs.is_empty() {
        return Err("no images to pack".into());
    }
    for i in inputs {
        if i.w == 0 || i.h == 0 || i.rgba.len() != (i.w as usize) * (i.h as usize) * 4 {
            return Err(format!("image '{}' is empty or its pixel data does not match its size", i.name));
        }
        let fits = |w: u32, h: u32| w <= o.max_size && h <= o.max_size;
        if !fits(i.w, i.h) && !(o.allow_rotate && fits(i.h, i.w)) {
            return Err(format!("image '{}' ({}x{}) is larger than the maximum atlas size {}", i.name, i.w, i.h, o.max_size));
        }
    }
    let pad = o.padding;
    let sizes: Vec<(u32, u32)> = inputs.iter().map(|i| (i.w + pad, i.h + pad)).collect();
    let area: u64 = sizes.iter().map(|s| s.0 as u64 * s.1 as u64).sum();
    // candidate bins, smallest area first (power-of-two sides, squares and 2:1)
    let mut bins: Vec<(u32, u32)> = Vec::new();
    let mut s = 1u32;
    while s <= o.max_size.next_power_of_two() {
        for (w, h) in [(s, s), (s * 2, s)] {
            bins.push((w, h));
            bins.push((h, w));
        }
        s *= 2;
    }
    bins.sort_by_key(|b| (b.0 as u64 * b.1 as u64, b.0.max(b.1)));
    bins.dedup();
    let mut found = None;
    for (bw, bh) in bins {
        // the padded bin: the last image needs no padding after it
        let (bw, bh) = (bw.min(o.max_size) + pad, bh.min(o.max_size) + pad);
        if (bw as u64) * (bh as u64) < area {
            continue;
        }
        if let Some(p) = max_rects(&sizes, bw, bh, o.allow_rotate) {
            found = Some(p);
            break;
        }
    }
    let placements = found.ok_or_else(|| format!("the images do not fit in a {0}x{0} atlas", o.max_size))?;
    let (mut w, mut h) = (0u32, 0u32);
    let mut entries = Vec::with_capacity(inputs.len());
    for (i, p) in inputs.iter().zip(&placements) {
        let (ew, eh) = if p.rotated { (i.h, i.w) } else { (i.w, i.h) };
        w = w.max(p.x + ew);
        h = h.max(p.y + eh);
        entries.push(AtlasEntry { name: i.name.clone(), x: p.x, y: p.y, w: ew, h: eh, rotated: p.rotated, source_w: i.w, source_h: i.h });
    }
    if o.power_of_two {
        w = next_pow2(w);
        h = next_pow2(h);
    }
    let mut rgba = vec![0u8; (w as usize) * (h as usize) * 4];
    for (i, e) in inputs.iter().zip(&entries) {
        for y in 0..e.h {
            for x in 0..e.w {
                // rotated 90 degrees clockwise: atlas (x, y) takes source (y, h_src - 1 - x)
                let (sx, sy) = if e.rotated { (y, i.h - 1 - x) } else { (x, y) };
                let s = ((sy * i.w + sx) * 4) as usize;
                let d = (((e.y + y) * w + e.x + x) * 4) as usize;
                rgba[d..d + 4].copy_from_slice(&i.rgba[s..s + 4]);
            }
        }
    }
    Ok(Atlas { width: w, height: h, entries, rgba })
}

impl Atlas {
    pub fn png(&self) -> Result<Vec<u8>, String> {
        let mut out = Vec::new();
        let mut enc = png::Encoder::new(&mut out, self.width, self.height);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut w = enc.write_header().map_err(|e| e.to_string())?;
        w.write_image_data(&self.rgba).map_err(|e| e.to_string())?;
        drop(w);
        Ok(out)
    }

    /// TexturePacker-style JSON hash (`frames` by name, `rotated` = 90 degrees clockwise).
    pub fn json(&self, image_name: &str) -> String {
        let esc = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
        let frames: Vec<String> = self
            .entries
            .iter()
            .map(|e| {
                format!(
                    "\n  \"{}\": {{\"frame\":{{\"x\":{},\"y\":{},\"w\":{},\"h\":{}}},\"rotated\":{},\"trimmed\":false,\"spriteSourceSize\":{{\"x\":0,\"y\":0,\"w\":{},\"h\":{}}},\"sourceSize\":{{\"w\":{},\"h\":{}}}}}",
                    esc(&e.name),
                    e.x,
                    e.y,
                    e.w,
                    e.h,
                    e.rotated,
                    e.source_w,
                    e.source_h,
                    e.source_w,
                    e.source_h
                )
            })
            .collect();
        format!(
            "{{\"frames\": {{{}\n}},\n\"meta\": {{\"app\":\"Flare\",\"version\":\"1.0\",\"image\":\"{}\",\"format\":\"RGBA8888\",\"size\":{{\"w\":{},\"h\":{}}},\"scale\":\"1\"}}\n}}\n",
            frames.join(","),
            esc(image_name),
            self.width,
            self.height
        )
    }
}

/// Decode a PNG or JPEG file's bytes to straight RGBA.
pub fn decode_atlas_image(name: &str, bytes: &[u8]) -> Result<AtlasInput, String> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        let mut dec = png::Decoder::new(bytes);
        dec.set_transformations(png::Transformations::normalize_to_color8());
        let mut r = dec.read_info().map_err(|e| format!("{name}: {e}"))?;
        let mut buf = vec![0; r.output_buffer_size()];
        let info = r.next_frame(&mut buf).map_err(|e| format!("{name}: {e}"))?;
        let (w, h) = (info.width, info.height);
        let px = &buf[..info.buffer_size()];
        let mut rgba = Vec::with_capacity((w * h * 4) as usize);
        match info.color_type {
            png::ColorType::Rgba => rgba.extend_from_slice(px),
            png::ColorType::Rgb => px.chunks_exact(3).for_each(|p| rgba.extend_from_slice(&[p[0], p[1], p[2], 255])),
            png::ColorType::GrayscaleAlpha => px.chunks_exact(2).for_each(|p| rgba.extend_from_slice(&[p[0], p[0], p[0], p[1]])),
            png::ColorType::Grayscale => px.iter().for_each(|&g| rgba.extend_from_slice(&[g, g, g, 255])),
            png::ColorType::Indexed => return Err(format!("{name}: indexed PNG was not expanded")),
        }
        Ok(AtlasInput { name: name.into(), w, h, rgba })
    } else {
        let img = image::load_from_memory(bytes).map_err(|e| format!("{name}: {e}"))?.to_rgba8();
        Ok(AtlasInput { name: name.into(), w: img.width(), h: img.height(), rgba: img.into_raw() })
    }
}

/// Pack the PNG/JPEG files (directories are searched one level deep) into
/// `<out>.png` and `<out>.json`. Names in the atlas are the file stems.
pub fn export_atlas(paths: &[std::path::PathBuf], o: &AtlasOpts, out: &std::path::Path) -> Result<(std::path::PathBuf, std::path::PathBuf, Atlas), String> {
    let mut files: Vec<std::path::PathBuf> = Vec::new();
    for p in paths {
        if p.is_dir() {
            let mut v: Vec<_> = std::fs::read_dir(p).map_err(|e| format!("{}: {e}", p.display()))?.filter_map(|e| e.ok().map(|e| e.path())).collect();
            v.sort();
            files.extend(v.into_iter().filter(|f| is_atlas_image(f)));
        } else {
            files.push(p.clone());
        }
    }
    let mut inputs = Vec::new();
    for f in &files {
        let name = f.file_stem().and_then(|s| s.to_str()).unwrap_or("image").to_string();
        let bytes = std::fs::read(f).map_err(|e| format!("{}: {e}", f.display()))?;
        inputs.push(decode_atlas_image(&name, &bytes)?);
    }
    let atlas = pack_atlas(&inputs, o)?;
    let png_path = out.with_extension("png");
    let json_path = out.with_extension("json");
    let image_name = png_path.file_name().and_then(|s| s.to_str()).unwrap_or("atlas.png").to_string();
    std::fs::write(&png_path, atlas.png()?).map_err(|e| format!("{}: {e}", png_path.display()))?;
    std::fs::write(&json_path, atlas.json(&image_name)).map_err(|e| format!("{}: {e}", json_path.display()))?;
    Ok((png_path, json_path, atlas))
}

fn is_atlas_image(p: &std::path::Path) -> bool {
    p.extension().and_then(|e| e.to_str()).map(|e| matches!(e.to_ascii_lowercase().as_str(), "png" | "jpg" | "jpeg")).unwrap_or(false)
}

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
