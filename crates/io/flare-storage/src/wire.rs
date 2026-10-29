//! Private on-disk payloads. Rebuilding goes through the validating constructors.

use std::num::NonZeroU32;

use flare_core::{
    DocumentId, DocumentPoint, FrameRate, InstanceId, LayerId, Rgba8, SceneId, ShapeId, SymbolId,
    Transform2D,
};
use flare_document::{
    Content, Document, FrameSpan, Instance, Layer, PathSegment, Scene, Shape, Stage, Stroke,
    Symbol, Timeline, Tween,
};
use glamx::DAffine2;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::StorageError;

fn uuid(text: &str) -> Result<Uuid, StorageError> {
    Uuid::parse_str(text).map_err(|_| StorageError::Corrupt(format!("bad id {text}")))
}

#[derive(Serialize, Deserialize)]
pub(crate) struct Settings {
    frame_rate: (u32, u32),
    /// Absent in files written before stages existed; those load with the default stage.
    #[serde(default)]
    stage: Option<StageWire>,
}

#[derive(Serialize, Deserialize)]
struct StageWire {
    width: u32,
    height: u32,
    background: [u8; 4],
}

impl Settings {
    pub(crate) fn of(document: &Document) -> Self {
        let rate = document.frame_rate();
        let stage = document.stage();
        Self {
            frame_rate: (rate.numerator(), rate.denominator()),
            stage: Some(StageWire {
                width: stage.width(),
                height: stage.height(),
                background: stage.background().to_array(),
            }),
        }
    }
}

#[derive(Serialize, Deserialize, Default)]
enum TweenWire {
    #[default]
    None,
    Linear,
}

#[derive(Serialize, Deserialize)]
enum SegmentWire {
    Move([f64; 2]),
    Line([f64; 2]),
    Cubic([f64; 6]),
    Close,
}

impl SegmentWire {
    fn of(segment: &PathSegment) -> Self {
        let xy = |p: &DocumentPoint| p.as_vec().to_array();
        match segment {
            PathSegment::MoveTo(p) => Self::Move(xy(p)),
            PathSegment::LineTo(p) => Self::Line(xy(p)),
            PathSegment::CubicTo(a, b, c) => {
                let (a, b, c) = (xy(a), xy(b), xy(c));
                Self::Cubic([a[0], a[1], b[0], b[1], c[0], c[1]])
            }
            PathSegment::Close => Self::Close,
        }
    }

    fn build(&self) -> Result<PathSegment, StorageError> {
        let point = |x: f64, y: f64| {
            DocumentPoint::new(x, y)
                .map_err(|_| StorageError::Corrupt("non-finite shape coordinate".into()))
        };
        Ok(match *self {
            Self::Move([x, y]) => PathSegment::MoveTo(point(x, y)?),
            Self::Line([x, y]) => PathSegment::LineTo(point(x, y)?),
            Self::Cubic([a, b, c, d, e, f]) => {
                PathSegment::CubicTo(point(a, b)?, point(c, d)?, point(e, f)?)
            }
            Self::Close => PathSegment::Close,
        })
    }
}

#[derive(Serialize, Deserialize)]
struct StrokeWire {
    width: f64,
    rgba: [u8; 4],
}

#[derive(Serialize, Deserialize)]
enum ContentWire {
    Empty,
    Shape {
        id: String,
        path: Vec<SegmentWire>,
        rgba: [u8; 4],
        /// Absent in files written before strokes existed.
        #[serde(default)]
        stroke: Option<StrokeWire>,
    },
    Instance {
        id: String,
        symbol: String,
        matrix: [f64; 6],
    },
}

#[derive(Serialize, Deserialize)]
struct SpanWire {
    #[serde(default)]
    tween: TweenWire,
    start: u32,
    duration: u32,
    content: ContentWire,
}

#[derive(Serialize, Deserialize)]
struct LayerWire {
    id: String,
    name: String,
    spans: Vec<SpanWire>,
}

#[derive(Serialize, Deserialize)]
struct TimelineWire {
    layers: Vec<LayerWire>,
}

impl TimelineWire {
    fn of(timeline: &Timeline) -> Self {
        let layers = timeline
            .layers()
            .iter()
            .map(|layer| LayerWire {
                id: layer.id().as_uuid().to_string(),
                name: layer.name().to_owned(),
                spans: layer
                    .spans()
                    .iter()
                    .map(|span| SpanWire {
                        tween: match span.tween() {
                            Tween::None => TweenWire::None,
                            Tween::Linear => TweenWire::Linear,
                        },
                        start: span.start(),
                        duration: span.duration().get(),
                        content: match span.content() {
                            Content::Empty => ContentWire::Empty,
                            Content::Shape(shape) => ContentWire::Shape {
                                id: shape.id().as_uuid().to_string(),
                                path: shape.path().iter().map(SegmentWire::of).collect(),
                                rgba: shape.fill().to_array(),
                                stroke: shape.stroke().map(|stroke| StrokeWire {
                                    width: stroke.width(),
                                    rgba: stroke.color().to_array(),
                                }),
                            },
                            Content::Instance(instance) => ContentWire::Instance {
                                id: instance.id().as_uuid().to_string(),
                                symbol: instance.symbol().as_uuid().to_string(),
                                matrix: instance.transform().as_affine().to_cols_array(),
                            },
                        },
                    })
                    .collect(),
            })
            .collect();
        Self { layers }
    }

