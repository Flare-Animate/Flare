//! HTML5 Canvas documents, and File > Publish for them: a page (<name>.html), our own small
//! player (runtime.js, from html5_runtime.js) and the movie as data (<name>.js): each
//! timeline's frames as display lists, tweens and graphic symbols baked as display.rs plays
//! them, the shapes they draw as path commands, the bitmaps as image files (images/), and the
//! frame scripts as JavaScript (script::to_js).

use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt::Write as _;
use std::path::Path;

use tiny_skia::PathSegment;

use crate::Result;
use crate::bitmap::{BitmapItem, Format};
use crate::display::{Item, Placed, display_list};
use crate::doc::{Cap, ColorTransform, Document, Join, Matrix, Paint, Rgba, Seg, Shape, Spread, Text, Timeline};
use crate::gradient::GradientKind;
use crate::player::{self, Keys};
use crate::script::{self, Dialect, Stmt, Track, js_str};

/// The page's player: it moves the playheads, keeps clips in step, runs the scripts and draws.
const RUNTIME: &str = include_str!("mm_html5_runtime.js");

// ponytail: Animate's own mark for a Canvas document isn't publicly known, so ours is an
// attribute of our own, which Animate passes over: there such a document opens as ActionScript 3.0.
const PLATFORM: &str = "minnamatePlatform";

impl Document {
    /// An HTML5 Canvas document, as New Document's platform type makes one: its scripts are
    /// JavaScript and File > Publish makes a web page. Otherwise it's ActionScript 3.0, publishing a SWF.
    pub fn is_canvas(&self) -> bool {
        self.extra.get(PLATFORM) == Some("HTML5 Canvas")
    }

    pub fn set_canvas(&mut self, on: bool) {
        self.extra.set(PLATFORM, on.then_some("HTML5 Canvas"));
    }
}

/// Writes `name`.html, `name`.js, runtime.js and the bitmaps' images/ into `dir`. Returns what
/// the page leaves out.
pub fn publish(doc: &Document, dir: &Path, name: &str) -> Result<BTreeSet<String>> {
    let (movie, skipped, images) = movie_js(doc);
    std::fs::create_dir_all(dir)?;
    std::fs::write(dir.join(format!("{name}.html")), page(doc, name))?;
    std::fs::write(dir.join("runtime.js"), RUNTIME)?;
    std::fs::write(dir.join(format!("{name}.js")), movie)?;
    if !images.is_empty() {
        std::fs::create_dir_all(dir.join("images"))?;
    }
    for (path, bytes) in images {
        std::fs::write(dir.join(path), bytes)?;
    }
    Ok(skipped)
}

/// A bitmap's image file for the page: its file as imported when a browser reads it, else a PNG
/// of its pixels. None if it can't be read.
fn image_file(i: usize, b: &BitmapItem) -> Option<(String, Vec<u8>)> {
    let ext = match b.data.format()? {
        Format::Png => "png",
        Format::Jpeg => "jpg",
        Format::Gif => "gif",
        Format::Lossless => return Some((format!("images/{i}.png"), b.data.pixels()?.encode_png().ok()?)),
    };
    Some((format!("images/{i}.{ext}"), b.data.bytes().to_vec()))
}

fn page(doc: &Document, name: &str) -> String {
    let title = name.replace('&', "&amp;").replace('<', "&lt;");
    let script = title.replace('"', "&quot;");
    format!(
        "<!DOCTYPE html>\n<html>\n<head>\n<meta charset=\"utf-8\">\n<title>{title}</title>\n\
         <style>\n  body {{ margin: 0; background: {}; }}\n  canvas {{ display: block; margin: 0 auto; }}\n</style>\n\
         </head>\n<body>\n<canvas id=\"movie\" width=\"{}\" height=\"{}\"></canvas>\n\
         <script src=\"{script}.js\"></script>\n<script src=\"runtime.js\"></script>\n</body>\n</html>\n",
        hex(doc.background),
        doc.width,
        doc.height,
    )
}

