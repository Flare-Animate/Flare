//! Layers, spans and the shapes inside them.

use crate::StorageError;
use flare_core::{
    DocumentPoint, InstanceId, LayerId, Rgba8, ShapeId, SoundId, SymbolId, Transform2D,
};
use flare_document::{
    Cap, Content, FrameSpan, Gradient, Instance, Join, Layer, PathSegment, Shape, SoundCue, Stroke,
    Timeline, Tween,
};
use glamx::DAffine2;
use serde::{Deserialize, Serialize};
use std::num::NonZeroU32;

use super::*;

#[derive(Serialize, Deserialize, Default)]
pub(super) enum TweenWire {
    #[default]
    None,
    Linear,
}

#[derive(Serialize, Deserialize)]
pub(super) enum SegmentWire {
    Move([f64; 2]),
    Line([f64; 2]),
    Cubic([f64; 6]),
    Close,
}

impl SegmentWire {
    pub(super) fn of(segment: &PathSegment) -> Self {
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

    pub(super) fn build(&self) -> Result<PathSegment, StorageError> {
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
pub(super) struct StrokeWire {
    pub(super) width: f64,
    pub(super) rgba: [u8; 4],
    /// "butt", "round" or "square"; absent in older files, which meant round.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) cap: Option<String>,
    /// "round", "bevel" or "miter"; absent in older files, which meant round.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) join: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub(super) struct GradientWire {
    pub(super) from: [f64; 2],
    pub(super) to: [f64; 2],
    pub(super) start: [u8; 4],
    pub(super) end: [u8; 4],
}

#[derive(Serialize, Deserialize)]
pub(super) enum ContentWire {
    Empty,
    Shape {
        id: String,
        path: Vec<SegmentWire>,
        rgba: [u8; 4],
        /// Absent in files written before strokes existed.
        #[serde(default)]
        stroke: Option<StrokeWire>,
        /// Absent in files written before gradients existed.
        #[serde(default)]
        gradient: Option<GradientWire>,
    },
    Instance {
        id: String,
        symbol: String,
        matrix: [f64; 6],
    },
}

#[derive(Serialize, Deserialize)]
pub(super) struct SpanWire {
    #[serde(default)]
    pub(super) tween: TweenWire,
    pub(super) start: u32,
    pub(super) duration: u32,
    pub(super) content: ContentWire,
}

pub(super) fn default_visible() -> bool {
    true
}

#[derive(Serialize, Deserialize)]
pub(super) struct LayerWire {
    pub(super) id: String,
    pub(super) name: String,
    /// Absent in files written before layer flags existed: visible and unlocked.
    #[serde(default = "default_visible")]
    pub(super) visible: bool,
    #[serde(default)]
    pub(super) locked: bool,
    /// Absent in files written before masks existed.
    #[serde(default)]
    pub(super) mask: bool,
    /// Absent in files written before folders existed.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub(super) folder: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) parent: Option<String>,
    /// Absent in files written before motion guides existed.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub(super) guide: bool,
    /// Absent in files written before sounds existed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) cues: Vec<CueWire>,
    pub(super) spans: Vec<SpanWire>,
}

#[derive(Serialize, Deserialize)]
pub(super) struct TimelineWire {
    pub(super) layers: Vec<LayerWire>,
}

impl TimelineWire {
    pub(super) fn of(timeline: &Timeline) -> Self {
        let layers = timeline
            .layers()
            .iter()
            .map(|layer| LayerWire {
                id: layer.id().as_uuid().to_string(),
                name: layer.name().to_owned(),
                visible: layer.is_visible(),
                locked: layer.is_locked(),
                mask: layer.is_mask(),
                folder: layer.is_folder(),
                guide: layer.is_guide(),
                parent: layer.parent().map(|p| p.as_uuid().to_string()),
                cues: layer
                    .cues()
                    .iter()
                    .map(|c| CueWire {
                        start: c.start,
                        sound: c.sound.as_uuid().to_string(),
                    })
                    .collect(),
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
                                    cap: (stroke.cap() != Cap::Round)
                                        .then(|| cap_name(stroke.cap()).to_owned()),
                                    join: (stroke.join() != Join::Round)
                                        .then(|| join_name(stroke.join()).to_owned()),
                                }),
                                gradient: shape.gradient().map(|g| GradientWire {
                                    from: [g.from().as_vec().x, g.from().as_vec().y],
                                    to: [g.to().as_vec().x, g.to().as_vec().y],
                                    start: g.start().to_array(),
                                    end: g.end().to_array(),
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

    pub(super) fn build(self) -> Result<Timeline, StorageError> {
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
                        gradient,
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
                            let cap = cap_from(stroke.cap.as_deref())?;
                            let join = join_from(stroke.join.as_deref())?;
                            let stroke = Stroke::new(stroke.width, Rgba8::new(r, g, b, a))
                                .map_err(corrupt)?
                                .with_cap(cap)
                                .with_join(join);
                            shape = shape.with_stroke(stroke);
                        }
                        if let Some(g) = gradient {
                            let point = |p: [f64; 2]| {
                                DocumentPoint::new(p[0], p[1]).map_err(|_| {
                                    StorageError::Corrupt("non-finite gradient".into())
                                })
                            };
                            let color = |c: [u8; 4]| Rgba8::new(c[0], c[1], c[2], c[3]);
                            let gradient = Gradient::linear(
                                point(g.from)?,
                                point(g.to)?,
                                color(g.start),
                                color(g.end),
                            )
                            .map_err(|error| StorageError::Corrupt(error.to_string()))?;
                            shape = shape.with_gradient(gradient);
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
            let cues = layer
                .cues
                .iter()
                .map(|c| {
                    Ok(SoundCue {
                        start: c.start,
                        sound: SoundId::from_uuid(uuid(&c.sound)?),
                    })
                })
                .collect::<Result<Vec<_>, StorageError>>()?;
            layers.push(
                Layer::new(LayerId::from_uuid(uuid(&layer.id)?), layer.name, spans)?
                    .with_visible(layer.visible)
                    .with_locked(layer.locked)
                    .with_mask(layer.mask)
                    .with_folder(layer.folder)
                    .with_guide(layer.guide)
                    .with_parent(
                        layer
                            .parent
                            .as_deref()
                            .map(uuid)
                            .transpose()?
                            .map(LayerId::from_uuid),
                    )
                    .with_cues(cues)
                    .map_err(|error| StorageError::Corrupt(error.to_string()))?,
            );
        }
        Ok(Timeline::new(layers))
    }
}

fn cap_name(cap: Cap) -> &'static str {
    match cap {
        Cap::Butt => "butt",
        Cap::Round => "round",
        Cap::Square => "square",
    }
}

fn join_name(join: Join) -> &'static str {
    match join {
        Join::Round => "round",
        Join::Bevel => "bevel",
        Join::Miter => "miter",
    }
}

fn cap_from(name: Option<&str>) -> Result<Cap, StorageError> {
    match name {
        None | Some("round") => Ok(Cap::Round),
        Some("butt") => Ok(Cap::Butt),
        Some("square") => Ok(Cap::Square),
        Some(other) => Err(StorageError::Corrupt(format!("unknown line cap {other:?}"))),
    }
}

fn join_from(name: Option<&str>) -> Result<Join, StorageError> {
    match name {
        None | Some("round") => Ok(Join::Round),
        Some("bevel") => Ok(Join::Bevel),
        Some("miter") => Ok(Join::Miter),
        Some(other) => Err(StorageError::Corrupt(format!(
            "unknown line join {other:?}"
        ))),
    }
}
