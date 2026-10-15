//! Writes XFL, the inverse of `xfl`: a `.fla` zip or an XFL folder. It saves what
//! the model holds plus the XML the reader kept for it (`Extra`, unsupported
//! elements and paints); `save_preserving` also copies the source's other files.

use std::collections::HashSet;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use zip::CompressionMethod;
use zip::write::SimpleFileOptions;

use crate::Result;
use crate::doc::{
    self, Cap, ColorTransform, Document, Edge, Element, Extra, Frame, Instance, Join, Layer, LayerKind, Loop, Matrix,
    Paint, Point, Rotate, Seg, Shape, Stroke, Symbol, SymbolKind, Text, TextAlign, TextKind, TextRun, Timeline, Tween,
};
use crate::gradient::{Gradient, GradientKind, Spread};
use crate::{text, xfl};

/// Saves a `.fla` file when `path` ends in `.fla`, otherwise an XFL folder: `path`,
/// or the folder of a `.xfl` / `DOMDocument.xml` path, as `xfl::load` takes them
/// (so an existing folder is saved into, whatever its name).
pub fn save(doc: &Document, path: &Path) -> Result<()> {
    save_preserving(doc, None, path)
}

/// `save`, also copying the files of `source` (the `.fla` or XFL folder `doc` was
/// loaded from, which may be `path` itself) that saving doesn't write: media in
/// `bin/` and `LIBRARY/`, publish settings, `META-INF/` and so on.
// ponytail: copied files pass through memory; stream zip to zip if huge media makes saving slow.
pub fn save_preserving(doc: &Document, source: Option<&Path>, path: &Path) -> Result<()> {
    let ext = |e: &str| path.extension().is_some_and(|x| x.eq_ignore_ascii_case(e));
    let folder = xfl::folder(path);
    let fla = folder.is_none() && ext("fla");
    let dir = folder.unwrap_or(path);
    // Animate names the marker after the document: movie.xfl for movie.fla or movie/.
    let name = if fla || ext("xfl") { path.file_stem() } else { dir.file_name() };
    let marker = format!("{}.xfl", name.map_or("document".into(), |n| n.to_string_lossy()));

    // A folder saved over itself already holds the files to keep.
    let same = |a: &Path| fs::canonicalize(a).is_ok_and(|a| fs::canonicalize(dir).is_ok_and(|d| a == d));
    let in_place = !fla && source.and_then(xfl::folder).is_some_and(same);
    let mut files = match source {
        Some(src) if !in_place => kept(src).map_err(|e| format!("{}: {e}", src.display()))?,
        _ => Vec::new(),
    };
    files.extend(documents(doc)?);
    // ponytail: no `mimetype` entry for a new .fla, as its content is unconfirmed;
    // one in the source is copied.
    files.push((marker, b"PROXY-CS5".to_vec()));
    if fla { write_zip(path, &files) } else { write_folder(dir, &files) }
}

/// The files of the XFL at `source` that saving doesn't write itself, by path in the
/// XFL: all but DOMDocument.xml, the `.xfl` marker and the symbol files in LIBRARY.
fn kept(source: &Path) -> Result<Vec<(String, Vec<u8>)>> {
    let written = |name: &str| {
        let name = name.to_ascii_lowercase();
        name == "domdocument.xml"
            || (!name.contains('/') && name.ends_with(".xfl"))
            || (name.starts_with("library/") && name.ends_with(".xml"))
    };
    let mut files = Vec::new();
    if let Some(dir) = xfl::folder(source) {
        let mut found = Vec::new();
        walk(dir, "", &mut found)?;
        found.sort();
        for (name, path) in found.into_iter().filter(|(name, _)| !written(name)) {
            files.push((name, fs::read(path)?));
        }
        return Ok(files);
    }
    let mut zip = zip::ZipArchive::new(fs::File::open(source)?)?;
    for i in 0..zip.len() {
        let mut file = zip.by_index(i)?;
        // Copied under its cleaned name: a root, drive or `..` in it would lead out of a
        // folder saved from this. (`enclosed_name` strips the first two, so its result counts.)
        let Some(path) = file.enclosed_name() else { continue };
        let name = path.to_string_lossy().replace('\\', "/");
        if file.is_dir() || written(&name) {
            continue;
        }
        let mut data = Vec::new();
        file.read_to_end(&mut data)?;
        files.push((name, data));
    }
    Ok(files)
}