/// The movie's script: `var movie = {...}`, what it leaves out, and the image files it loads.
fn movie_js(doc: &Document) -> (String, BTreeSet<String>, Vec<(String, Vec<u8>)>) {
    let mut p = Publisher { doc, shapes: Vec::new(), ids: HashMap::new(), images: Vec::new(), symbols: Vec::new(), seen: HashSet::new(), skipped: BTreeSet::new() };
    let mut timelines = vec![(String::new(), p.timeline(&Track::movie(doc), true))];
    // Each movie clip's (and button's) timeline, as the ones exported so far show them.
    while let Some(name) = p.symbols.pop() {
        if let Some(symbol) = doc.library.get(&name) {
            let js = p.timeline(&Track::symbol(&symbol.timeline), false);
            timelines.push((name, js));
        }
    }
    let mut out = String::from("// Published by minnamate: the movie runtime.js plays on index.html's canvas.\nvar movie = {\n");
    let _ = writeln!(out, "  width: {},\n  height: {},\n  fps: {},\n  background: \"{}\",", doc.width, doc.height, doc.frame_rate, hex(doc.background));
    let mut files = Vec::new();
    out.push_str("  images: [\n");
    for (i, b) in p.images.iter().enumerate() {
        let (w, h) = b.size().unwrap_or((0, 0));
        let src = image_file(i, b).map_or(String::new(), |(path, bytes)| {
            files.push((path.clone(), bytes));
            path
        });
        let _ = writeln!(out, "    {{src:{},w:{w},h:{h}}},", js_str(&src));
    }
    out.push_str("  ],\n  shapes: [\n");
    for shape in &p.shapes {
        let _ = writeln!(out, "    {shape},");
    }
    out.push_str("  ],\n  timelines: {\n");
    for (name, js) in timelines {
        let _ = writeln!(out, "    {}: {js},", js_str(&name));
    }
    out.push_str("  }\n};\n");
    (out, p.skipped, files)
}

struct Publisher<'a> {
    doc: &'a Document,
    /// Shape definitions, as JavaScript objects.
    shapes: Vec<String>,
    /// Shapes and texts defined, by address.
    ids: HashMap<usize, usize>,
    /// The bitmaps drawn, each an image the page loads.
    images: Vec<&'a BitmapItem>,
    /// Symbols whose timelines are still to export, and all those found.
    symbols: Vec<String>,
    seen: HashSet<String>,
    skipped: BTreeSet<String>,
}

impl<'a> Publisher<'a> {
    /// A timeline as the player reads it: labels (and the main one's scenes), each frame's
    /// display list, and the scripts as functions run with `this` the timeline.
    fn timeline(&mut self, track: &Track<'a>, main: bool) -> String {
        let mut out = String::from("{\n");
        let mut labels = Vec::new();
        let mut scripted = BTreeSet::new();
        for span in &track.scenes {
            for key in span.timeline.layers.iter().flat_map(|l| &l.frames) {
                if let Some((name, _)) = key.label().filter(|&(_, kind)| kind != "comment") {
                    labels.push(format!("[{},{}]", js_str(name), span.start + key.index));
                }
                if key.script().is_some() {
                    scripted.insert(span.start + key.index);
                }
            }
        }
        let _ = writeln!(out, "      labels: [{}],", labels.join(","));
        if main {
            let scenes: Vec<_> = track.scenes.iter().map(|s| format!("[{},{},{}]", js_str(s.name), s.start, s.len)).collect();
            let _ = writeln!(out, "      scenes: [{}],", scenes.join(","));
        }
        out.push_str("      frames: [\n");
        for f in 0..track.len() {
            let (s, local) = track.at(f);
            let tl = track.scenes[s].timeline;
            let list = display_list(self.doc, tl, local, &mut self.skipped);
            let items = self.items(&list, tl, &mut Keys::default());
            let _ = writeln!(out, "        // frame {}\n        {items},", f + 1);
        }
        out.push_str("      ],\n      scripts: {\n");
        let dialect = Dialect::of(self.doc);
        for f in scripted {
            let _ = writeln!(out, "        {f}: function () {{");
            for (layer, script) in track.scripts(f, dialect) {
                for stmt in &script.stmts {
                    if let Stmt::Skip { line, source } = stmt {
                        self.skipped.insert(format!("script `{source}` ({})", track.place(f, layer, *line)));
                    }
                }
                for line in script::to_js(&script).lines() {
                    let _ = writeln!(out, "          {line}");
                }
            }
            out.push_str("        },\n");
        }
        out.push_str("      }\n    }");
        out
    }

