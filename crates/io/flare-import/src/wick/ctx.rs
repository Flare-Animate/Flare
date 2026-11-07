//! The state of one import: the object table, the symbols, bitmaps and source text made so far, and
//! the notes. A clip or a button becomes a symbol here, and its timeline is built by `timeline`.
//!
//! Wick places a clip by the centre of its content's bounds (Paper.js rotates and scales a group
//! about its bounds centre). The importer measures that centre from the clip's own paths on every
//! frame, which is an approximation: Wick measures the bounds of what is drawn at each moment.

use std::collections::{BTreeMap, HashMap, HashSet};

use flare_core::{InstanceId, Rgba8, ShapeId, SymbolId, Transform2D};
use flare_document::{
    Anchor, Attachment, BitmapInstance, Content, Instance, Placement, PlayMode, Playback, Shape,
    Symbol, SymbolKind, Timeline,
};
use glamx::{DAffine2, DVec2};
use serde_json::Value;

use super::error::WickError;
use super::json::{Objects, array, child_ids, class_of, text};
use super::library::Library;
use super::limits::{MAX_DEPTH, MAX_SYMBOLS};
use super::paths;
use crate::xfl::{Diagnostic, Severity};

/// A clip's or button's stored transform: where it is, how it is scaled and turned, and its opacity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Values {
    pub x: f64,
    pub y: f64,
    pub scale_x: f64,
    pub scale_y: f64,
    pub rotation: f64,
    pub opacity: f64,
}

impl Values {
    pub fn read(value: Option<&Value>) -> Values {
        let get = |key: &str, default: f64| {
            value
                .and_then(|v| v.get(key))
                .and_then(Value::as_f64)
                .filter(|v| v.is_finite())
                .unwrap_or(default)
        };
        Values {
            x: get("x", 0.0),
            y: get("y", 0.0),
            scale_x: get("scaleX", 1.0),
            scale_y: get("scaleY", 1.0),
            rotation: get("rotation", 0.0),
            opacity: get("opacity", 1.0),
        }
    }

    /// The transform from a symbol's own space to its parent's: move the content's centre to the
    /// position, then turn and scale about it.
    pub fn affine(&self, centre: DVec2) -> DAffine2 {
        // Wick does not allow a zero scale; it protects the same way.
        let scale = |s: f64| if s == 0.0 { 0.001 } else { s };
        DAffine2::from_translation(DVec2::new(self.x, self.y))
            * DAffine2::from_angle(self.rotation.to_radians())
            * DAffine2::from_scale(DVec2::new(scale(self.scale_x), scale(self.scale_y)))
            * DAffine2::from_translation(-centre)
    }
}

/// A clip or button placed in a frame: its symbol, how it plays, and its stored transform.
#[derive(Debug, Clone, Copy)]
pub(super) struct Inst {
    pub symbol: SymbolId,
    pub playback: Playback,
    pub values: Values,
    pub centre: DVec2,
}

impl Inst {
    pub fn content(&self, values: Values) -> Result<Content, WickError> {
        let transform = Transform2D::new(values.affine(self.centre))
            .map_err(|_| WickError::BadPath("has a transform that is not finite"))?;
        Ok(Content::Instance(
            Instance::new(InstanceId::new(), self.symbol, transform).with_playback(self.playback),
        ))
    }
}

struct Note {
    severity: Severity,
    text: &'static str,
    count: u64,
}

pub(super) struct Ctx<'a> {
    pub objects: &'a Objects,
    pub files: &'a HashMap<String, Vec<u8>>,
    pub symbols: Vec<Symbol>,
    pub library: Library,
    pub attachments: Vec<Attachment>,
    names: HashSet<String>,
    stack: Vec<String>,
    counted: HashSet<(String, &'static str)>,
    notes: BTreeMap<&'static str, Note>,
}

impl<'a> Ctx<'a> {
    pub fn new(objects: &'a Objects, files: &'a HashMap<String, Vec<u8>>) -> Self {
        Ctx {
            objects,
            files,
            symbols: Vec::new(),
            library: Library::default(),
            attachments: Vec::new(),
            names: HashSet::new(),
            stack: Vec::new(),
            counted: HashSet::new(),
            notes: BTreeMap::new(),
        }
    }

    /// Records a note about a kind of thing, counting each occurrence once per object.
    pub fn note(
        &mut self,
        object: &str,
        key: &'static str,
        severity: Severity,
        text: &'static str,
    ) {
        if !self.counted.insert((object.to_owned(), key)) {
            return;
        }
        self.notes
            .entry(key)
            .or_insert(Note {
                severity,
                text,
                count: 0,
            })
            .count += 1;
    }

