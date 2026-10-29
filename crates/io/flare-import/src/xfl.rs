//! Maps an XFL document (DOMDocument.xml plus LIBRARY/ items) onto a Flare document.
//!
//! The importer is deliberately explicit about its subset. Anything it cannot represent is
//! reported as a [`Diagnostic`] rather than silently dropped or guessed, and there is no
//! round-trip preservation of foreign data yet: what is not imported is lost on save.
//!
//! Imported: stage size, frame rate and background; scenes; normal layers with name,
//! visibility and lock; keyframes and their durations; symbol instances with their matrix;
//! solid-color fills and solid strokes of drawn shapes (lines and quadratic curves);
//! the library as Graphic symbols; classic motion tweens between instances.
//! Reported and skipped: gradients, bitmaps, text, video, sound, scripts, filters, color
//! effects, masks, guides, folders, primitives, shape tweens, 3D, and symbol runtime
//! behaviour (movie clips and buttons become Graphic symbols).

use std::collections::{BTreeMap, HashMap};
use std::io::{self, Read};

use flare_core::{
    DocumentId, FrameRate, InstanceId, LayerId, Rgba8, SceneId, ShapeId, SymbolId, Transform2D,
};
use flare_document::{
    Content, Document, FrameSpan, Instance, Layer, PathSegment, Scene, Shape, Stage, Stroke,
    Symbol, Timeline, Tween,
};
use glamx::DAffine2;
use thiserror::Error;

use crate::edges::{self, Contour, Segment};
use crate::xml::{self, Node, XmlError};

const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_SYMBOLS: usize = 10_000;
const MAX_DIAGNOSTICS: usize = 1_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Imported faithfully but worth knowing about (for example elements split over layers).
    Info,
    /// Imported with a change in meaning.
    Warning,
    /// Not imported.
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub location: String,
    pub message: String,
}

#[derive(Debug)]
pub struct Imported {
    pub document: Document,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Error)]
pub enum ImportError {
    #[error("not an XFL document: {0}")]
    NotXfl(String),
    #[error("i/o error: {0}")]
    Io(#[from] io::Error),
    #[error("{file}: {error}")]
    Xml { file: String, error: XmlError },
    #[error("{0} is larger than the {MAX_FILE_BYTES} byte limit")]
    TooLarge(String),
    #[error("the document has no timelines")]
    NoTimelines,
    #[error("the library has more than {MAX_SYMBOLS} items")]
    TooManySymbols,
    #[error("invalid {what}: {message}")]
    BadAttribute { what: String, message: String },
    #[error("the imported document is invalid: {0:?}")]
    Invalid(Vec<flare_document::Diagnostic>),
}

/// Where XFL files are read from. Names use `/` and are relative to the document root,
/// like `DOMDocument.xml` or `LIBRARY/Symbol 1.xml`.
pub trait FileSource {
    /// `Ok(None)` means the file does not exist.
    fn read(&self, name: &str) -> io::Result<Option<Vec<u8>>>;
}

/// Reads at most `MAX_FILE_BYTES + 1` bytes so an oversized file is detected without being
/// loaded in full.
pub(crate) fn read_capped(reader: &mut impl io::Read, name: &str) -> Result<Vec<u8>, ImportError> {
    let mut bytes = Vec::new();
    reader.take(MAX_FILE_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(ImportError::TooLarge(name.to_owned()));
    }
    Ok(bytes)
}

// ---- small attribute helpers ----------------------------------------------------------

fn color_from(text: &str, alpha: Option<&str>) -> Option<Rgba8> {
    let hex = text.strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
    let alpha = match alpha {
        None => 255,
        Some(a) => {
            let a: f64 = a.parse().ok().filter(|v: &f64| v.is_finite())?;
            (a.clamp(0.0, 1.0) * 255.0).round() as u8
        }
    };
    Some(Rgba8::new(byte(0)?, byte(2)?, byte(4)?, alpha))
}

fn bool_attr(node: &Node, key: &str, default: bool) -> bool {
    node.attr(key)
        .map_or(default, |v| v.eq_ignore_ascii_case("true"))
}

/// A frame rate as an exact rational: NTSC rates stay exact, others become n/1000.
pub fn frame_rate(text: &str) -> Result<FrameRate, String> {
    let rate: f64 = text
        .parse()
        .map_err(|_| format!("frame rate {text:?} is not a number"))?;
    if !rate.is_finite() || rate <= 0.0 || rate > 1000.0 {
        return Err(format!("frame rate {text:?} is out of range"));
    }
    for (num, den) in [(24_000_u32, 1001_u32), (30_000, 1001), (60_000, 1001)] {
        if (rate - f64::from(num) / f64::from(den)).abs() < 0.005 {
            return FrameRate::new(num, den).map_err(|e| e.to_string());
        }
    }
    FrameRate::new((rate * 1000.0).round() as u32, 1000).map_err(|e| e.to_string())
}

fn stage_side(node: &Node, key: &str, default: u32) -> Result<u32, ImportError> {
    let Some(text) = node.attr(key) else {
        return Ok(default);
    };
    let value: f64 = text.parse().map_err(|_| ImportError::BadAttribute {
        what: key.to_owned(),
        message: format!("{text:?} is not a number"),
    })?;
    if !value.is_finite() || value < 1.0 || value > f64::from(flare_document::MAX_STAGE_SIDE) {
        return Err(ImportError::BadAttribute {
            what: key.to_owned(),
            message: format!("{text:?} is outside the supported stage size"),
        });
    }
    Ok(value.round() as u32)
}

// ---- the importer ---------------------------------------------------------------------

struct Importer {
    symbols: HashMap<String, SymbolId>,
    diagnostics: Vec<Diagnostic>,
    suppressed: usize,
}

impl Importer {
    fn note(&mut self, severity: Severity, location: &str, message: impl Into<String>) {
        let diagnostic = Diagnostic {
            severity,
            location: location.to_owned(),
            message: message.into(),
        };
        if self.diagnostics.contains(&diagnostic) {
            return;
        }
        if self.diagnostics.len() >= MAX_DIAGNOSTICS {
            self.suppressed += 1;
        } else {
            self.diagnostics.push(diagnostic);
        }
    }