/// The files under `dir` as (path in the XFL, path on disk), leaving out hidden ones such as `.git`.
fn walk(dir: &Path, prefix: &str, out: &mut Vec<(String, PathBuf)>) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let name = format!("{prefix}{name}");
        if entry.file_type()?.is_dir() {
            walk(&entry.path(), &format!("{name}/"), out)?;
        } else {
            out.push((name, entry.path()));
        }
    }
    Ok(())
}

fn write_zip(path: &Path, files: &[(String, Vec<u8>)]) -> Result<()> {
    replace(path, |file| {
        let mut zip = zip::ZipWriter::new(file);
        // As in Adobe's UCF containers, a `mimetype` entry goes first, uncompressed.
        let (first, rest): (Vec<_>, Vec<_>) = files.iter().partition(|(name, _)| name == "mimetype");
        for (name, data) in first.into_iter().chain(rest) {
            let method = if name == "mimetype" { CompressionMethod::Stored } else { CompressionMethod::Deflated };
            zip.start_file(name.as_str(), SimpleFileOptions::default().compression_method(method))?;
            zip.write_all(data)?;
        }
        zip.finish()?;
        Ok(())
    })
}

// ponytail: files are replaced one by one and none are deleted, so a failure partway
// leaves a mix of old and new ones (each whole), and a deleted symbol's file stays,
// unreferenced. Write to a new folder and swap it in if that matters.
fn write_folder(dir: &Path, files: &[(String, Vec<u8>)]) -> Result<()> {
    for (name, data) in files {
        let path = dir.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        replace(&path, |mut file| Ok(file.write_all(data)?))?;
    }
    Ok(())
}

/// Writes `path` through a temporary file beside it, so a failed save leaves the old file whole.
fn replace(path: &Path, write: impl FnOnce(fs::File) -> Result<()>) -> Result<()> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    let result = fs::File::create(&tmp).map_err(Into::into).and_then(write).and_then(|()| Ok(fs::rename(&tmp, path)?));
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

/// DOMDocument.xml and a LIBRARY file per symbol, by path in the XFL.
fn documents(doc: &Document) -> Result<Vec<(String, Vec<u8>)>> {
    let mut symbols: Vec<_> = doc.library.iter().collect();
    symbols.sort_by_key(|&(name, _)| name);
    // File names differ ignoring case, or on Windows and macOS one symbol would overwrite
    // another. The reader goes by the name inside the file, so a numbered one is fine.
    let (mut hrefs, mut taken) = (Vec::new(), HashSet::new());
    for (name, _) in &symbols {
        let stem = href(name)?;
        let mut href = format!("{stem}.xml");
        for n in 2.. {
            if taken.insert(href.to_lowercase()) {
                break;
            }
            href = format!("{stem} {n}.xml");
        }
        hrefs.push(href);
    }
    let mut files = vec![("DOMDocument.xml".to_string(), document(doc, &hrefs).into_bytes())];
    for ((name, symbol), href) in symbols.into_iter().zip(hrefs) {
        files.push((format!("LIBRARY/{href}"), symbol_item(name, symbol).into_bytes()));
    }
    Ok(files)
}

/// A symbol's file under LIBRARY/, less `.xml`: its name, folders and all, with the
/// characters that aren't safe in a file name, or would lead out of the folder, %-escaped.
// ponytail: Windows device names (CON, NUL...) and folder names ending in a dot or space
// (which Windows drops) aren't escaped.
fn href(name: &str) -> Result<String> {
    let mut href = String::new();
    for (i, part) in name.split('/').enumerate() {
        if part.is_empty() {
            return Err(format!("symbol {name:?} has an empty folder or file name").into());
        }
        if i > 0 {
            href.push('/');
        }
        for (j, c) in part.chars().enumerate() {
            if c.is_control() || "%\\:*?\"<>|".contains(c) || (j == 0 && c == '.') {
                href.push_str(&format!("%{:02X}", c as u32));
            } else {
                href.push(c);
            }
        }
    }
    Ok(href)
}

