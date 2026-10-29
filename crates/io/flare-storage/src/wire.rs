//! Private on-disk payloads. Rebuilding goes through the validating constructors.

use std::num::NonZeroU32;

use flare_core::{DocumentId, FrameRate, InstanceId, LayerId, SceneId, SymbolId, Transform2D};
use flare_document::{Content, Document, FrameSpan, Instance, Layer, Scene, Symbol, Timeline};
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
}

impl Settings {
    pub(crate) fn of(document: &Document) -> Self {
        let rate = document.frame_rate();
        Self {
            frame_rate: (rate.numerator(), rate.denominator()),
        }
    }
}

#[derive(Serialize, Deserialize)]
enum ContentWire {
    Empty,
    Instance {
        id: String,
        symbol: String,
        matrix: [f64; 6],
    },
}

#[derive(Serialize, Deserialize)]
struct SpanWire {
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
                        start: span.start(),
                        duration: span.duration().get(),
                        content: match span.content() {
                            Content::Empty => ContentWire::Empty,
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
                spans.push(FrameSpan::new(span.start, duration, content));
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
    Document::new(
        DocumentId::from_uuid(uuid(project_id)?),
        rate,
        scenes,
        symbols,
    )
    .map_err(StorageError::Invalid)
}