    /// A display list as the player draws it: shapes by index, clips by symbol and key (keyed
    /// as player.rs keys them), masks with what they mask.
    fn items(&mut self, list: &[Placed<'a>], tl: &'a Timeline, keys: &mut Keys) -> String {
        let mut items = Vec::new();
        for p in list {
            // Visible unchecked: not drawn, but a clip still plays, as the SWF's does.
            if p.effects.hidden && !matches!(p.item, Item::Clip(..)) {
                continue;
            }
            if !p.effects.plain() {
                self.skipped.insert("filters and blend modes".into());
            }
            let place = format!("m:{}{}", matrix(&p.matrix), color_transform(&p.color));
            items.push(match &p.item {
                Item::Shape(s) => format!("{{s:{},{place}}}", self.shape(Some(*s as *const Shape as usize), s)),
                Item::Text(t) => {
                    let id = match self.ids.get(&(*t as *const Text as usize)) {
                        Some(&id) => id,
                        None => self.shape(Some(*t as *const Text as usize), &crate::text::to_shape(t)),
                    };
                    format!("{{s:{id},{place}}}")
                }
                Item::Morph(s) => format!("{{s:{},{place}}}", self.shape(None, s)),
                Item::Bitmap(b) => format!("{{i:{},{place}}}", image_index(&mut self.images, b)),
                Item::Clip(symbol, kind) => {
                    let (layer, element, nth) = keys.next(p.source);
                    let Some(name) = player::symbol_name(self.doc, symbol) else { continue };
                    if self.seen.insert(name.to_string()) {
                        self.symbols.push(name.to_string());
                    }
                    let instance = player::instance_name(tl, p.source).map_or(String::new(), |n| format!(",n:{}", js_str(&n)));
                    let button = if *kind == crate::doc::SymbolKind::Button { ",b:1" } else { "" };
                    let hidden = if p.effects.hidden { ",h:1" } else { "" };
                    format!("{{clip:{},k:\"{layer}:{element}:{nth}\"{instance}{button}{hidden},{place}}}", js_str(name))
                }
                Item::Masked { mask, content } => {
                    let mask = self.items(mask, tl, keys);
                    format!("{{mask:{mask},items:{}}}", self.items(content, tl, keys))
                }
            });
        }
        format!("[{}]", items.join(","))
    }

    /// The index of a shape's definition: made once per `key` (an address), or afresh without.
    fn shape(&mut self, key: Option<usize>, s: &Shape) -> usize {
        if let Some(&id) = key.and_then(|k| self.ids.get(&k)) {
            return id;
        }
        let (doc, images) = (self.doc, &mut self.images);
        let js = shape_js(s, &mut self.skipped, &mut |name| doc.bitmap(name).map(|b| image_index(images, b)));
        self.shapes.push(js);
        let id = self.shapes.len() - 1;
        if let Some(k) = key {
            self.ids.insert(k, id);
        }
        id
    }
}

/// The index of bitmap `b` among the page's images, added if new.
fn image_index<'a>(images: &mut Vec<&'a BitmapItem>, b: &'a BitmapItem) -> usize {
    images.iter().position(|i| std::ptr::eq(*i, b)).unwrap_or_else(|| {
        images.push(b);
        images.len() - 1
    })
}

/// A shape as the player draws it: each fill as one even-odd path, each stroke style's edges as
/// one path, in SVG path syntax (the player's Path2D reads it). `image` gives a bitmap's index.
fn shape_js(s: &Shape, skipped: &mut BTreeSet<String>, image: &mut dyn FnMut(&str) -> Option<usize>) -> String {
    let mut fills = Vec::new();
    for (i, paint) in s.fills.iter().enumerate() {
        let (Some(path), Some(paint)) = (crate::render::fill_path(&s.edges, i as u32 + 1), paint_js(paint, skipped, image)) else { continue };
        let mut d = String::new();
        for seg in path.segments() {
            let _ = match seg {
                PathSegment::MoveTo(p) => write!(d, "M{} {}", num(p.x, 2), num(p.y, 2)),
                PathSegment::LineTo(p) => write!(d, "L{} {}", num(p.x, 2), num(p.y, 2)),
                PathSegment::QuadTo(c, p) => write!(d, "Q{} {} {} {}", num(c.x, 2), num(c.y, 2), num(p.x, 2), num(p.y, 2)),
                PathSegment::CubicTo(a, b, p) => {
                    write!(d, "C{} {} {} {} {} {}", num(a.x, 2), num(a.y, 2), num(b.x, 2), num(b.y, 2), num(p.x, 2), num(p.y, 2))
                }
                PathSegment::Close => write!(d, "Z"),
            };
        }
        fills.push(format!("{{d:\"{d}\",{paint}}}"));
    }
    let mut strokes = Vec::new();
    for (i, stroke) in s.strokes.iter().enumerate() {
        let mut d = String::new();
        for path in s.edges.iter().filter(|e| e.stroke == i as u32 + 1).flat_map(|e| &e.paths) {
            let _ = write!(d, "M{} {}", num(path.start.0, 2), num(path.start.1, 2));
            for seg in &path.segs {
                let _ = match *seg {
                    Seg::Line((x, y)) => write!(d, "L{} {}", num(x, 2), num(y, 2)),
                    Seg::Quad((cx, cy), (x, y)) => write!(d, "Q{} {} {} {}", num(cx, 2), num(cy, 2), num(x, 2), num(y, 2)),
                };
            }
        }
        let Some(paint) = paint_js(&stroke.paint, skipped, image).filter(|_| !d.is_empty()) else { continue };
        let cap = match stroke.cap {
            Cap::None => "butt",
            Cap::Round => "round",
            Cap::Square => "square",
        };
        let join = match stroke.join {
            Join::Miter => "miter",
            Join::Round => "round",
            Join::Bevel => "bevel",
        };
        strokes.push(format!("{{d:\"{d}\",w:{},cap:\"{cap}\",join:\"{join}\",{paint}}}", num(stroke.width, 2)));
    }
    format!("{{f:[{}],s:[{}]}}", fills.join(","), strokes.join(","))
}