// ponytail: a new document gets none of Animate's version attributes (xflVersion,
// creatorInfo...), and <Include> keeps only its href, not itemID, itemIcon or
// loadImmediate. Add them if Animate turns our files down.
fn document(doc: &Document, hrefs: &[String]) -> String {
    let mut attrs = xmlns();
    let size = [("width", doc.width, 550.0), ("height", doc.height, 400.0), ("frameRate", doc.frame_rate, 24.0)];
    attrs.extend(nums(&size));
    let [r, g, b, _] = doc.background;
    let background = hex([r, g, b]);
    if background != "#FFFFFF" {
        attrs.push(("backgroundColor", background));
    }
    let mut x = Xml::default();
    x.kept("DOMDocument", &attrs, &doc.extra, xfl::DOCUMENT_ATTRS, |x| {
        if !hrefs.is_empty() {
            x.el("symbols", &[], |x| hrefs.iter().for_each(|href| x.el("Include", &[("href", href.clone())], |_| {})));
        }
        x.el("timelines", &[], |x| doc.scenes.iter().for_each(|tl| timeline(x, tl)));
    });
    x.text
}

fn symbol_item(name: &str, symbol: &Symbol) -> String {
    let mut attrs = xmlns();
    attrs.push(("name", name.into()));
    attrs.extend(symbol_type(symbol.kind));
    let mut x = Xml::default();
    x.kept("DOMSymbolItem", &attrs, &symbol.extra, xfl::SYMBOL_ATTRS, |x| {
        x.el("timeline", &[], |x| timeline(x, &symbol.timeline));
    });
    x.text
}

fn xmlns() -> Vec<(&'static str, String)> {
    vec![
        ("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance".into()),
        ("xmlns", "http://ns.adobe.com/xfl/2008/".into()),
    ]
}

fn timeline(x: &mut Xml, tl: &Timeline) {
    x.kept("DOMTimeline", &[("name", tl.name.clone())], &tl.extra, xfl::TIMELINE_ATTRS, |x| {
        x.el("layers", &[], |x| tl.layers.iter().for_each(|l| layer(x, l)));
    });
}

fn layer(x: &mut Xml, l: &Layer) {
    let mut attrs = vec![("name", l.name.clone())];
    let kind = match l.kind {
        LayerKind::Normal => None,
        LayerKind::Guide => Some("guide"),
        LayerKind::Mask => Some("mask"),
        LayerKind::Folder => Some("folder"),
    };
    attrs.extend(kind.map(|k| ("layerType", k.to_string())));
    x.kept("DOMLayer", &attrs, &l.extra, xfl::LAYER_ATTRS, |x| {
        x.el("frames", &[], |x| l.frames.iter().for_each(|f| frame(x, f)));
    });
}

fn frame(x: &mut Xml, f: &Frame) {
    let mut attrs = vec![("index", f.index.to_string())];
    if f.duration != 1 {
        attrs.push(("duration", f.duration.to_string()));
    }
    match &f.tween {
        Tween::None => {}
        Tween::Unsupported(kind) => attrs.push(("tweenType", kind.clone())),
        Tween::Classic { ease, rotate } => {
            attrs.push(("tweenType", "motion".into()));
            if *ease != 0.0 {
                attrs.push(("acceleration", unscale(*ease, 100.0).to_string()));
            }
            let turns = match *rotate {
                Rotate::Auto => None,
                Rotate::Cw(n) => Some(("clockwise", n)),
                Rotate::Ccw(n) => Some(("counter-clockwise", n)),
            };
            // The count goes in even when it's 1, since the reader's default for it is a guess.
            if let Some((way, n)) = turns {
                attrs.extend([("motionTweenRotate", way.to_string()), ("motionTweenRotateTimes", n.to_string())]);
            }
        }
        Tween::Shape { ease, angular, hints } => {
            attrs.push(("tweenType", "shape".into()));
            if *ease != 0.0 {
                attrs.push(("acceleration", unscale(*ease, 100.0).to_string()));
            }
            if *angular {
                attrs.push(("shapeTweenBlend", "angular".into()));
            }
            if !hints.is_empty() {
                attrs.push(("shapeHints", crate::shape_tween::write_hints(hints)));
            }
        }
        Tween::Motion(m) => {
            attrs.push(("tweenType", "motion object".into()));
            let turns = match m.rotate {
                Rotate::Auto => None,
                Rotate::Cw(n) => Some(("clockwise", n)),
                Rotate::Ccw(n) => Some(("counter-clockwise", n)),
            };
            attrs.push(("motionTweenRotate", turns.map_or("none", |t| t.0).into()));
            attrs.extend(turns.map(|(_, n)| ("motionTweenRotateTimes", n.to_string())));
        }
    }
    // A motion tween's XML goes before its elements: as read while the tween is, else afresh.
    let extra = crate::motion::saved_extra(f);
    x.kept("DOMFrame", &attrs, &extra, xfl::frame_attrs(&f.tween), |x| {
        crate::motion::write(x, f);
        x.el("elements", &[], |x| f.elements.iter().for_each(|el| element(x, el)));
    });
}