    fn timeline(&mut self, timeline: &Node, location: &str) -> Timeline {
        let Some(layers_node) = timeline.child("layers") else {
            return Timeline::default();
        };
        let mut layers = Vec::new();
        for layer in layers_node.children_named("DOMLayer") {
            let name = layer.attr("name").unwrap_or("Layer");
            let place = format!("{location} / layer \"{name}\"");
            match layer.attr("layerType") {
                None | Some("normal") => {}
                Some(other) => {
                    self.note(
                        Severity::Unsupported,
                        &place,
                        format!("{other} layers are not imported; the layer was skipped"),
                    );
                    continue;
                }
            }
            if layer.attr("parentLayerIndex").is_some() {
                self.note(Severity::Info, &place, "folder membership is flattened");
            }
            self.layer(layer, name, &place, &mut layers);
        }
        Timeline::new(layers)
    }

    /// Imports one DOMLayer. A keyframe may hold several elements but a span holds one, so
    /// extra elements go to additional layers right below, named "<name> (2)" and so on.
    fn layer(&mut self, layer: &Node, name: &str, place: &str, out: &mut Vec<Layer>) {
        let mut slots: Vec<Vec<FrameSpan>> = Vec::new();
        let frames = layer
            .child("frames")
            .into_iter()
            .flat_map(|f| f.children_named("DOMFrame"));
        for frame in frames {
            let Some(index) = frame.attr("index").and_then(|v| v.parse::<u32>().ok()) else {
                self.note(
                    Severity::Unsupported,
                    place,
                    "a frame without a valid index was skipped",
                );
                continue;
            };
            let duration = match frame.attr("duration") {
                None => 1,
                Some(text) => match text.parse::<u32>() {
                    Ok(n) if n >= 1 => n,
                    _ => {
                        self.note(
                            Severity::Unsupported,
                            place,
                            format!(
                                "frame {index} has an invalid duration {text:?} and was skipped"
                            ),
                        );
                        continue;
                    }
                },
            };
            let Some(duration) = std::num::NonZeroU32::new(duration) else {
                continue;
            };
            let here = format!("{place} / frame {index}");
            self.frame_extras(frame, &here);
            let contents = frame
                .child("elements")
                .map(|e| self.elements(e, &here))
                .unwrap_or_default();
            let contents = if contents.is_empty() {
                vec![Content::Empty]
            } else {
                contents
            };
            let tween = self.tween_for(frame, &contents, &here);
            for (slot, content) in contents.into_iter().enumerate() {
                if slots.len() <= slot {
                    slots.push(Vec::new());
                }
                let mut span = FrameSpan::new(index, duration, content);
                if slot == 0 {
                    span = span.with_tween(tween);
                }
                slots[slot].push(span);
            }
        }
        if slots.len() > 1 {
            self.note(
                Severity::Info,
                place,
                format!(
                    "{} elements share a keyframe; the extras were placed on additional layers above the first, preserving stacking",
                    slots.len()
                ),
            );
        }
        let visible = bool_attr(layer, "visible", true);
        let locked = bool_attr(layer, "locked", false);
        // XFL lists a frame's elements bottom to top, so later elements must sit above
        // earlier ones: the extra layers go above the first, highest slot first.
        let mut made = Vec::new();
        for (slot, spans) in slots.into_iter().enumerate() {
            let layer_name = if slot == 0 {
                name.to_owned()
            } else {
                format!("{name} ({})", slot + 1)
            };
            match Layer::new(LayerId::new(), layer_name, spans) {
                Ok(l) => made.push(l.with_visible(visible).with_locked(locked)),
                Err(_) => self.note(
                    Severity::Unsupported,
                    place,
                    "frames overlap; the layer was skipped",
                ),
            }
        }
        out.extend(made.into_iter().rev());
    }