    fn build(self) -> Result<Timeline, StorageError> {
        let mut layers = Vec::with_capacity(self.layers.len());
        for layer in self.layers {
            let mut spans = Vec::with_capacity(layer.spans.len());
            for span in layer.spans {
                let duration = NonZeroU32::new(span.duration)
                    .ok_or_else(|| StorageError::Corrupt("zero-length span".into()))?;
                let content = match span.content {
                    ContentWire::Empty => Content::Empty,
                    ContentWire::Shape {
                        id,
                        path,
                        rgba,
                        stroke,
                    } => {
                        let path = path
                            .iter()
                            .map(SegmentWire::build)
                            .collect::<Result<_, _>>()?;
                        let fill = Rgba8::new(rgba[0], rgba[1], rgba[2], rgba[3]);
                        let corrupt = |error: flare_document::ShapeError| {
                            StorageError::Corrupt(error.to_string())
                        };
                        let mut shape = Shape::new(ShapeId::from_uuid(uuid(&id)?), path, fill)
                            .map_err(corrupt)?;
                        if let Some(stroke) = stroke {
                            let [r, g, b, a] = stroke.rgba;
                            let stroke = Stroke::new(stroke.width, Rgba8::new(r, g, b, a))
                                .map_err(corrupt)?;
                            shape = shape.with_stroke(stroke);
                        }
                        Content::Shape(shape)
                    }
                    ContentWire::Instance { id, symbol, matrix } => {
                        let transform = Transform2D::new(DAffine2::from_cols_array(&matrix))
                            .map_err(|_| StorageError::Corrupt("non-finite transform".into()))?;
                        Content::Instance(Instance::new(
                            InstanceId::from_uuid(uuid(&id)?),
                            SymbolId::from_uuid(uuid(&symbol)?),
                            transform,
                        ))
                    }
                };
                let tween = match span.tween {
                    TweenWire::None => Tween::None,
                    TweenWire::Linear => Tween::Linear,
                };
                spans.push(FrameSpan::new(span.start, duration, content).with_tween(tween));
            }
            layers.push(Layer::new(
                LayerId::from_uuid(uuid(&layer.id)?),
                layer.name,
                spans,
            )?);
        }
        Ok(Timeline::new(layers))
    }
}

#[derive(Serialize, Deserialize)]
pub(crate) struct SceneWire {
    name: String,
    timeline: TimelineWire,
}

impl SceneWire {
    pub(crate) fn of(scene: &Scene) -> Self {
        Self {
            name: scene.name().to_owned(),
            timeline: TimelineWire::of(scene.timeline()),
        }
    }

    pub(crate) fn build(self, id: &str) -> Result<Scene, StorageError> {
        Ok(Scene::new(
            SceneId::from_uuid(uuid(id)?),
            self.name,
            self.timeline.build()?,
        ))
    }
}

#[derive(Serialize, Deserialize)]
pub(crate) struct SymbolWire {
    name: String,
    timeline: TimelineWire,
}

impl SymbolWire {
    pub(crate) fn of(symbol: &Symbol) -> Self {
        Self {
            name: symbol.name().to_owned(),
            timeline: TimelineWire::of(symbol.timeline()),
        }
    }

    pub(crate) fn build(self, id: &str) -> Result<Symbol, StorageError> {
        Ok(Symbol::new(
            SymbolId::from_uuid(uuid(id)?),
            self.name,
            self.timeline.build()?,
        ))
    }
}

pub(crate) fn assemble(
    project_id: &str,
    settings: &str,
    scenes: Vec<Scene>,
    symbols: Vec<Symbol>,
) -> Result<Document, StorageError> {
    let settings: Settings = serde_json::from_str(settings)?;
    let rate = FrameRate::new(settings.frame_rate.0, settings.frame_rate.1)
        .map_err(|error| StorageError::Corrupt(error.to_string()))?;
    let document = Document::new(
        DocumentId::from_uuid(uuid(project_id)?),
        rate,
        scenes,
        symbols,
    )
    .map_err(StorageError::Invalid)?;
    match settings.stage {
        None => Ok(document),
        Some(wire) => {
            let [r, g, b, a] = wire.background;
            let stage = Stage::new(wire.width, wire.height, Rgba8::new(r, g, b, a))
                .map_err(|error| StorageError::Corrupt(error.to_string()))?;
            Ok(document.with_stage(stage))
        }
    }
}