/// `elements` as XFL, the way they sit in a keyframe: what the editor puts on
/// the system clipboard when it copies them.
pub fn elements_xml(elements: &[Element]) -> String {
    let mut x = Xml::default();
    elements.iter().for_each(|el| element(&mut x, el));
    x.text
}

fn element(x: &mut Xml, el: &Element) {
    match el {
        Element::Shape(s) => shape(x, s),
        Element::Instance(i) => instance(x, i),
        Element::Group(members) => {
            x.el("DOMGroup", &[], |x| x.el("members", &[], |x| members.iter().for_each(|el| element(x, el))));
        }
        Element::Text(t) => text_field(x, t),
        Element::Unsupported { xml, .. } => x.raw(xml),
    }
}

fn text_field(x: &mut Xml, t: &Text) {
    let tag = match t.kind {
        TextKind::Static => "DOMStaticText",
        TextKind::Dynamic => "DOMDynamicText",
        TextKind::Input => "DOMInputText",
    };
    // The box as laid out, where the model doesn't keep it: a fixed dynamic or input field's
    // height is in `extra`.
    let layout = text::layout(t);
    let fixed = t.width.is_some();
    let mut attrs = vec![("width", t.width.unwrap_or(layout.width).to_string())];
    if t.kind == TextKind::Static || !fixed {
        attrs.push(("height", layout.height.to_string()));
    }
    if !fixed {
        attrs.push(("autoExpand", "true".into()));
    }
    x.kept(tag, &attrs, &t.extra, xfl::text_attrs(t.kind, fixed), |x| {
        matrix(x, &t.matrix);
        x.el("textRuns", &[], |x| t.runs.iter().for_each(|run| text_run(x, t, run)));
    });
}

fn text_run(x: &mut Xml, t: &Text, run: &TextRun) {
    // Paragraph settings go with every run, as Animate stores them.
    let align = match t.align {
        TextAlign::Left => None,
        TextAlign::Center => Some("center"),
        TextAlign::Right => Some("right"),
        TextAlign::Justify => Some("justify"),
    };
    let mut attrs: Vec<_> = align.map(|a| ("alignment", a.to_string())).into_iter().collect();
    attrs.extend(nums(&[("letterSpacing", run.letter_spacing, 0.0)]));
    attrs.push(("lineSpacing", t.line_spacing.to_string()));
    attrs.push(("size", run.size.to_string()));
    attrs.push(("bitmapSize", (run.size * 20.0).round().to_string()));
    attrs.push(("face", run.face.clone()));
    let [r, g, b, a] = run.color;
    attrs.push(("fillColor", hex([r, g, b])));
    attrs.extend(nums(&[("alpha", a, 1.0)]));
    for (name, on) in [("bold", run.bold), ("italic", run.italic)] {
        if on {
            attrs.push((name, "true".into()));
        }
    }
    x.el("DOMTextRun", &[], |x| {
        // Animate ends paragraphs with a carriage return.
        x.raw(&format!("<characters>{}</characters>", escape(&run.text.replace('\n', "\r"))));
        x.el("textAttrs", &[], |x| x.kept("DOMTextAttrs", &attrs, &run.extra, xfl::TEXT_RUN_ATTRS, |_| {}));
    });
}

fn shape(x: &mut Xml, s: &Shape) {
    x.kept("DOMShape", &[], &s.extra, &[], |x| {
        matrix(x, &s.matrix);
        if !s.fills.is_empty() {
            x.el("fills", &[], |x| {
                for (i, fill) in s.fills.iter().enumerate() {
                    x.el("FillStyle", &[("index", (i + 1).to_string())], |x| paint(x, fill));
                }
            });
        }
        if !s.strokes.is_empty() {
            x.el("strokes", &[], |x| {
                for (i, st) in s.strokes.iter().enumerate() {
                    x.el("StrokeStyle", &[("index", (i + 1).to_string())], |x| stroke(x, st));
                }
            });
        }
        if !s.edges.is_empty() {
            x.el("edges", &[], |x| s.edges.iter().for_each(|e| x.el("Edge", &edge(e), |_| {})));
        }
    });
}

