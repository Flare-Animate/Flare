//! A Toonz scene as a Flare document, built as one validated command batch. The frame rate and
//! camera resolution become the document's; each level column becomes a layer whose spans show
//! the level's drawings; each drawing of a vector level becomes a Graphic symbol (strokes
//! grouped by palette style, drawn as cubic paths with the mean thickness as line width).
//! Raster levels (`.tlv`) and sound columns are named as unsupported. The caller supplies the
//! level files, since where a scene's `+drawings` folder lies is up to the surface.

use std::collections::HashMap;
use std::num::NonZeroU32;

use flare_commands::{Command, apply_all};
use flare_core::{
    DocumentId, DocumentPoint, FrameRate, InstanceId, LayerId, Rgba8, SceneId, ShapeId, SymbolId,
    Transform2D,
};
use flare_document::{
    Content, Document, FrameSpan, Instance, Layer, MAX_STAGE_SIDE, PathSegment, Scene as DocScene,
    Shape, Stage, Stroke as LineStroke, Symbol, Timeline,
};

use crate::error::ToonzError;
use crate::pli::{self, VectorLevel};
use crate::scene::{CellRun, Column, LevelDecl, Scene};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Info,
    Warning,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    pub severity: Severity,
    pub location: String,
    pub message: String,
}

#[derive(Debug)]
pub struct Converted {
    pub document: Document,
    pub notes: Vec<Note>,
}

const MAX_SPANS: usize = 500_000;
const MAX_SYMBOLS: usize = 50_000;
const MAX_NOTES: usize = 1_000;
/// One stage inch is this many level units in Toonz.
const UNITS_PER_INCH: f64 = 53.333_333;
const DEFAULT_INCHES: f64 = 16.0;
const DEFAULT_RES: (u32, u32) = (1920, 1080);

struct Ctx<'a> {
    commands: Vec<Command>,
    notes: Vec<Note>,
    scene: SceneId,
    px_per_unit: f64,
    centre: [f64; 2],
    levels: HashMap<u32, Option<VectorLevel>>,
    symbols: HashMap<(u32, u32, String), SymbolId>,
    read: &'a dyn Fn(&LevelDecl) -> Option<Vec<u8>>,
    spans: usize,
}

impl Ctx<'_> {
    fn note(&mut self, severity: Severity, location: &str, message: impl Into<String>) {
        if self.notes.len() < MAX_NOTES {
            self.notes.push(Note {
                severity,
                location: location.to_owned(),
                message: message.into(),
            });
        }
    }
}

/// `read` returns the bytes of a level's file, or `None` when they are not available.
pub fn to_flare_document(
    scene: &Scene,
    read: &dyn Fn(&LevelDecl) -> Option<Vec<u8>>,
) -> Result<Converted, ToonzError> {
    let doc = |e: &dyn std::fmt::Display| ToonzError::Document(e.to_string());
    let (w, h) = scene
        .camera_res
        .filter(|(w, h)| *w <= MAX_STAGE_SIDE && *h <= MAX_STAGE_SIDE)
        .unwrap_or(DEFAULT_RES);
    let stage = Stage::new(w, h, Rgba8::WHITE).map_err(|e| doc(&e))?;
    let fps = scene.frame_rate.unwrap_or(24.0);
    let rate = FrameRate::new((fps * 1000.0).round() as u32, 1000).map_err(|e| doc(&e))?;
    let inches = scene.camera_size.map_or(DEFAULT_INCHES, |s| s.0);
    let mut cx = Ctx {
        commands: vec![Command::SetStage(stage), Command::SetFrameRate(rate)],
        notes: Vec::new(),
        scene: SceneId::new(),
        px_per_unit: f64::from(w) / inches / UNITS_PER_INCH,
        centre: [f64::from(w) / 2.0, f64::from(h) / 2.0],
        levels: HashMap::new(),
        symbols: HashMap::new(),
        read,
        spans: 0,
    };
    if scene.camera_res.is_none() {
        cx.note(
            Severity::Warning,
            "scene",
            "no camera resolution; the stage is 1920x1080",
        );
    }
    // Columns run bottom to top; a Flare timeline lists layers top to bottom.
    for (index, column) in scene.columns.iter().enumerate().rev() {
        let name = format!("Column {}", index + 1);
        match column {
            Column::Level(runs) => level_column(&mut cx, scene, &name, runs)?,
            Column::Palette(_) => cx.note(
                Severity::Info,
                &name,
                "palette column: palettes are read from the levels",
            ),
            Column::Sound(cells) => {
                let path = scene
                    .levels
                    .iter()
                    .find(|l| l.id == cells.level)
                    .and_then(|l| l.path.clone())
                    .unwrap_or_default();
                cx.note(
                    Severity::Unsupported,
                    &name,
                    format!("sound column ({path}): not embedded"),
                );
            }
            Column::Other { tag } => cx.note(
                Severity::Unsupported,
                &name,
                format!("{tag} columns are not imported"),
            ),
        }
    }
    let empty = DocScene::new(cx.scene, "Scene 1", Timeline::new(Vec::new()));
    let base = Document::new(DocumentId::new(), rate, vec![empty], Vec::new())
        .map_err(|d| doc(&format!("{d:?}")))?;
    let document = apply_all(&base, cx.commands)
        .map_err(|e| doc(&e))?
        .unwrap_or(base);
    Ok(Converted {
        document,
        notes: cx.notes,
    })
}