    fn frame_extras(&mut self, frame: &Node, place: &str) {
        for child in &frame.children {
            let (severity, message) = match child.name.as_str() {
                "elements" => continue,
                "Actionscript" => (Severity::Unsupported, "frame scripts are not imported"),
                "sound" | "DOMSound" => (Severity::Unsupported, "frame sounds are not imported"),
                "tweens" | "Motion" | "motionObjectXML" => (
                    Severity::Unsupported,
                    "motion-tween property editors are not imported",
                ),
                other => {
                    self.note(Severity::Info, place, format!("<{other}> was ignored"));
                    continue;
                }
            };
            self.note(severity, place, message);
        }
        if frame.attr("name").is_some() {
            self.note(Severity::Info, place, "frame labels are not imported");
        }
    }

    fn tween_for(&mut self, frame: &Node, contents: &[Content], place: &str) -> Tween {
        match frame.attr("tweenType") {
            None | Some("none") => Tween::None,
            Some("motion") if matches!(contents.first(), Some(Content::Instance(_))) => {
                if frame.attr("acceleration").is_some_and(|a| a != "0") {
                    self.note(
                        Severity::Warning,
                        place,
                        "tween easing is ignored; the tween is linear",
                    );
                }
                Tween::Linear
            }
            Some("motion") => {
                self.note(
                    Severity::Unsupported,
                    place,
                    "only instance transforms can be tweened",
                );
                Tween::None
            }
            Some(other) => {
                self.note(
                    Severity::Unsupported,
                    place,
                    format!("{other} tweens are not imported"),
                );
                Tween::None
            }
        }
    }

    fn elements(&mut self, elements: &Node, place: &str) -> Vec<Content> {
        let mut out = Vec::new();
        for element in &elements.children {
            match element.name.as_str() {
                "DOMShape" => out.extend(self.shape(element, place)),
                "DOMSymbolInstance" => out.extend(self.instance(element, place)),
                "DOMGroup" => {
                    self.note(
                        Severity::Info,
                        place,
                        "groups are flattened into their members",
                    );
                    if let Some(members) = element.child("members") {
                        out.extend(self.elements(members, place));
                    }
                }
                other => self.note(
                    Severity::Unsupported,
                    place,
                    format!("{other} is not imported"),
                ),
            }
        }
        out
    }

