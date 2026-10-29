//! Private on-disk payloads. Rebuilding goes through the validating constructors.

use crate::StorageError;
use flare_core::{DocumentId, FrameRate, Rgba8, SceneId, SymbolId};
use flare_document::{Attachment, Document, Scene, Sound, Stage, Symbol};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

mod attachment;
mod settings;
mod sound;
mod timeline;

pub(crate) use attachment::AttachmentWire;
#[allow(unused_imports)]
use attachment::*;
pub(crate) use settings::Settings;
#[allow(unused_imports)]
use settings::*;
pub(crate) use sound::SoundWire;
#[allow(unused_imports)]
use sound::*;
#[allow(unused_imports)]
use timeline::*;

fn uuid(text: &str) -> Result<Uuid, StorageError> {
    Uuid::parse_str(text).map_err(|_| StorageError::Corrupt(format!("bad id {text}")))
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
    attachments: Vec<Attachment>,
    sounds: Vec<Sound>,
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
    let document = document
        .with_attachments(attachments)
        .map_err(StorageError::Invalid)?
        .with_sounds(sounds)
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
