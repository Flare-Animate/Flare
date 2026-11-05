//! The document: one layer per FlipaClip layer (top of the stack first), one bitmap per drawn
//! frame cropped to what it draws, and a background layer at the bottom when there is one.
//! Identical pictures share one bitmap, and every bound in `limits` is checked as it is reached.

use std::collections::HashMap;
use std::num::NonZeroU32;

use flare_core::{
    BitmapId, DocumentId, FrameRate, InstanceId, LayerId, Rgba8, SceneId, Transform2D,
};
use flare_document::{
    Bitmap, BitmapInstance, Content, Document, FrameSpan, Layer, MAX_BITMAPS, Scene, Stage,
    Timeline,
};
use glamx::{DAffine2, DVec2};

use super::archive::{Archive, Picture};
use super::error::FlipaError;
use super::frame::{self, Drawn};
use super::limits::{MAX_DECODED_BYTES, MAX_STORED_BYTES};
use super::meta::Meta;
use crate::xfl::{Diagnostic, ImportError, Severity};

/// Bitmaps kept so far, shared by content hash, and the bytes they hold.
struct Library {
    bitmaps: Vec<Bitmap>,
    by_hash: HashMap<[u8; 32], BitmapId>,
    stored: u64,
}

impl Library {
    fn add(&mut self, name: String, drawn: Drawn) -> Result<BitmapId, ImportError> {
        let bytes = drawn.rgba.len() as u64;
        let bitmap = Bitmap::new(BitmapId::new(), name, drawn.width, drawn.height, drawn.rgba)
            .map_err(|e| invalid_picture(&e.to_string()))?;
        if let Some(id) = self.by_hash.get(bitmap.content_hash()) {
            return Ok(*id);
        }
        if self.bitmaps.len() >= MAX_BITMAPS {
            return Err(too_large("the project's bitmaps"));
        }
        self.stored += bytes;
        if self.stored > MAX_STORED_BYTES {
            return Err(too_large("the pictures kept"));
        }
        let id = bitmap.id();
        self.by_hash.insert(*bitmap.content_hash(), id);
        self.bitmaps.push(bitmap);
        Ok(id)
    }
}

fn picture_error(e: FlipaError) -> ImportError {
    ImportError::Foreign(e.to_string())
}

fn too_large(what: &'static str) -> ImportError {
    ImportError::Foreign(FlipaError::TooLarge(what).to_string())
}

fn invalid_picture(why: &str) -> ImportError {
    ImportError::Foreign(format!("a picture: {why}"))
}

fn placed(id: BitmapId, x: u32, y: u32) -> Result<Content, ImportError> {
    let offset = DAffine2::from_translation(DVec2::new(f64::from(x), f64::from(y)));
    let transform = Transform2D::new(offset)
        .map_err(|_| ImportError::Foreign("a picture is placed off the canvas".into()))?;
    Ok(Content::Bitmap(BitmapInstance::new(
        InstanceId::new(),
        id,
        transform,
    )))
}

/// The layer's frames as spans; a frame that draws nothing leaves a gap.
fn layer_spans(
    name: &str,
    pictures: &[&Picture],
    meta: &Meta,
    budget: &mut u64,
    library: &mut Library,
) -> Result<Vec<FrameSpan>, ImportError> {
    let mut spans = Vec::new();
    for picture in pictures {
        let entry = &picture.name;
        let drawn = frame::decode(entry, &picture.data, (meta.width, meta.height), budget)
            .map_err(picture_error)?;
        let Some(drawn) = drawn else { continue };
        let (x, y) = (drawn.x, drawn.y);
        let id = library.add(format!("{name} {}", picture.frame + 1), drawn)?;
        let one = NonZeroU32::MIN;
        spans.push(FrameSpan::new(picture.frame, one, placed(id, x, y)?));
    }
    Ok(spans)
}

