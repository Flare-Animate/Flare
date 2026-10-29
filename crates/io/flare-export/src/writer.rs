use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use flare_core::{Rgba8, SymbolId};
use flare_document::{
    Anchor, Attachment, Content, Document, FrameSpan, Instance, Layer, PathSegment, Placement,
    Shape, Timeline, Tween,
};
use thiserror::Error;

use crate::curves::{self, Point};

/// Curve approximation tolerance, in pixels.
const CURVE_TOLERANCE_PX: f64 = 0.05;
const TWIPS_PER_PX: f64 = 20.0;
const NAMESPACE: &str = "http://ns.adobe.com/xfl/2008/";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Written faithfully but worth knowing about.
    Info,
    /// Written with a change in meaning.
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub location: String,
    pub message: String,
}

/// The files of an exported document, by path relative to its root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    /// Name used for the `.xfl` stub and, with `.fla`, the archive.
    pub name: String,
    pub files: Vec<(String, String)>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Error)]
pub enum ExportError {
    #[error("{0} already exists")]
    AlreadyExists(PathBuf),
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("zip error: {0}")]
    Zip(#[from] zip::result::ZipError),
}

// ---- text helpers ---------------------------------------------------------------------

/// Escapes text for an XML attribute value. Characters XML 1.0 cannot carry become spaces.
fn esc(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            '\t' | '\n' | '\r' => out.push(' '),
            c if (c as u32) < 0x20 => out.push(' '),
            c => out.push(c),
        }
    }
    out
}

/// A number with up to four decimals and no trailing zeros.
fn num(value: f64) -> String {
    let text = format!("{value:.4}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text.is_empty() || text == "-0" {
        "0".to_owned()
    } else {
        text.to_owned()
    }
}

fn hex(color: Rgba8) -> String {
    format!("#{:02X}{:02X}{:02X}", color.r, color.g, color.b)
}

fn alpha(color: Rgba8) -> String {
    num(f64::from(color.a) / 255.0)
}

/// A file stem that is safe on every platform and cannot escape a directory.
fn safe_stem(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, ' ' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = cleaned
        .trim_matches([' ', '.'])
        .chars()
        .take(80)
        .collect::<String>();
    if trimmed.is_empty() {
        "Symbol".to_owned()
    } else {
        trimmed
    }
}

struct Names {
    /// Library names (unique) and file names, in symbol order.
    library: Vec<(SymbolId, String, String)>,
}

impl Names {
    fn library_name(&self, id: SymbolId) -> &str {
        self.library
            .iter()
            .find(|(s, _, _)| *s == id)
            .map_or("", |(_, name, _)| name)
    }
}

// ---- shapes ---------------------------------------------------------------------------

fn twips(p: Point) -> Point {
    (p.0 * TWIPS_PER_PX, p.1 * TWIPS_PER_PX)
}

fn pt(p: Point) -> String {
    format!("{} {}", num(p.0), num(p.1))
}

struct Subpath {
    edges: String,
}

/// Encodes a shape's path as XFL edge strings, one per subpath.
fn encode_edges(shape: &Shape, approximated: &mut bool) -> Vec<Subpath> {
    let xy = |p: &flare_core::DocumentPoint| twips((p.as_vec().x, p.as_vec().y));
    let mut out: Vec<Subpath> = Vec::new();
    let mut current: Option<String> = None;
    let (mut start, mut at): (Point, Point) = ((0.0, 0.0), (0.0, 0.0));
    let flush = |current: &mut Option<String>, out: &mut Vec<Subpath>| {
        if let Some(edges) = current.take()
            && edges.contains(['|', '['])
        {
            out.push(Subpath { edges });
        }
    };
    for segment in shape.path() {
        match segment {
            PathSegment::MoveTo(p) => {
                flush(&mut current, &mut out);
                (start, at) = (xy(p), xy(p));
                current = Some(format!("!{}", pt(start)));
            }
            PathSegment::LineTo(p) => {
                at = xy(p);
                if let Some(edges) = &mut current {
                    edges.push_str(&format!("|{}", pt(at)));
                }
            }
            PathSegment::CubicTo(a, b, c) => {
                let cubic = [at, xy(a), xy(b), xy(c)];
                let quads = curves::to_quads(cubic, CURVE_TOLERANCE_PX * TWIPS_PER_PX);
                if quads.len() > 1 || cubic_is_curved(cubic) {
                    *approximated = true;
                }
                if let Some(edges) = &mut current {
                    for (control, end) in quads {
                        edges.push_str(&format!("[{} {}", pt(control), pt(end)));
                    }
                }
                at = cubic[3];
            }
            PathSegment::Close => {
                if let Some(edges) = &mut current
                    && at != start
                {
                    edges.push_str(&format!("|{}", pt(start)));
                }
                flush(&mut current, &mut out);
                at = start;
            }
        }
    }
    flush(&mut current, &mut out);
    out
}