    fn instance(&mut self, node: &Node, place: &str) -> Option<Content> {
        let target = node.attr("libraryItemName").unwrap_or_default();
        let Some(symbol) = self.symbols.get(target).copied() else {
            self.note(
                Severity::Unsupported,
                place,
                format!("instance of missing symbol \"{target}\" was skipped"),
            );
            return None;
        };
        if node.child("color").is_some() {
            self.note(
                Severity::Unsupported,
                place,
                "instance color effects are ignored",
            );
        }
        if node.child("filters").is_some() {
            self.note(Severity::Unsupported, place, "instance filters are ignored");
        }
        if node.attr("blendMode").is_some_and(|b| b != "normal") {
            self.note(Severity::Unsupported, place, "blend modes are ignored");
        }
        let matrix = node.child("matrix").and_then(|m| m.child("Matrix"));
        let get = |key: &str, default: f64| {
            matrix.and_then(|m| m.attr(key)).map_or(Some(default), |v| {
                v.parse::<f64>().ok().filter(|n| n.is_finite())
            })
        };
        let values = [
            get("a", 1.0),
            get("b", 0.0),
            get("c", 0.0),
            get("d", 1.0),
            get("tx", 0.0),
            get("ty", 0.0),
        ];
        let transform = if let [Some(a), Some(b), Some(c), Some(d), Some(tx), Some(ty)] = values {
            Transform2D::new(DAffine2::from_cols_array(&[a, b, c, d, tx, ty])).ok()
        } else {
            None
        };
        let Some(transform) = transform else {
            self.note(
                Severity::Unsupported,
                place,
                "an instance with an invalid matrix was skipped",
            );
            return None;
        };
        Some(Content::Instance(Instance::new(
            InstanceId::new(),
            symbol,
            transform,
        )))
    }

    fn shape(&mut self, node: &Node, place: &str) -> Vec<Content> {
        let mut fills: HashMap<u32, Option<Rgba8>> = HashMap::new();
        for style in node
            .child("fills")
            .into_iter()
            .flat_map(|f| f.children_named("FillStyle"))
        {
            let Some(index) = style.attr("index").and_then(|v| v.parse().ok()) else {
                continue;
            };
            let color = style
                .child("SolidColor")
                .and_then(|c| color_from(c.attr("color").unwrap_or("#000000"), c.attr("alpha")));
            if color.is_none() {
                self.note(
                    Severity::Unsupported,
                    place,
                    "gradient and bitmap fills are not imported",
                );
            }
            fills.insert(index, color);
        }
        let mut strokes: HashMap<u32, Option<(f64, Rgba8)>> = HashMap::new();
        for style in node
            .child("strokes")
            .into_iter()
            .flat_map(|s| s.children_named("StrokeStyle"))
        {
            let Some(index) = style.attr("index").and_then(|v| v.parse().ok()) else {
                continue;
            };
            let solid = style.child("SolidStroke");
            let color = solid
                .and_then(|s| s.child("fill"))
                .and_then(|f| f.child("SolidColor"))
                .and_then(|c| color_from(c.attr("color").unwrap_or("#000000"), c.attr("alpha")));
            let weight = solid.map_or(Some(1.0), |s| {
                s.attr("weight")
                    .map_or(Some(1.0), |w| w.parse::<f64>().ok())
            });
            let resolved = color.zip(weight).map(|(c, w)| (w, c));
            if resolved.is_none() {
                self.note(
                    Severity::Unsupported,
                    place,
                    "only solid strokes are imported",
                );
            }
            strokes.insert(index, resolved);
        }

        let mut fill_edges: BTreeMap<u32, Vec<Segment>> = BTreeMap::new();
        let mut stroke_edges: BTreeMap<u32, Vec<Segment>> = BTreeMap::new();
        for edge in node
            .child("edges")
            .into_iter()
            .flat_map(|e| e.children_named("Edge"))
        {
            let segments = match edges::decode_edges(edge.attr("edges").unwrap_or_default()) {
                Ok(segments) => segments,
                Err(error) => {
                    self.note(
                        Severity::Unsupported,
                        place,
                        format!("an edge with bad data was skipped ({error})"),
                    );
                    continue;
                }
            };
            let index = |key: &str| {
                edge.attr(key)
                    .and_then(|v| v.parse::<u32>().ok())
                    .filter(|i| *i > 0)
            };
            // The right-hand fill follows the edge direction, the left-hand one is reversed so
            // each fill's boundary is oriented consistently.
            if let Some(i) = index("fillStyle1") {
                fill_edges
                    .entry(i)
                    .or_default()
                    .extend(segments.iter().copied());
            }
            if let Some(i) = index("fillStyle0") {
                fill_edges
                    .entry(i)
                    .or_default()
                    .extend(segments.iter().rev().map(Segment::reversed));
            }
            if let Some(i) = index("strokeStyle") {
                stroke_edges
                    .entry(i)
                    .or_default()
                    .extend(segments.iter().copied());
            }
        }

        let mut out = Vec::new();
        for (index, segments) in fill_edges {
            let Some(Some(color)) = fills.get(&index).copied() else {
                continue;
            };
            let contours = edges::chain(&segments);
            if contours.iter().any(|c| !c.closed) {
                self.note(Severity::Info, place, "an open fill outline was closed");
            }
            self.push_shape(&mut out, &contours, color, None, true, place);
        }
        for (index, segments) in stroke_edges {
            let Some(Some((weight, color))) = strokes.get(&index).copied() else {
                continue;
            };
            let contours = edges::chain(&segments);
            self.push_shape(
                &mut out,
                &contours,
                Rgba8::TRANSPARENT,
                Some((weight, color)),
                false,
                place,
            );
        }
        out
    }