/// A fill's or stroke's paint: `c` a colour, `g` a gradient in its ±819.2 px square, or `b` a
/// bitmap (`i` its image), tiled (`r`) or not.
fn paint_js(p: &Paint, skipped: &mut BTreeSet<String>, image: &mut dyn FnMut(&str) -> Option<usize>) -> Option<String> {
    match p {
        Paint::Solid(c) => Some(format!("c:{}", rgba(c))),
        Paint::Gradient(g) => {
            let (kind, focal) = match g.kind {
                GradientKind::Linear => ("l", 0.0),
                GradientKind::Radial { focal } => ("r", focal),
            };
            // ponytail: canvas gradients only pad; repeat and reflect would need the stops laid out over a wider ramp.
            if g.spread != Spread::Pad {
                skipped.insert("gradients' repeat and reflect (they pad)".into());
            }
            let stops: Vec<_> = g.stops.iter().map(|(t, c)| format!("[{},{}]", num(*t, 4), rgba(c))).collect();
            Some(format!("g:{{t:\"{kind}\",m:{},f:{},s:[{}]}}", matrix(&g.matrix), num(focal, 4), stops.join(",")))
        }
        Paint::Bitmap(f) => match image(&f.name) {
            Some(i) => Some(format!("b:{{i:{i},m:{},r:{}}}", matrix(&f.matrix), u8::from(!f.clipped))),
            None => {
                skipped.insert(format!("bitmap fill \"{}\" (not in the library)", f.name));
                None
            }
        },
        Paint::Unsupported { name, .. } => {
            skipped.insert(name.clone());
            None
        }
    }
}

fn matrix(m: &Matrix) -> String {
    format!("[{},{},{},{},{},{}]", num(m.a, 5), num(m.b, 5), num(m.c, 5), num(m.d, 5), num(m.tx, 2), num(m.ty, 2))
}

/// `,c:[...]`, the multipliers then the offsets, unless it changes nothing.
fn color_transform(cx: &ColorTransform) -> String {
    if *cx == ColorTransform::IDENTITY {
        return String::new();
    }
    let parts: Vec<_> = cx.mult.iter().chain(&cx.add).map(|&v| num(v, 4)).collect();
    format!(",c:[{}]", parts.join(","))
}

fn rgba(c: &Rgba) -> String {
    format!("[{}]", c.map(|v| num(v, 3)).join(","))
}

fn hex(c: Rgba) -> String {
    let [r, g, b, _] = c.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8);
    format!("#{r:02X}{g:02X}{b:02X}")
}

/// `v` with up to `decimals` decimals, and none it doesn't need.
fn num(v: f32, decimals: usize) -> String {
    if !v.is_finite() {
        return "0".into();
    }
    let s = format!("{v:.decimals$}");
    let s = if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.') } else { &s };
    if s == "-0" { "0".into() } else { s.into() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_stay_short() {
        assert_eq!([num(1.0, 2), num(-0.001, 2), num(12.345, 2), num(0.5, 5), num(f32::NAN, 2), num(100.0, 0)], ["1", "0", "12.35", "0.5", "0", "100"]);
    }

    #[test]
    fn the_platform_is_kept_in_the_document() {
        let mut doc = Document::new(550.0, 400.0, 24.0);
        assert!(!doc.is_canvas());
        doc.set_canvas(true);
        assert!(doc.is_canvas() && Dialect::of(&doc) == Dialect::JavaScript);
        doc.set_canvas(false);
        assert_eq!(doc, Document::new(550.0, 400.0, 24.0));
    }
}