fn cubic_is_curved(c: [Point; 4]) -> bool {
    let dx = c[3].0 - 3.0 * c[2].0 + 3.0 * c[1].0 - c[0].0;
    let dy = c[3].1 - 3.0 * c[2].1 + 3.0 * c[1].1 - c[0].1;
    dx.hypot(dy) > 1e-9
}

fn write_shape(
    out: &mut String,
    shape: &Shape,
    place: &str,
    notes: &mut Vec<Diagnostic>,
    approximated: &mut bool,
) {
    let fill = shape.flat_fill();
    let has_fill = fill.a > 0;
    if shape.gradient().is_some() {
        notes.push(Diagnostic {
            severity: Severity::Warning,
            location: place.to_owned(),
            message: "a gradient was written as a flat color".into(),
        });
    }
    let stroke = shape.stroke();
    if !has_fill && stroke.is_none() {
        notes.push(Diagnostic {
            severity: Severity::Info,
            location: place.to_owned(),
            message: "an invisible shape (no fill, no stroke) was skipped".into(),
        });
        return;
    }
    out.push_str("<DOMShape>");
    if has_fill {
        out.push_str(&format!(
            "<fills><FillStyle index=\"1\"><SolidColor color=\"{}\" alpha=\"{}\"/></FillStyle></fills>",
            hex(fill),
            alpha(fill)
        ));
    }
    if let Some(stroke) = stroke {
        out.push_str(&format!(
            "<strokes><StrokeStyle index=\"1\"><SolidStroke weight=\"{}\" caps=\"round\" joints=\"round\"><fill><SolidColor color=\"{}\" alpha=\"{}\"/></fill></SolidStroke></StrokeStyle></strokes>",
            num(stroke.width()),
            hex(stroke.color()),
            alpha(stroke.color())
        ));
    }
    out.push_str("<edges>");
    for subpath in encode_edges(shape, approximated) {
        let mut attrs = String::new();
        if has_fill {
            attrs.push_str(" fillStyle1=\"1\"");
        }
        if stroke.is_some() {
            attrs.push_str(" strokeStyle=\"1\"");
        }
        out.push_str(&format!("<Edge{attrs} edges=\"{}\"/>", subpath.edges));
    }
    out.push_str("</edges></DOMShape>");
}

// ---- timelines ------------------------------------------------------------------------

fn write_instance(out: &mut String, instance: &Instance, names: &Names) {
    let [a, b, c, d, tx, ty] = instance.transform().as_affine().to_cols_array();
    out.push_str(&format!(
        "<DOMSymbolInstance libraryItemName=\"{}\" symbolType=\"graphic\" loop=\"loop\"><matrix><Matrix a=\"{}\" b=\"{}\" c=\"{}\" d=\"{}\" tx=\"{}\" ty=\"{}\"/></matrix></DOMSymbolInstance>",
        esc(names.library_name(instance.symbol())),
        num(a), num(b), num(c), num(d), num(tx), num(ty)
    ));
}

/// Whether `xml` is exactly one well-formed element with no DTD. Preserved source data
/// came from a file, so it is checked before being written into the output.
fn well_formed(xml: &str) -> bool {
    use quick_xml::events::Event;
    let mut reader = quick_xml::Reader::from_str(xml);
    let (mut depth, mut roots) = (0_usize, 0_usize);
    loop {
        match reader.read_event() {
            Ok(Event::Start(_)) => {
                if depth == 0 {
                    roots += 1;
                }
                depth += 1;
            }
            Ok(Event::Empty(_)) => {
                if depth == 0 {
                    roots += 1;
                }
            }
            Ok(Event::End(_)) => match depth.checked_sub(1) {
                Some(d) => depth = d,
                None => return false,
            },
            Ok(Event::DocType(_) | Event::PI(_)) => return false,
            Ok(Event::Text(t)) if depth == 0 && !t.xml10_content().trim().is_empty() => {
                return false;
            }
            Ok(Event::Eof) => return depth == 0 && roots == 1,
            Err(_) => return false,
            Ok(_) => {}
        }
    }
}