    /// The diagnostics, one per kind of note, with how many objects it covers.
    pub fn diagnostics(&self, location: &str) -> Vec<Diagnostic> {
        self.notes
            .values()
            .map(|n| Diagnostic {
                severity: n.severity,
                location: location.to_owned(),
                message: format!("{} ({} found)", n.text, n.count),
            })
            .collect()
    }

    /// A clip or a button as a symbol, with its timeline built, and the instance that places it.
    pub fn instance(&mut self, id: &str) -> Result<Inst, WickError> {
        let objects = self.objects;
        let obj = objects.get(id)?;
        let class = class_of(obj);
        if class != "Clip" && class != "Button" {
            return Err(WickError::BadObject(
                id.chars().take(64).collect(),
                "clip or button",
            ));
        }
        if self.symbols.len() >= MAX_SYMBOLS {
            return Err(WickError::TooLarge("the project's clips and buttons"));
        }
        if self.stack.len() >= MAX_DEPTH {
            return Err(WickError::TooLarge("the depth of clips inside clips"));
        }
        if self.stack.iter().any(|s| s == id) {
            return Err(WickError::Cycle(id.chars().take(64).collect()));
        }
        let timeline_id = objects.child_of_class(obj, "Timeline")?;
        let symbol_id = SymbolId::new();
        let base = text(obj, "identifier")
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(if class == "Button" { "Button" } else { "Clip" });
        let name = self.unique_name(base);
        let place = format!("{class} \"{name}\"");

        self.stack.push(id.to_owned());
        let timeline = super::timeline::build(self, &timeline_id, Anchor::Symbol(symbol_id));
        self.stack.pop();
        let timeline: Timeline = timeline?;

        self.scripts(id, obj, Anchor::Symbol(symbol_id), &place);
        let centre = self.centre(&timeline_id)?;
        let (kind, playback) = if class == "Button" {
            self.note(
                id,
                "button",
                Severity::Warning,
                "buttons keep their type, but their states and scripts are not run",
            );
            (SymbolKind::Button, Playback::default())
        } else {
            let (kind, mode, first_frame) = match text(obj, "animationType").unwrap_or("loop") {
                "loop" => (SymbolKind::MovieClip, PlayMode::Loop, 0),
                "playOnce" => (SymbolKind::Graphic, PlayMode::PlayOnce, 0),
                "single" => {
                    let frame = obj
                        .get("singleFrameNumber")
                        .and_then(Value::as_u64)
                        .filter(|n| *n >= 1)
                        .ok_or(WickError::BadField("singleFrameNumber"))?;
                    let first = u32::try_from(frame - 1)
                        .map_err(|_| WickError::BadField("singleFrameNumber"))?;
                    (SymbolKind::Graphic, PlayMode::SingleFrame, first)
                }
                _ => return Err(WickError::BadField("animationType")),
            };
            (kind, Playback { mode, first_frame })
        };
        let values = Values::read(obj.get("transformation"));
        if values.opacity != 1.0 {
            self.note(
                id,
                "clip opacity",
                Severity::Warning,
                "opacity of clips is not imported; Flare instances have no alpha",
            );
        }
        self.symbols
            .push(Symbol::new(symbol_id, name, timeline).with_kind(kind));
        Ok(Inst {
            symbol: symbol_id,
            playback,
            values,
            centre,
        })
    }

    fn unique_name(&mut self, base: &str) -> String {
        let base: String = base.chars().take(120).collect();
        if self.names.insert(base.clone()) {
            return base;
        }
        let mut n = 2;
        loop {
            let candidate = format!("{base} {n}");
            if self.names.insert(candidate.clone()) {
                return candidate;
            }
            n += 1;
        }
    }

    /// The centre of the bounds of the paths drawn in a timeline, on every frame.
    fn centre(&self, timeline_id: &str) -> Result<DVec2, WickError> {
        let timeline = self.objects.of_class(timeline_id, "Timeline")?;
        let mut points = Vec::new();
        for layer in child_ids(timeline)? {
            for frame in child_ids(self.objects.of_class(&layer, "Layer")?)? {
                for child in child_ids(self.objects.of_class(&frame, "Frame")?)? {
                    if self.objects.class(&child) != "Path" {
                        continue;
                    }
                    if let Some(json) = self.objects.get(&child)?.get("json") {
                        paths::points(json, DAffine2::IDENTITY, &mut points)?;
                    }
                }
            }
        }
        let Some(first) = points.first().copied() else {
            return Ok(DVec2::ZERO);
        };
        let (lo, hi) = points
            .iter()
            .fold((first, first), |(lo, hi), p| (lo.min(*p), hi.max(*p)));
        Ok((lo + hi) * 0.5)
    }