fn stroke(x: &mut Xml, st: &Stroke) {
    // Strokes scale with their shape when drawn here, as Animate's "normal" scale mode does.
    let mut attrs = vec![("scaleMode", "normal".to_string())];
    attrs.extend(nums(&[("weight", st.width, 1.0)]));
    let caps = match st.cap {
        Cap::None => Some("none"),
        Cap::Round => None,
        Cap::Square => Some("square"),
    };
    let joints = match st.join {
        Join::Miter => Some("miter"),
        Join::Round => None,
        Join::Bevel => Some("bevel"),
    };
    attrs.extend(caps.map(|c| ("caps", c.to_string())));
    attrs.extend(joints.map(|j| ("joints", j.to_string())));
    x.el("SolidStroke", &attrs, |x| x.el("fill", &[], |x| paint(x, &st.paint)));
}

fn paint(x: &mut Xml, p: &Paint) {
    match p {
        Paint::Solid([r, g, b, a]) => {
            let color = hex([*r, *g, *b]);
            let mut attrs = if color == "#000000" { Vec::new() } else { vec![("color", color)] };
            attrs.extend(nums(&[("alpha", *a, 1.0)]));
            x.el("SolidColor", &attrs, |_| {});
        }
        Paint::Gradient(g) => gradient(x, g),
        Paint::Unsupported { xml, .. } => x.raw(xml),
    }
}

fn gradient(x: &mut Xml, g: &Gradient) {
    let (tag, focal) = match g.kind {
        GradientKind::Linear => ("LinearGradient", 0.0),
        GradientKind::Radial { focal } => ("RadialGradient", focal),
    };
    let spread = match g.spread {
        Spread::Pad => None,
        Spread::Reflect => Some("reflect"),
        Spread::Repeat => Some("repeat"),
    };
    let mut attrs: Vec<_> = spread.map(|s| ("spreadMethod", s.to_string())).into_iter().collect();
    if g.linear_rgb {
        attrs.push(("interpolationMethod", "linearRGB".into()));
    }
    attrs.extend(nums(&[("focalPointRatio", focal, 0.0)]));
    x.kept(tag, &attrs, &g.extra, xfl::GRADIENT_ATTRS, |x| {
        matrix(x, &g.matrix);
        for &(ratio, [r, gr, b, a]) in &g.stops {
            let color = hex([r, gr, b]);
            let mut attrs = if color == "#000000" { Vec::new() } else { vec![("color", color)] };
            attrs.extend(nums(&[("alpha", a, 1.0)]));
            attrs.push(("ratio", ratio.to_string()));
            x.el("GradientEntry", &attrs, |_| {});
        }
    });
}

// ponytail: Edge attributes other than these, like Animate's `cubics` (the curves as
// drawn), aren't read, so they aren't saved either.
fn edge(e: &Edge) -> Vec<(&'static str, String)> {
    let styles = [("fillStyle0", e.fill0), ("fillStyle1", e.fill1), ("strokeStyle", e.stroke)];
    let mut attrs: Vec<_> = styles.into_iter().filter(|&(_, i)| i != 0).map(|(k, i)| (k, i.to_string())).collect();
    if !e.paths.is_empty() {
        attrs.push(("edges", edges(&e.paths)));
    }
    attrs
}

/// The inverse of `xfl::parse_edges`, in decimal twips rounded to Animate's 1/256-twip grid:
/// whole twips print as integers, and every point read from that grid reads back the same.
fn edges(paths: &[doc::Path]) -> String {
    // ponytail: `as i64` saturates and makes NaN 0, as the SWF export does, so a broken point
    // moves instead of failing the save, and the file always loads. Validate in the editor if needed.
    let tw = |v: f32| ((v as f64 * 5120.0).round() as i64) as f64 / 256.0;
    let pt = |(x, y): Point| format!("{} {}", tw(x), tw(y));
    let path = |p: &doc::Path| {
        let segs = p.segs.iter().map(|&seg| match seg {
            Seg::Line(to) => format!("|{}", pt(to)),
            Seg::Quad(c, to) => format!("[{} {}", pt(c), pt(to)),
        });
        format!("!{}{}", pt(p.start), segs.collect::<String>())
    };
    paths.iter().map(path).collect()
}