struct Ctx<'a> {
    names: &'a Names,
    notes: Vec<Diagnostic>,
    approximated: bool,
    /// Source data kept by the importer, and which of it has been written back.
    attachments: &'a [Attachment],
    used: Vec<bool>,
}

impl<'a> Ctx<'a> {
    /// The unused attachments on `anchor` whose placement satisfies `wanted`, marked as
    /// used. Malformed ones are reported and skipped rather than written.
    fn take(
        &mut self,
        anchor: Anchor,
        wanted: impl Fn(Placement) -> bool,
        place: &str,
    ) -> Vec<&'a str> {
        let mut out = Vec::new();
        for (index, attachment) in self.attachments.iter().enumerate() {
            if self.used[index] || attachment.anchor() != anchor || !wanted(attachment.placement())
            {
                continue;
            }
            self.used[index] = true;
            if attachment.format() != "xfl" {
                self.notes.push(Diagnostic {
                    severity: Severity::Warning,
                    location: place.to_owned(),
                    message: format!(
                        "preserved source data in the unknown format {:?} was not written",
                        attachment.format()
                    ),
                });
                continue;
            }
            if well_formed(attachment.content()) {
                out.push(attachment.content());
            } else {
                self.notes.push(Diagnostic {
                    severity: Severity::Warning,
                    location: place.to_owned(),
                    message: "preserved source data is not well-formed XML and was not written"
                        .into(),
                });
            }
        }
        out
    }
}

fn write_frame(out: &mut String, span: &FrameSpan, layer: Anchor, place: &str, ctx: &mut Ctx<'_>) {
    let tween = match (span.tween(), span.content()) {
        (Tween::Linear, Content::Instance(_)) => " tweenType=\"motion\"",
        _ => "",
    };
    out.push_str(&format!(
        "<DOMFrame index=\"{}\" duration=\"{}\"{tween}><elements>",
        span.start(),
        span.duration().get()
    ));
    match span.content() {
        Content::Empty => {}
        Content::Instance(instance) => write_instance(out, instance, ctx.names),
        Content::Shape(shape) => {
            let Ctx {
                notes,
                approximated,
                ..
            } = ctx;
            write_shape(out, shape, place, notes, approximated);
        }
    }
    let frame = span.start();
    for xml in ctx.take(layer, |p| p == Placement::FrameElement { frame }, place) {
        out.push_str(xml);
    }
    out.push_str("</elements>");
    for xml in ctx.take(layer, |p| p == Placement::FrameExtra { frame }, place) {
        out.push_str(xml);
    }
    out.push_str("</DOMFrame>");
}

fn write_layer(out: &mut String, layer: &Layer, place: &str, ctx: &mut Ctx<'_>) {
    let mut attrs = String::new();
    if !layer.is_visible() {
        attrs.push_str(" visible=\"false\"");
    }
    if layer.is_locked() {
        attrs.push_str(" locked=\"true\"");
    }
    out.push_str(&format!(
        "<DOMLayer name=\"{}\"{attrs}><frames>",
        esc(layer.name())
    ));
    let here = format!("{place} / layer \"{}\"", layer.name());
    if layer.spans().is_empty() {
        // Animate requires every layer to have a frame; keep the layer alive on re-import.
        ctx.notes.push(Diagnostic {
            severity: Severity::Info,
            location: here.clone(),
            message: "an empty layer was written with one empty frame".into(),
        });
        out.push_str("<DOMFrame index=\"0\" duration=\"1\"><elements>");
        let anchor = Anchor::Layer(layer.id());
        for xml in ctx.take(anchor, |p| p == Placement::FrameElement { frame: 0 }, &here) {
            out.push_str(xml);
        }
        out.push_str("</elements>");
        for xml in ctx.take(anchor, |p| p == Placement::FrameExtra { frame: 0 }, &here) {
            out.push_str(xml);
        }
        out.push_str("</DOMFrame>");
    }
    let mut cursor = 0_u64;
    for span in layer.spans() {
        if u64::from(span.start()) > cursor {
            let gap = u64::from(span.start()) - cursor;
            ctx.notes.push(Diagnostic {
                severity: Severity::Warning,
                location: here.clone(),
                message: format!(
                    "a gap of {gap} frame(s) before frame {} became empty frames",
                    span.start()
                ),
            });
            out.push_str(&format!(
                "<DOMFrame index=\"{cursor}\" duration=\"{gap}\"><elements/></DOMFrame>"
            ));
        }
        write_frame(out, span, Anchor::Layer(layer.id()), &here, ctx);
        cursor = span.end();
    }
    out.push_str("</frames></DOMLayer>");
}