fn level_column(
    cx: &mut Ctx<'_>,
    scene: &Scene,
    name: &str,
    runs: &[CellRun],
) -> Result<(), ToonzError> {
    let mut spans = Vec::new();
    for run in runs {
        let Some(decl) = scene.levels.iter().find(|l| l.id == run.level) else {
            continue;
        };
        let steps = if run.increment == 0 { 1 } else { run.count };
        for k in 0..steps {
            cx.spans += 1;
            if cx.spans > MAX_SPANS {
                return Err(ToonzError::Limit {
                    what: "converted cells",
                    max: MAX_SPANS,
                });
            }
            let number = i64::from(run.frame.number) + i64::from(run.increment) * i64::from(k);
            let letter = run.frame.letter.map(String::from).unwrap_or_default();
            let number = u32::try_from(number).unwrap_or(0);
            let Some(symbol) = symbol_for(cx, decl, number, &letter)? else {
                continue;
            };
            let length = if run.increment == 0 { run.count } else { 1 };
            let (Some(duration), start) = (NonZeroU32::new(length), run.row + k) else {
                continue;
            };
            let instance = Instance::new(InstanceId::new(), symbol, Transform2D::IDENTITY);
            spans.push(FrameSpan::new(start, duration, Content::Instance(instance)));
        }
    }
    if let Ok(layer) = Layer::new(LayerId::new(), name, spans) {
        cx.commands.push(Command::AddLayer {
            scene: cx.scene,
            layer,
        });
    }
    Ok(())
}

fn symbol_for(
    cx: &mut Ctx<'_>,
    decl: &LevelDecl,
    number: u32,
    letter: &str,
) -> Result<Option<SymbolId>, ToonzError> {
    let key = (decl.id, number, letter.to_owned());
    if let Some(id) = cx.symbols.get(&key) {
        return Ok(Some(*id));
    }
    if cx.symbols.len() >= MAX_SYMBOLS {
        return Err(ToonzError::Limit {
            what: "converted drawings",
            max: MAX_SYMBOLS,
        });
    }
    if let std::collections::hash_map::Entry::Vacant(slot) = cx.levels.entry(decl.id) {
        let loaded = load(cx.read, decl);
        let location = decl.name.clone();
        let outcome = match loaded {
            Ok(level) => Some(level),
            Err(message) => {
                slot.insert(None);
                cx.note(Severity::Unsupported, &location, message);
                None
            }
        };
        if let Some(level) = outcome {
            cx.levels.insert(decl.id, Some(level));
        }
    }
    let (shapes, warning) = match cx.levels.get(&decl.id) {
        Some(Some(level)) => drawing(cx, level, number, letter),
        _ => (Vec::new(), None),
    };
    if let Some(message) = warning {
        cx.note(
            Severity::Warning,
            &format!("{} {number}", decl.name),
            message,
        );
    }
    let symbol = SymbolId::new();
    let layers = shapes
        .into_iter()
        .enumerate()
        .filter_map(|(i, shape)| {
            let span = FrameSpan::new(0, NonZeroU32::MIN, Content::Shape(shape));
            Layer::new(LayerId::new(), format!("Style {}", i + 1), vec![span]).ok()
        })
        .collect();
    cx.commands.push(Command::AddSymbol(Symbol::new(
        symbol,
        format!("{} {number}{letter}", decl.name),
        Timeline::new(layers),
    )));
    cx.symbols.insert(key, symbol);
    Ok(Some(symbol))
}