fn instance(x: &mut Xml, i: &Instance) {
    let mut attrs = vec![("libraryItemName", i.symbol.clone())];
    attrs.extend(symbol_type(i.kind));
    // Animate writes the loop setting of every graphic instance, including the default.
    let looping = match i.looping {
        Loop::Loop if i.kind != SymbolKind::Graphic => None,
        Loop::Loop => Some("loop"),
        Loop::PlayOnce => Some("play once"),
        Loop::SingleFrame => Some("single frame"),
    };
    attrs.extend(looping.map(|l| ("loop", l.to_string())));
    if i.first_frame != 0 {
        attrs.push(("firstFrame", i.first_frame.to_string()));
    }
    x.kept("DOMSymbolInstance", &attrs, &i.extra, xfl::INSTANCE_ATTRS, |x| {
        matrix(x, &i.matrix);
        let pivot = nums(&[("x", i.pivot.0, 0.0), ("y", i.pivot.1, 0.0)]);
        x.el("transformationPoint", &[], |x| x.el("Point", &pivot, |_| {}));
        if i.color != ColorTransform::IDENTITY {
            // ponytail: saved as Animate's Advanced color effect, so a Tint or Brightness
            // setting comes back looking the same but set as Advanced.
            let mult = ["redMultiplier", "greenMultiplier", "blueMultiplier", "alphaMultiplier"];
            let add = ["redOffset", "greenOffset", "blueOffset", "alphaOffset"];
            let mut color: Vec<_> = mult.into_iter().zip(i.color.mult).map(|(k, v)| (k, v, 1.0)).collect();
            color.extend(add.into_iter().zip(i.color.add).map(|(k, v)| (k, unscale(v, 255.0), 0.0)));
            x.el("color", &[], |x| x.el("Color", &nums(&color), |_| {}));
        }
    });
}

fn symbol_type(kind: SymbolKind) -> Option<(&'static str, String)> {
    let name = match kind {
        SymbolKind::MovieClip => return None,
        SymbolKind::Graphic => "graphic",
        SymbolKind::Button => "button",
    };
    Some(("symbolType", name.into()))
}

fn matrix(x: &mut Xml, m: &Matrix) {
    if *m != Matrix::IDENTITY {
        let values =
            [("a", m.a, 1.0), ("b", m.b, 0.0), ("c", m.c, 0.0), ("d", m.d, 1.0), ("tx", m.tx, 0.0), ("ty", m.ty, 0.0)];
        x.el("matrix", &[], |x| x.el("Matrix", &nums(&values), |_| {}));
    }
}

/// `(name, value, default)`s as attributes, leaving out the defaults. f32's shortest
/// decimal reads back exactly.
fn nums<'a>(list: &[(&'a str, f32, f32)]) -> Vec<(&'a str, String)> {
    list.iter().filter(|(_, v, default)| v != default).map(|&(name, v, _)| (name, v.to_string())).collect()
}

/// What to write for `v` where the reader divides by `k`: an integer if that reads back
/// exactly, as Animate writes them, else the f32 nearest `v * k` that does, if any.
fn unscale(v: f32, k: f32) -> f32 {
    let x = v * k;
    [x.round(), x, x.next_down(), x.next_up()].into_iter().find(|x| x / k == v).unwrap_or(x)
}

pub(crate) fn hex(rgb: [f32; 3]) -> String {
    let [r, g, b] = rgb.map(|c| (c * 255.0).round() as u8);
    format!("#{r:02X}{g:02X}{b:02X}")
}

/// Indented XML text, written element by element.
#[derive(Default)]
pub(crate) struct Xml {
    text: String,
    depth: usize,
}

impl Xml {
    /// `<tag attrs>`, what `body` writes, `</tag>`; or `<tag attrs/>` if it writes nothing.
    pub(crate) fn el(&mut self, tag: &str, attrs: &[(&str, String)], body: impl FnOnce(&mut Self)) {
        self.indent();
        self.text.push('<');
        self.text.push_str(tag);
        for (name, value) in attrs {
            self.text.push_str(&format!(" {name}=\"{}\"", escape(value)));
        }
        self.text.push_str(">\n");
        let start = self.text.len();
        self.depth += 1;
        body(self);
        self.depth -= 1;
        if self.text.len() == start {
            self.text.truncate(start - 2);
            self.text.push_str("/>\n");
        } else {
            self.indent();
            self.text.push_str(&format!("</{tag}>\n"));
        }
    }