fn write_timeline(
    out: &mut String,
    name: &str,
    timeline: &Timeline,
    place: &str,
    owner: Anchor,
    ctx: &mut Ctx<'_>,
) {
    out.push_str(&format!("<DOMTimeline name=\"{}\"><layers>", esc(name)));
    let count = timeline.layers().len();
    for (position, layer) in timeline.layers().iter().enumerate() {
        // Whole layers that were kept go back at their original index among the layers.
        let at =
            |p: Placement| matches!(p, Placement::LayerAt { index } if index as usize == position);
        for xml in ctx.take(owner, at, place) {
            out.push_str(xml);
        }
        write_layer(out, layer, place, ctx);
    }
    // Any whose index lies beyond the layers there are now go last.
    let after = |p: Placement| matches!(p, Placement::LayerAt { index } if index as usize >= count);
    for xml in ctx.take(owner, after, place) {
        out.push_str(xml);
    }
    out.push_str("</layers></DOMTimeline>");
}

// ---- document -------------------------------------------------------------------------

const PROLOG: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"no\"?>\n";

fn frame_rate_text(rate: flare_core::FrameRate) -> String {
    if rate.denominator() == 1 {
        rate.numerator().to_string()
    } else {
        // Three decimals keep NTSC rates (29.970...) recognisable on re-import.
        num((f64::from(rate.numerator()) / f64::from(rate.denominator()) * 1000.0).round() / 1000.0)
    }
}

/// Builds the files of an XFL document. Nothing is written to disk.
pub fn export_xfl(document: &Document) -> Result<Output, ExportError> {
    let mut notes: Vec<Diagnostic> = Vec::new();

    // Library names must be unique and file names must be safe and distinct.
    let mut used_names: HashSet<String> = HashSet::new();
    let mut used_files: HashSet<String> = HashSet::new();
    let mut library = Vec::new();
    for symbol in document.symbols() {
        let base = if symbol.name().is_empty() {
            "Symbol"
        } else {
            symbol.name()
        };
        let mut name = base.to_owned();
        let mut n = 2;
        while !used_names.insert(name.clone()) {
            name = format!("{base} {n}");
            n += 1;
        }
        if name != base {
            notes.push(Diagnostic {
                severity: Severity::Warning,
                location: format!("symbol \"{base}\""),
                message: format!(
                    "the library needs unique names, so one copy was renamed \"{name}\""
                ),
            });
        }
        let stem = safe_stem(&name);
        let mut file = format!("{stem}.xml");
        let mut k = 2;
        while !used_files.insert(file.to_lowercase()) {
            file = format!("{stem} {k}.xml");
            k += 1;
        }
        library.push((symbol.id(), name, file));
    }
    let names = Names { library };
    let mut ctx = Ctx {
        names: &names,
        notes: Vec::new(),
        approximated: false,
        attachments: document.attachments(),
        used: vec![false; document.attachments().len()],
    };

    let stage = document.stage();
    if stage.background().a != 255 {
        notes.push(Diagnostic {
            severity: Severity::Warning,
            location: "stage".into(),
            message: "the background's transparency cannot be stored; it was written opaque".into(),
        });
    }

    let mut main = String::from(PROLOG);
    main.push_str(&format!(
        "<DOMDocument xmlns=\"{NAMESPACE}\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" frameRate=\"{}\" width=\"{}\" height=\"{}\" backgroundColor=\"{}\" currentTimeline=\"1\" xflVersion=\"2.2\">",
        frame_rate_text(document.frame_rate()),
        stage.width(),
        stage.height(),
        hex(stage.background())
    ));
    if !names.library.is_empty() {
        main.push_str("<symbols>");
        for (_, _, file) in &names.library {
            main.push_str(&format!("<Include href=\"{}\"/>", esc(file)));
        }
        main.push_str("</symbols>");
    }
    main.push_str("<timelines>");
    for scene in document.scenes() {
        write_timeline(
            &mut main,
            scene.name(),
            scene.timeline(),
            &format!("scene \"{}\"", scene.name()),
            Anchor::Scene(scene.id()),
            &mut ctx,
        );
    }
    main.push_str("</timelines>");
    for xml in ctx.take(Anchor::Document, |p| p == Placement::Child, "document") {
        main.push_str(xml);
    }
    main.push_str("</DOMDocument>\n");

    let mut files = vec![("DOMDocument.xml".to_owned(), main)];
    for (symbol, (_, name, file)) in document.symbols().iter().zip(&names.library) {
        let mut item = String::from(PROLOG);
        item.push_str(&format!(
            "<DOMSymbolItem xmlns=\"{NAMESPACE}\" name=\"{}\" symbolType=\"graphic\"><timeline>",
            esc(name)
        ));
        write_timeline(
            &mut item,
            name,
            symbol.timeline(),
            &format!("symbol \"{name}\""),
            Anchor::Symbol(symbol.id()),
            &mut ctx,
        );
        item.push_str("</timeline>");
        for xml in ctx.take(
            Anchor::Symbol(symbol.id()),
            |p| p == Placement::Child,
            &format!("symbol \"{name}\""),
        ) {
            item.push_str(xml);
        }
        item.push_str("</DOMSymbolItem>\n");
        files.push((format!("LIBRARY/{file}"), item));
    }

    // Source data whose keyframe no longer exists on its layer cannot be placed honestly.
    for (attachment, used) in ctx.attachments.iter().zip(&ctx.used) {
        if !used {
            ctx.notes.push(Diagnostic {
                severity: Severity::Warning,
                location: "source data".into(),
                message: format!(
                    "preserved source data ({:?}) could not be placed because its keyframe no longer exists; it was not written",
                    attachment.placement()
                ),
            });
        }
    }
    notes.append(&mut ctx.notes);
    if ctx.approximated {
        notes.push(Diagnostic {
            severity: Severity::Info,
            location: "shapes".into(),
            message: format!(
                "XFL edges cannot hold cubic curves, so they were approximated by quadratic segments within {CURVE_TOLERANCE_PX} px"
            ),
        });
    }
    let mut seen = HashSet::new();
    notes.retain(|d| seen.insert((d.severity as u8, d.location.clone(), d.message.clone())));
    let name = safe_stem(document.scenes().first().map_or("Document", |s| s.name()));
    Ok(Output {
        name,
        files,
        diagnostics: notes,
    })
}