pub(super) fn build(
    meta: &Meta,
    archive: Archive,
) -> Result<(Document, Vec<Diagnostic>), ImportError> {
    let rate = FrameRate::new(meta.fps, 1)
        .map_err(|_| ImportError::Foreign(FlipaError::BadField("fps").to_string()))?;
    let mut keyed: Vec<&Picture> = archive.pictures.iter().collect();
    keyed.sort_by_key(|p| (p.layer, p.frame));
    for pair in keyed.windows(2) {
        if (pair[0].layer, pair[0].frame) == (pair[1].layer, pair[1].frame) {
            return Err(ImportError::Foreign(
                FlipaError::BadPicture {
                    entry: pair[1].name.clone(),
                    reason: "the same frame of this layer is listed twice",
                }
                .to_string(),
            ));
        }
    }
    for picture in &keyed {
        if picture.layer as usize >= meta.layers.len() {
            return Err(ImportError::Foreign(
                FlipaError::BadPicture {
                    entry: picture.name.clone(),
                    reason: "the layer is not in meta",
                }
                .to_string(),
            ));
        }
        if picture.frame >= meta.frame_count {
            return Err(ImportError::Foreign(
                FlipaError::BadPicture {
                    entry: picture.name.clone(),
                    reason: "the frame is past frameCount",
                }
                .to_string(),
            ));
        }
    }
    let mut budget = MAX_DECODED_BYTES;
    let mut library = Library {
        bitmaps: Vec::new(),
        by_hash: HashMap::new(),
        stored: 0,
    };
    let mut layers = Vec::new();
    // FlipaClip lists its layers bottom first; the timeline lists them top first.
    for index in (0..meta.layers.len()).rev() {
        let name = &meta.layers[index];
        let index = u32::try_from(index).unwrap_or(u32::MAX);
        let pictures: Vec<&Picture> = keyed.iter().copied().filter(|p| p.layer == index).collect();
        let spans = layer_spans(name, &pictures, meta, &mut budget, &mut library)?;
        layers.push(
            Layer::new(LayerId::new(), name.as_str(), spans)
                .map_err(|e| invalid_picture(&e.to_string()))?,
        );
    }
    if let Some(bytes) = &archive.background {
        let drawn = frame::decode("bg", bytes, (meta.width, meta.height), &mut budget)
            .map_err(picture_error)?;
        if let Some(drawn) = drawn {
            let (x, y) = (drawn.x, drawn.y);
            let id = library.add("Background".to_owned(), drawn)?;
            let hold = NonZeroU32::new(meta.frame_count).unwrap_or(NonZeroU32::MIN);
            let spans = vec![FrameSpan::new(0, hold, placed(id, x, y)?)];
            layers.push(
                Layer::new(LayerId::new(), "Background", spans)
                    .map_err(|e| invalid_picture(&e.to_string()))?,
            );
        }
    }
    let scene = Scene::new(SceneId::new(), meta.name.as_str(), Timeline::new(layers));
    let stage = Stage::new(meta.width, meta.height, Rgba8::WHITE)
        .map_err(|e| invalid_picture(&e.to_string()))?;
    let document = Document::new(DocumentId::new(), rate, vec![scene], vec![])
        .map_err(ImportError::Invalid)?
        .with_stage(stage)
        .with_bitmaps(library.bitmaps)
        .map_err(ImportError::Invalid)?;
    let mut notes = vec![Diagnostic {
        severity: Severity::Info,
        location: meta.name.clone(),
        message: format!(
            "FlipaClip project (format 1{}): {}x{} at {} fps, {} frames, {} layers; the layers are \
             stacked in FlipaClip's order, assumed bottom first",
            meta.version
                .map_or_else(String::new, |v| format!(", app version {v}")),
            meta.width,
            meta.height,
            meta.fps,
            meta.frame_count,
            meta.layers.len(),
        ),
    }];
    notes.push(Diagnostic {
        severity: Severity::Warning,
        location: meta.name.clone(),
        message: "layer settings in meta (blend modes, glow, alpha lock, visibility) are not \
                  imported; every layer is drawn normal and visible"
            .into(),
    });
    if !archive.unsupported.is_empty() {
        notes.push(Diagnostic {
            severity: Severity::Unsupported,
            location: meta.name.clone(),
            message: format!(
                "{} entries are not imported (sound, fonts and anything else not drawn): {}",
                archive.unsupported.len(),
                archive
                    .unsupported
                    .iter()
                    .take(5)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", "),
            ),
        });
    }
    Ok((document, notes))
}