    /// The Flare content of a frame's drawing: its paths, pictures and text, in order.
    pub fn drawing(&mut self, ids: &[String]) -> Result<Vec<Content>, WickError> {
        let mut out = Vec::new();
        let objects = self.objects;
        for id in ids {
            let obj = objects.get(id)?;
            let json = obj
                .get("json")
                .ok_or(WickError::BadObject(id.chars().take(64).collect(), "path"))?;
            let (kind, props) = paths::split(json)?;
            match kind {
                "Path" | "CompoundPath" => {
                    let Some(drawn) = paths::read(json, DAffine2::IDENTITY)? else {
                        continue;
                    };
                    if drawn.gradient {
                        self.note(id, "gradient", Severity::Unsupported, "gradient fills are not imported; the shape is drawn in its first colour or not at all");
                    }
                    if drawn.fill.is_none() && drawn.stroke.is_none() {
                        continue;
                    }
                    if props.get("fillRule").and_then(Value::as_str) == Some("evenodd") {
                        self.note(
                            id,
                            "fill rule",
                            Severity::Info,
                            "fill rules are not carried; shapes fill as Flare fills them",
                        );
                    }
                    let fill = drawn.fill.unwrap_or(Rgba8::new(0, 0, 0, 0));
                    let Ok(mut shape) = Shape::new(ShapeId::new(), drawn.segments, fill) else {
                        self.note(
                            id,
                            "shape",
                            Severity::Unsupported,
                            "shapes that could not be made are skipped",
                        );
                        continue;
                    };
                    if let Some(stroke) = drawn.stroke {
                        shape = shape.with_stroke(stroke);
                    }
                    out.push(Content::Shape(shape));
                }
                "Raster" => {
                    if let Some(content) = self.raster(id, props)? {
                        out.push(content);
                    }
                }
                "PointText" => self.note(
                    id,
                    "text",
                    Severity::Unsupported,
                    "text is not imported; its words and font are not carried",
                ),
                _ => self.note(
                    id,
                    "unknown",
                    Severity::Unsupported,
                    "items of an unknown kind are not imported",
                ),
            }
        }
        Ok(out)
    }

    fn raster(&mut self, id: &str, props: &Value) -> Result<Option<Content>, WickError> {
        let objects = self.objects;
        let source = text(props, "source").unwrap_or("");
        let Some(asset) = source.strip_prefix("asset:") else {
            self.note(
                id,
                "inline picture",
                Severity::Unsupported,
                "pictures stored inside the project are not imported",
            );
            return Ok(None);
        };
        let Ok(asset_obj) = objects.get(asset) else {
            self.note(
                id,
                "picture",
                Severity::Unsupported,
                "pictures that are missing or unreadable are not imported",
            );
            return Ok(None);
        };
        let ext = text(asset_obj, "fileExtension").unwrap_or("");
        if ext != "png" {
            self.note(
                id,
                "picture kind",
                Severity::Unsupported,
                "only PNG pictures are imported",
            );
            return Ok(None);
        }
        let file = format!("{asset}.png");
        let Some(picked) = self.library.asset(&file, self.files.get(&file))? else {
            self.note(
                id,
                "picture",
                Severity::Unsupported,
                "pictures that are missing or unreadable are not imported",
            );
            return Ok(None);
        };
        // A Paper.js raster is drawn about its own centre, so its origin is the centre of the picture.
        let centre = DAffine2::from_translation(DVec2::new(
            -f64::from(picked.width) / 2.0,
            -f64::from(picked.height) / 2.0,
        ));
        let transform = Transform2D::new(paths::matrix(props)? * centre)
            .map_err(|_| WickError::BadPath("has a transform that is not finite"))?;
        Ok(Some(Content::Bitmap(BitmapInstance::new(
            InstanceId::new(),
            picked.id,
            transform,
        ))))
    }

    /// Keeps `content` as source text attached to `anchor`; a text too large to keep is noted.
    pub fn keep(&mut self, id: &str, anchor: &Anchor, content: String) {
        match Attachment::new("wick", *anchor, Placement::Child, content) {
            Ok(attachment) => self.attachments.push(attachment),
            Err(_) => self.note(
                id,
                "kept size",
                Severity::Unsupported,
                "text too large to keep is not kept",
            ),
        }
    }

    /// Keeps a script's text as source attached to `anchor`. Scripts are not run.
    pub fn scripts(&mut self, id: &str, obj: &Value, anchor: Anchor, place: &str) {
        let mut any = false;
        for script in array(obj, "scripts").map_or(&[][..], Vec::as_slice) {
            let src = text(script, "src").unwrap_or("");
            if src.trim().is_empty() {
                continue;
            }
            let name = text(script, "name").unwrap_or("script");
            let content = format!("{place}: script \"{name}\" (not run)\n{src}");
            let before = self.attachments.len();
            self.keep(id, &anchor, content);
            any |= self.attachments.len() > before;
        }
        if any {
            self.note(
                id,
                "script",
                Severity::Info,
                "scripts are kept as text and not run",
            );
        }
    }
}