// ---- writing --------------------------------------------------------------------------

/// Writes the XFL folder to `dir`, which must not exist. The folder appears only when it is
/// complete: files go to a sibling directory that is renamed into place.
pub fn write_directory(output: &Output, dir: &Path) -> Result<(), ExportError> {
    if dir.exists() {
        return Err(ExportError::AlreadyExists(dir.to_owned()));
    }
    let parent = dir
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;
    let staging = parent.join(format!(
        ".{}.partial-{}",
        dir.file_name()
            .map_or_else(|| "xfl".into(), |n| n.to_string_lossy().into_owned()),
        std::process::id()
    ));
    let result = (|| -> Result<(), ExportError> {
        fs::create_dir_all(&staging)?;
        for (name, body) in &output.files {
            let path = staging.join(name);
            if let Some(directory) = path.parent() {
                fs::create_dir_all(directory)?;
            }
            fs::write(path, body)?;
        }
        // The `.xfl` stub that tells Animate a folder is an XFL document.
        fs::write(staging.join(format!("{}.xfl", output.name)), "PROXY-CS5")?;
        fs::rename(&staging, dir)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&staging);
    }
    result
}

/// Writes a zipped modern `.fla` to `path`, which must not exist.
pub fn write_fla(output: &Output, path: &Path) -> Result<(), ExportError> {
    if path.exists() {
        return Err(ExportError::AlreadyExists(path.to_owned()));
    }
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;
    let (staging, mut file) = tempfile_in(parent)?;
    let result = (|| -> Result<(), ExportError> {
        {
            let mut zip = zip::ZipWriter::new(&mut file);
            let stored = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            let deflated = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            zip.start_file("mimetype", stored)?;
            zip.write_all(b"application/vnd.adobe.xfl")?;
            for (name, body) in &output.files {
                zip.start_file(name.as_str(), deflated)?;
                zip.write_all(body.as_bytes())?;
            }
            zip.finish()?;
        }
        file.flush()?;
        fs::rename(&staging, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&staging);
    }
    result
}

/// A uniquely named file in `dir` to stage output in; renamed into place on success.
fn tempfile_in(dir: &Path) -> Result<(PathBuf, fs::File), ExportError> {
    for attempt in 0..100 {
        let path = dir.join(format!(
            ".flare-export-{}-{attempt}.tmp",
            std::process::id()
        ));
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.into()),
        }
    }
    Err(std::io::Error::other("could not create a staging file").into())
}
