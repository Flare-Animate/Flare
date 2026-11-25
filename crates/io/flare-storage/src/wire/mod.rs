//! Private on-disk payloads. Rebuilding goes through the validating constructors.

use crate::StorageError;
use flare_core::{DocumentId, FrameRate, Rgba8, SceneId, ShaderGraph, ShaderNode, SymbolId};
use flare_document::{Attachment, Document, Scene, ShaderAsset, Sound, Stage, Symbol, SymbolKind};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

mod armature;
mod attachment;
mod bitmap;
mod content;
mod effects;
pub(crate) mod game;
mod origin;
mod settings;
mod sound;
mod timeline;

pub(crate) use attachment::AttachmentWire;
#[allow(unused_imports)]
use attachment::*;
use attachment::{hex, unhex};
pub(crate) use bitmap::{BitmapMeta, MAX_BITMAP_RECORD_BYTES};
pub(crate) use game::MAX_GAME_RECORD_BYTES;
pub(crate) use origin::{MAX_ORIGIN_RECORD_BYTES, OriginMeta};
pub(crate) use settings::Settings;
#[allow(unused_imports)]
use settings::*;
pub(crate) use sound::SoundMeta;
#[allow(unused_imports)]
use sound::*;
#[allow(unused_imports)]
use timeline::*;

pub(crate) fn uuid(text: &str) -> Result<Uuid, StorageError> {
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
    /// Files from before symbol kinds name none, and mean a graphic.
    #[serde(default)]
    kind: String,
    timeline: TimelineWire,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct StopMotionWire {
    fps: u32,
    default_hold: u32,
    frames: Vec<(flare_core::BitmapId, u32)>,
}

impl StopMotionWire {
    pub(crate) fn of(session: &flare_document::StopMotion) -> Self {
        Self {
            fps: session.fps(),
            default_hold: session.default_hold(),
            frames: session
                .frames()
                .iter()
                .map(|f| (f.bitmap(), f.hold()))
                .collect(),
        }
    }

    pub(crate) fn build(self) -> Result<flare_document::StopMotion, StorageError> {
        let corrupt = |error: flare_document::StopMotionError| {
            StorageError::Corrupt(format!("invalid stop-motion session: {error}"))
        };
        let frames = self
            .frames
            .into_iter()
            .map(|(bitmap, hold)| flare_document::StopFrame::new(bitmap, hold))
            .collect::<Result<Vec<_>, _>>()
            .map_err(corrupt)?;
        flare_document::StopMotion::new(self.fps, self.default_hold)
            .and_then(|session| session.with_frames(frames))
            .map_err(corrupt)
    }
}

#[derive(Serialize, Deserialize)]
pub(crate) struct RigWire {
    pub(crate) symbol: SymbolId,
    pub(crate) payload: String,
}

impl RigWire {
    pub(crate) fn of(rig: &flare_document::RigAsset) -> Self {
        Self {
            symbol: rig.symbol(),
            payload: rig.payload().to_owned(),
        }
    }

    /// A rig that does not read back as a valid rig for its symbol is a corrupt file.
    pub(crate) fn build(self) -> Result<flare_document::RigAsset, StorageError> {
        let rig: flare_rig::Rig = serde_json::from_str(&self.payload)
            .map_err(|error| StorageError::Corrupt(format!("invalid rig: {error}")))?;
        rig.validate()
            .map_err(|error| StorageError::Corrupt(format!("invalid rig: {error}")))?;
        if rig.symbol != self.symbol {
            return Err(StorageError::Corrupt("a rig is for another symbol".into()));
        }
        flare_document::RigAsset::new(self.symbol, self.payload)
            .map_err(|error| StorageError::Corrupt(format!("invalid rig: {error}")))
    }
}

#[derive(Serialize, Deserialize)]
pub(crate) struct ShaderGraphWire {
    pub(crate) name: String,
    pub(crate) nodes: Vec<ShaderNode>,
    pub(crate) output: usize,
}

impl ShaderGraphWire {
    pub(crate) fn of(asset: &ShaderAsset) -> Self {
        Self {
            name: asset.name().to_owned(),
            nodes: asset.graph().nodes().to_vec(),
            output: asset.graph().output(),
        }
    }

    pub(crate) fn build(self) -> Result<ShaderAsset, StorageError> {
        let graph = ShaderGraph::new(self.nodes, self.output)
            .map_err(|error| StorageError::Corrupt(format!("invalid shader graph: {error}")))?;
        ShaderAsset::new(self.name, graph)
            .map_err(|error| StorageError::Corrupt(format!("invalid shader asset: {error}")))
    }
}

impl SymbolWire {
    pub(crate) fn of(symbol: &Symbol) -> Self {
        Self {
            name: symbol.name().to_owned(),
            kind: symbol.kind().name().to_owned(),
            timeline: TimelineWire::of(symbol.timeline()),
        }
    }

    pub(crate) fn build(self, id: &str) -> Result<Symbol, StorageError> {
        // A name that is not a kind is a damaged file, not something to guess at.
        let kind = if self.kind.is_empty() {
            SymbolKind::Graphic
        } else {
            SymbolKind::from_name(&self.kind).ok_or_else(|| {
                StorageError::Corrupt(format!("unknown symbol kind {:?}", self.kind))
            })?
        };
        Ok(Symbol::new(
            SymbolId::from_uuid(uuid(id)?),
            self.name,
            self.timeline.build()?,
        )
        .with_kind(kind))
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn assemble(
    project_id: &str,
    settings: &str,
    scenes: Vec<Scene>,
    symbols: Vec<Symbol>,
    attachments: Vec<Attachment>,
    sounds: Vec<Sound>,
    bitmaps: Vec<flare_document::Bitmap>,
    origin: Option<flare_document::Origin>,
    game: Option<flare_game::GameSpec>,
    shader_assets: Vec<ShaderAsset>,
    rig_assets: Vec<flare_document::RigAsset>,
    stop_motion: Option<flare_document::StopMotion>,
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
        .map_err(StorageError::Invalid)?
        .with_bitmaps(bitmaps)
        .map_err(StorageError::Invalid)?
        .with_game(game)
        .map_err(StorageError::Invalid)?
        .with_shader_assets(shader_assets)
        .map_err(StorageError::Invalid)?
        .with_rig_assets(rig_assets)
        .map_err(StorageError::Invalid)?
        .with_stop_motion(stop_motion)
        .map_err(StorageError::Invalid)?;
    let document = document.with_origin(origin.map(std::sync::Arc::new));
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