fn load(
    read: &dyn Fn(&LevelDecl) -> Option<Vec<u8>>,
    decl: &LevelDecl,
) -> Result<VectorLevel, String> {
    match decl.kind.as_str() {
        "vector" => {
            let bytes = read(decl)
                .ok_or_else(|| "the level file was not found beside the scene".to_owned())?;
            pli::decode(&bytes).map_err(|e| e.to_string())
        }
        other => Err(format!(
            "{other} levels cannot be imported as drawings yet; the cells are left empty"
        )),
    }
}

fn drawing(
    cx: &Ctx<'_>,
    level: &VectorLevel,
    number: u32,
    letter: &str,
) -> (Vec<Shape>, Option<String>) {
    let Some(frame) = level
        .frames
        .iter()
        .find(|f| f.number == number && f.suffix == letter)
    else {
        return (Vec::new(), None);
    };
    let warning = (frame.regions > 0).then(|| {
        format!(
            "{} filled regions are not decoded; only strokes are drawn",
            frame.regions
        )
    });
    let mut by_style: Vec<(u32, Vec<&pli::Stroke>)> = Vec::new();
    for stroke in &frame.strokes {
        match by_style.iter_mut().find(|(s, _)| *s == stroke.style) {
            Some((_, list)) => list.push(stroke),
            None => by_style.push((stroke.style, vec![stroke])),
        }
    }
    let colour = |id: u32| -> Rgba8 {
        level
            .styles
            .iter()
            .find(|(i, _)| *i == id)
            .and_then(|(_, s)| s.color)
            .or_else(|| {
                usize::try_from(id)
                    .ok()
                    .and_then(|i| level.legacy_colors.get(i).copied())
            })
            .map_or(Rgba8::new(0, 0, 0, 255), |[r, g, b, a]| {
                Rgba8::new(r, g, b, a)
            })
    };
    let shapes = by_style
        .into_iter()
        .filter_map(|(style, strokes)| shape(cx, &strokes, colour(style)))
        .collect();
    (shapes, warning)
}

fn shape(cx: &Ctx<'_>, strokes: &[&pli::Stroke], colour: Rgba8) -> Option<Shape> {
    let point = |p: &pli::ThickPoint| {
        DocumentPoint::new(
            cx.centre[0] + p.x * cx.px_per_unit,
            cx.centre[1] - p.y * cx.px_per_unit,
        )
        .ok()
    };
    let (mut path, mut thick_sum, mut thick_n) = (Vec::new(), 0.0, 0usize);
    for stroke in strokes {
        let Some(first) = stroke.chunks.first() else {
            continue;
        };
        path.push(PathSegment::MoveTo(point(&first.0[0])?));
        for chunk in &stroke.chunks {
            let [a, c, b] = &chunk.0;
            // A quadratic as a cubic: each handle two thirds of the way to the control point.
            let lift = |from: &pli::ThickPoint| {
                let (x, y) = (
                    from.x + 2.0 / 3.0 * (c.x - from.x),
                    from.y + 2.0 / 3.0 * (c.y - from.y),
                );
                point(&pli::ThickPoint { x, y, thick: 0.0 })
            };
            path.push(PathSegment::CubicTo(lift(a)?, lift(b)?, point(b)?));
            thick_sum += a.thick + b.thick;
            thick_n += 2;
        }
        if stroke.looped {
            path.push(PathSegment::Close);
        }
    }
    if path.is_empty() {
        return None;
    }
    let width = (2.0 * thick_sum / thick_n.max(1) as f64 * cx.px_per_unit).clamp(1.0, 500.0);
    let line = LineStroke::new(width, colour).ok()?;
    Shape::new(ShapeId::new(), path, Rgba8::TRANSPARENT)
        .ok()
        .map(|s| s.with_stroke(line))
}