    /// `el` plus what `extra` kept of the element, but none of the `owned` attributes:
    /// those are the model's, which `attrs` has.
    fn kept(
        &mut self,
        tag: &str,
        attrs: &[(&str, String)],
        extra: &Extra,
        owned: &[&str],
        body: impl FnOnce(&mut Self),
    ) {
        let kept = extra.attrs.iter().filter(|(name, _)| !owned.contains(&name.as_str()));
        let attrs: Vec<_> = attrs.iter().cloned().chain(kept.map(|(name, v)| (name.as_str(), v.clone()))).collect();
        self.el(tag, &attrs, |x| {
            extra.before.iter().for_each(|xml| x.raw(xml));
            body(x);
            extra.after.iter().for_each(|xml| x.raw(xml));
        });
    }

    /// XML from the source file, as it was.
    pub(crate) fn raw(&mut self, xml: &str) {
        if !xml.is_empty() {
            self.indent();
            self.text.push_str(xml);
            self.text.push('\n');
        }
    }

    fn indent(&mut self) {
        self.text.push_str(&"  ".repeat(self.depth));
    }
}

/// Escapes an attribute value, tabs and line breaks too: XML would read those as spaces.
pub(crate) fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\t' | '\n' | '\r' => out.push_str(&format!("&#{};", c as u32)),
            // Characters XML can't hold at all go.
            '\0'..='\u{1f}' | '\u{fffe}' | '\u{ffff}' => {}
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xfl::parse_edges;

    #[test]
    fn edges_keep_animates_twips() {
        // Whole twips stay integers, though most aren't exact in f32 pixels; hex steps turn decimal.
        assert_eq!(edges(&parse_edges("!1 3|7 11[#1A.40 -3 #FFFFFF.80 5").unwrap()), "!1 3|7 11[26.25 -3 -0.5 5");

        // Every point read from Animate's grid, whole or 1/256 twips, comes back the same.
        let hex = |k: i32| format!("#{:X}.{:02X}", k as u32 >> 8, k as u32 & 0xFF);
        let mut tokens: Vec<String> = (-3000..3000).chain((i32::MIN..i32::MAX).step_by(40_009)).map(hex).collect();
        tokens.extend(["2147483647", "-99999999999", "123456789"].map(String::from));
        let source: String = tokens.chunks(2).map(|p| format!("!{} {}|0 0", p[0], p[p.len() - 1])).collect();
        let paths = parse_edges(&source).unwrap();
        assert_eq!(parse_edges(&edges(&paths)).unwrap(), paths);

        // Off the grid, points round to it; non-finite ones still make a file that loads.
        let odd = [doc::Path { start: (1.0 / 3.0, f32::NAN), segs: vec![Seg::Line((f32::INFINITY, -1e-7))] }];
        let back = parse_edges(&edges(&odd)).unwrap();
        assert_eq!(back[0].start, ((6.667_968_75 / 20.0) as f32, 0.0));
        assert_eq!(back[0].segs[0].end().1, 0.0);
    }

    #[test]
    fn control_characters_keep_the_xml_loadable() {
        // Pasted text can bring them; XML can't hold them, not even as references.
        assert!(roxmltree::Document::parse("<a>\u{b}</a>").is_err() && roxmltree::Document::parse("<a>&#11;</a>").is_err());
        let text = escape("tab\tvt\u{b}nul\u{0}");
        assert_eq!(text, "tab&#9;vtnul");
        assert!(roxmltree::Document::parse(&format!("<a b=\"{text}\">{text}</a>")).is_ok());
    }

    #[test]
    fn hrefs_stay_in_library() {
        assert_eq!(href("Folder/Ball 2").unwrap(), "Folder/Ball 2");
        assert_eq!(href("../a?b:c%").unwrap(), "%2E./a%3Fb%3Ac%25");
        assert!(href("/abs").is_err());
    }

    #[test]
    fn scaled_values_prefer_integers() {
        assert_eq!(unscale(-0.37, 100.0), -37.0);
        assert_eq!(unscale(128.0 / 255.0, 255.0), 128.0);
        assert_eq!(unscale(0.5, 255.0), 127.5);
    }
}