    fn push_shape(
        &mut self,
        out: &mut Vec<Content>,
        contours: &[Contour],
        fill: Rgba8,
        stroke: Option<(f64, Rgba8)>,
        close: bool,
        place: &str,
    ) {
        // Edge coordinates are twips; document units are pixels.
        let point = |(x, y): edges::Point| flare_core::DocumentPoint::new(x / 20.0, y / 20.0).ok();
        let mut path = Vec::new();
        for contour in contours {
            let Some(first) = contour.segments.first().and_then(|s| point(s.from())) else {
                continue;
            };
            path.push(PathSegment::MoveTo(first));
            for segment in &contour.segments {
                let converted = match *segment {
                    Segment::Line { to, .. } => point(to).map(PathSegment::LineTo),
                    Segment::Quad { from, ctrl, to } => {
                        let (c1, c2) = edges::quad_to_cubic(from, ctrl, to);
                        point(c1)
                            .zip(point(c2))
                            .zip(point(to))
                            .map(|((a, b), c)| PathSegment::CubicTo(a, b, c))
                    }
                };
                if let Some(c) = converted {
                    path.push(c);
                }
            }
            if close || contour.closed {
                path.push(PathSegment::Close);
            }
        }
        if path.is_empty() {
            return;
        }
        match Shape::new(ShapeId::new(), path, fill) {
            Ok(shape) => {
                let shape = match stroke {
                    Some((weight, color)) => match Stroke::new(weight, color) {
                        Ok(stroke) => shape.with_stroke(stroke),
                        Err(error) => {
                            self.note(
                                Severity::Unsupported,
                                place,
                                format!("a stroke was skipped ({error})"),
                            );
                            return;
                        }
                    },
                    None => shape,
                };
                out.push(Content::Shape(shape));
            }
            Err(error) => self.note(
                Severity::Unsupported,
                place,
                format!("a shape was skipped ({error})"),
            ),
        }
    }
}

/// Imports an XFL document from `source`.
pub fn import_xfl(source: &dyn FileSource) -> Result<Imported, ImportError> {
    let read = |name: &str| -> Result<Option<Node>, ImportError> {
        let Some(bytes) = source.read(name)? else {
            return Ok(None);
        };
        if bytes.len() as u64 > MAX_FILE_BYTES {
            return Err(ImportError::TooLarge(name.to_owned()));
        }
        xml::parse(&bytes)
            .map(Some)
            .map_err(|error| ImportError::Xml {
                file: name.to_owned(),
                error,
            })
    };
    let root = read("DOMDocument.xml")?
        .ok_or_else(|| ImportError::NotXfl("DOMDocument.xml was not found".into()))?;
    if root.name != "DOMDocument" {
        return Err(ImportError::NotXfl(format!(
            "the root element is <{}>, expected <DOMDocument>",
            root.name
        )));
    }

    let mut importer = Importer {
        symbols: HashMap::new(),
        diagnostics: Vec::new(),
        suppressed: 0,
    };

    // Pass 1: read every library item and assign IDs, so symbols can refer to each other.
    let mut items: Vec<(String, SymbolId, Node)> = Vec::new();
    let includes: Vec<&Node> = root
        .child("symbols")
        .into_iter()
        .flat_map(|s| s.children_named("Include"))
        .collect();
    if includes.len() > MAX_SYMBOLS {
        return Err(ImportError::TooManySymbols);
    }
    for include in includes {
        let href = include.attr("href").unwrap_or_default();
        let file = format!("LIBRARY/{href}");
        let Some(item) = read(&file)? else {
            importer.note(
                Severity::Unsupported,
                &file,
                "library item not found; instances of it will be skipped",
            );
            continue;
        };
        if item.name != "DOMSymbolItem" {
            importer.note(
                Severity::Unsupported,
                &file,
                format!("unexpected <{}> in the library", item.name),
            );
            continue;
        }
        let Some(name) = item.attr("name").map(str::to_owned) else {
            importer.note(
                Severity::Unsupported,
                &file,
                "library item without a name was skipped",
            );
            continue;
        };
        let id = SymbolId::new();
        importer.symbols.insert(name.clone(), id);
        items.push((name, id, item));
    }

    // Pass 2: build the symbols' timelines.
    let mut symbols = Vec::with_capacity(items.len());
    for (name, id, item) in &items {
        let place = format!("symbol \"{name}\"");
        match item.attr("symbolType") {
            Some("graphic") => {}
            Some("button") => importer.note(Severity::Warning, &place, "a button was imported as a Graphic symbol; its states and events are not preserved"),
            _ => importer.note(Severity::Warning, &place, "a movie clip was imported as a Graphic symbol; its independent clock and scripts are not preserved"),
        }
        let timeline = item
            .child("timeline")
            .and_then(|t| t.child("DOMTimeline"))
            .map_or_else(Timeline::default, |t| importer.timeline(t, &place));
        symbols.push(Symbol::new(*id, name.clone(), timeline));
    }

    let mut scenes = Vec::new();
    for node in root
        .child("timelines")
        .into_iter()
        .flat_map(|t| t.children_named("DOMTimeline"))
    {
        let name = node.attr("name").unwrap_or("Scene").to_owned();
        let timeline = importer.timeline(node, &format!("scene \"{name}\""));
        scenes.push(Scene::new(SceneId::new(), name, timeline));
    }
    if scenes.is_empty() {
        return Err(ImportError::NoTimelines);
    }

    let rate = frame_rate(root.attr("frameRate").unwrap_or("24")).map_err(|message| {
        ImportError::BadAttribute {
            what: "frameRate".into(),
            message,
        }
    })?;
    let background = root
        .attr("backgroundColor")
        .and_then(|c| color_from(c, None))
        .unwrap_or(Rgba8::WHITE);
    let stage = Stage::new(
        stage_side(&root, "width", 550)?,
        stage_side(&root, "height", 400)?,
        background,
    )
    .map_err(|e| ImportError::BadAttribute {
        what: "stage".into(),
        message: e.to_string(),
    })?;

    let document = Document::new(DocumentId::new(), rate, scenes, symbols)
        .map_err(ImportError::Invalid)?
        .with_stage(stage);
    if importer.suppressed > 0 {
        let message = format!(
            "{} further diagnostics were suppressed",
            importer.suppressed
        );
        importer.diagnostics.push(Diagnostic {
            severity: Severity::Info,
            location: "import".into(),
            message,
        });
    }
    Ok(Imported {
        document,
        diagnostics: importer.diagnostics,
    })
}
