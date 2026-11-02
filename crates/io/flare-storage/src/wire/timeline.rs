//! Layers, spans and the shapes inside them.

use crate::StorageError;
use flare_core::{DocumentPoint, LayerId, Rgba8, SoundId};
use flare_document::{
    Cap, FrameSpan, Join, Layer, PathSegment, SoundCue, TextSource, Timeline, Tween,
};
use serde::{Deserialize, Serialize};
use std::num::NonZeroU32;

use super::content::{build_content, wire_content};
use super::*;

#[derive(Serialize, Deserialize, Default)]
pub(super) enum TweenWire {
    #[default]
    None,
    Linear,
    Shape,
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

/// The words of a text object and the font they are set in.
#[derive(Serialize, Deserialize)]
pub(super) struct TextWire {
    text: String,
    family: String,
    size: f32,
    #[serde(default)]
    bold: bool,
    #[serde(default)]
    italic: bool,
    origin: [f64; 2],
}

impl TextWire {
    pub(super) fn of(source: &TextSource) -> Self {
        Self {
            text: source.text().to_owned(),
            family: source.family().to_owned(),
            size: source.size(),
            bold: source.bold(),
            italic: source.italic(),
            origin: source.origin().as_vec().to_array(),
        }
    }

    pub(super) fn build(self) -> Result<TextSource, StorageError> {
        let origin = DocumentPoint::new(self.origin[0], self.origin[1])
            .map_err(|_| StorageError::Corrupt("non-finite text position".into()))?;
        TextSource::new(
            self.text,
            self.family,
            self.size,
            self.bold,
            self.italic,
            origin,
        )
        .map_err(|error| StorageError::Corrupt(error.to_string()))
    }
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
        /// Absent in files written before text could be edited again, and on shapes that are not
        /// text.
        #[serde(default)]
        text: Option<TextWire>,
    },
    Instance {
        id: String,
        symbol: String,
        matrix: [f64; 6],
        /// How a graphic plays: `loop`, `play once` or `single frame`. Absent in files written
        /// before the option existed, which loop from the first frame.
        #[serde(default)]
        play: Option<String>,
        #[serde(default)]
        first_frame: u32,
    },
    /// Absent in files written before cameras existed. The angle is in radians.
    Camera {
        center: [f64; 2],
        zoom: f64,
        rotation: f64,
    },
}

#[derive(Serialize, Deserialize)]
pub(super) struct SpanWire {
    #[serde(default)]
    pub(super) tween: TweenWire,
    /// Easing of the tween; spans saved before it have none.
    #[serde(default)]
    pub(super) ease: i8,
    pub(super) start: u32,
    pub(super) duration: u32,
    pub(super) content: ContentWire,
    /// More elements above `content`; absent in files written before a span could hold several.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) extras: Vec<ContentWire>,
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
    /// Absent in files written before cameras existed.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub(super) camera: bool,
    /// Absent in files written before outline colors existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) outline: Option<[u8; 3]>,
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
                camera: layer.is_camera(),
                outline: layer.outline().map(|c| [c.r, c.g, c.b]),
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
                            Tween::Shape => TweenWire::Shape,
                        },
                        ease: span.ease(),
                        start: span.start(),
                        duration: span.duration().get(),
                        content: wire_content(span.content()),
                        extras: span.extras().iter().map(wire_content).collect(),
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
                let content = build_content(span.content)?;
                let extras = span
                    .extras
                    .into_iter()
                    .map(build_content)
                    .collect::<Result<Vec<_>, _>>()?;
                let tween = match span.tween {
                    TweenWire::None => Tween::None,
                    TweenWire::Linear => Tween::Linear,
                    TweenWire::Shape => Tween::Shape,
                };
                spans.push(
                    FrameSpan::new(span.start, duration, content)
                        .with_extras(extras)?
                        .with_tween(tween)
                        .with_ease(span.ease),
                );
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
                    .with_camera(layer.camera)
                    .with_outline(layer.outline.map(|[r, g, b]| Rgba8::opaque(r, g, b)))
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

pub(super) fn cap_name(cap: Cap) -> &'static str {
    match cap {
        Cap::Butt => "butt",
        Cap::Round => "round",
        Cap::Square => "square",
    }
}

pub(super) fn join_name(join: Join) -> &'static str {
    match join {
        Join::Round => "round",
        Join::Bevel => "bevel",
        Join::Miter => "miter",
    }
}

pub(super) fn cap_from(name: Option<&str>) -> Result<Cap, StorageError> {
    match name {
        None | Some("round") => Ok(Cap::Round),
        Some("butt") => Ok(Cap::Butt),
        Some("square") => Ok(Cap::Square),
        Some(other) => Err(StorageError::Corrupt(format!("unknown line cap {other:?}"))),
    }
}

pub(super) fn join_from(name: Option<&str>) -> Result<Join, StorageError> {
    match name {
        None | Some("round") => Ok(Join::Round),
        Some("bevel") => Ok(Join::Bevel),
        Some("miter") => Ok(Join::Miter),
        Some(other) => Err(StorageError::Corrupt(format!(
            "unknown line join {other:?}"
        ))),
    }
}
