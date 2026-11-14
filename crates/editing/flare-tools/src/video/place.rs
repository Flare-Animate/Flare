//! The pictures read from a video as one batch: each distinct picture joins the bitmap library
//! once, a Graphic symbol shows them one to a frame (a picture that repeats is held, not stored
//! again), and an instance of the symbol, fitted to the stage and playing once, sits on a new
//! layer at the bottom of the scene for as many frames as there are pictures.

use std::collections::HashMap;
use std::num::NonZeroU32;

use flare_commands::Command;
use flare_core::{BitmapId, InstanceId, LayerId, SceneId, SymbolId, Transform2D};
use flare_document::{
    Bitmap, BitmapInstance, Content, Document, FrameSpan, Instance, Layer, MAX_BITMAPS,
    MAX_TOTAL_BITMAP_BYTES, PlayMode, Playback, Symbol, SymbolKind, Timeline,
};
use glamx::{DAffine2, DVec2};

use super::{Plan, TooLarge, VideoRefusal};
use crate::common::{SpanRef, scene_timeline};
use crate::library::{MAX_SYMBOL_NAME_CHARS, free_symbol_name};

/// What [`place`] made.
#[derive(Debug, Clone, PartialEq)]
pub struct Placed {
    pub commands: Vec<Command>,
    /// The new layer, at the bottom of the scene.
    pub layer: LayerId,
    /// The Graphic symbol holding the pictures.
    pub symbol: SymbolId,
    /// The instance of it, as an element of the layer's one keyframe.
    pub at: SpanRef,
    /// Frames of the timeline the video takes: one per picture read.
    pub frames: u32,
    /// Pictures added to the library: those read, less the ones that repeat.
    pub distinct: usize,
    /// Each picture's size in pixels.
    pub size: (u32, u32),
}

/// The pictures of `plan` read from the video, as a layer called `name` at the bottom of
/// `scene` whose keyframe starts at `frame`. Fewer pictures than the plan said is normal (a
/// movie's length is rounded); each must be the plan's size.
pub fn place(
    document: &Document,
    scene: SceneId,
    frame: u32,
    name: &str,
    plan: &Plan,
    pictures: Vec<Vec<u8>>,
) -> Result<Placed, VideoRefusal> {
    scene_timeline(document, scene).ok_or(VideoRefusal::NoSuchScene)?;
    if pictures.is_empty() {
        return Err(VideoRefusal::NoPictures);
    }
    let (w, h) = plan.size;
    let expected = u64::from(w) * u64::from(h) * 4;
    if let Some(index) = pictures.iter().position(|p| p.len() as u64 != expected) {
        return Err(VideoRefusal::WrongSize { index });
    }
    let frames = u32::try_from(pictures.len()).map_err(|_| too_many(pictures.len(), document))?;
    let name = short_name(name);
    let library = library(document, &name, w, h, pictures)?;
    let symbol = video_symbol(document, &name, &library.order)?;
    let symbol_id = symbol.id();
    let instance = Instance::new(
        InstanceId::new(),
        symbol_id,
        fit_to_stage(document, plan.size)?,
    )
    .with_playback(Playback {
        mode: PlayMode::PlayOnce,
        first_frame: 0,
    });
    let duration = NonZeroU32::new(frames).ok_or(VideoRefusal::NoPictures)?;
    let layer_id = LayerId::new();
    let layer = Layer::new(
        layer_id,
        &*name,
        vec![FrameSpan::new(frame, duration, Content::Instance(instance))],
    )
    .map_err(|e| VideoRefusal::Build(e.to_string()))?
    .with_view_opacity(plan.settings.opacity);
    let distinct = library.added.len();
    let mut commands: Vec<Command> = library.added.into_iter().map(Command::AddBitmap).collect();
    commands.push(Command::AddSymbol(symbol));
    commands.push(Command::AddLayer { scene, layer });
    Ok(Placed {
        commands,
        layer: layer_id,
        symbol: symbol_id,
        at: SpanRef {
            layer: layer_id,
            start: frame,
            element: 0,
        },
        frames,
        distinct,
        size: plan.size,
    })
}

/// A name short enough to number in the library ("clip 2") and never blank.
fn short_name(name: &str) -> String {
    let trimmed = name.trim();
    let name = if trimmed.is_empty() { "Video" } else { trimmed };
    name.chars().take(MAX_SYMBOL_NAME_CHARS - 12).collect()
}

fn too_many(pictures: usize, document: &Document) -> VideoRefusal {
    VideoRefusal::TooLarge(TooLarge::Frames {
        needed: u32::try_from(pictures).unwrap_or(u32::MAX),
        room: u32::try_from(MAX_BITMAPS.saturating_sub(document.bitmaps().len()))
            .unwrap_or(u32::MAX),
    })
}

/// The pictures for the library, and which one each frame shows.
struct Library {
    /// New pictures, in the order they are first shown.
    added: Vec<Bitmap>,
    /// The picture each frame shows, in order.
    order: Vec<BitmapId>,
}

/// Each distinct picture once: one the library already holds is shown rather than added again,
/// and the room the document has left is checked against what is really added.
fn library(
    document: &Document,
    name: &str,
    w: u32,
    h: u32,
    pictures: Vec<Vec<u8>>,
) -> Result<Library, VideoRefusal> {
    let mut known: HashMap<[u8; 32], BitmapId> = document
        .bitmaps()
        .iter()
        .map(|b| (*b.content_hash(), b.id()))
        .collect();
    let (mut added, mut order) = (Vec::new(), Vec::with_capacity(pictures.len()));
    let held: u64 = document.bitmaps().iter().map(|b| b.byte_len()).sum();
    let mut bytes = held;
    for (index, rgba) in pictures.into_iter().enumerate() {
        let bitmap = Bitmap::new(
            BitmapId::new(),
            format!("{name} {:04}", index + 1),
            w,
            h,
            rgba,
        )
        .map_err(|e| VideoRefusal::Build(e.to_string()))?;
        if let Some(id) = known.get(bitmap.content_hash()) {
            order.push(*id);
            continue;
        }
        bytes += bitmap.byte_len();
        if document.bitmaps().len() + added.len() >= MAX_BITMAPS {
            return Err(too_many(added.len() + 1, document));
        }
        if bytes > MAX_TOTAL_BITMAP_BYTES {
            return Err(VideoRefusal::TooLarge(TooLarge::Bytes {
                needed: bytes - held,
                room: MAX_TOTAL_BITMAP_BYTES.saturating_sub(held),
            }));
        }
        known.insert(*bitmap.content_hash(), bitmap.id());
        order.push(bitmap.id());
        added.push(bitmap);
    }
    Ok(Library { added, order })
}

/// A Graphic symbol showing `order` one picture to a frame, a picture shown on frames in a row
/// held as one keyframe.
fn video_symbol(
    document: &Document,
    name: &str,
    order: &[BitmapId],
) -> Result<Symbol, VideoRefusal> {
    let mut spans = Vec::new();
    let mut start = 0_usize;
    while start < order.len() {
        let picture = order[start];
        let run = order[start..]
            .iter()
            .take_while(|id| **id == picture)
            .count();
        let (Ok(at), Some(length)) = (
            u32::try_from(start),
            u32::try_from(run).ok().and_then(NonZeroU32::new),
        ) else {
            return Err(VideoRefusal::Build("too many frames".to_owned()));
        };
        let shown = BitmapInstance::new(InstanceId::new(), picture, Transform2D::IDENTITY);
        spans.push(FrameSpan::new(at, length, Content::Bitmap(shown)));
        start += run;
    }
    let layer =
        Layer::new(LayerId::new(), name, spans).map_err(|e| VideoRefusal::Build(e.to_string()))?;
    let free = free_symbol_name(document, |n| {
        if n == 1 {
            name.to_owned()
        } else {
            format!("{name} {n}")
        }
    });
    Ok(
        Symbol::new(SymbolId::new(), free, Timeline::new(vec![layer]))
            .with_kind(SymbolKind::Graphic),
    )
}

/// The pictures scaled to fit inside the stage, kept in proportion, and centred on it.
fn fit_to_stage(document: &Document, (w, h): (u32, u32)) -> Result<Transform2D, VideoRefusal> {
    let stage = document.stage();
    let (sw, sh) = (f64::from(stage.width()), f64::from(stage.height()));
    let (pw, ph) = (f64::from(w), f64::from(h));
    let scale = (sw / pw).min(sh / ph);
    let matrix = DAffine2::from_scale_angle_translation(
        DVec2::splat(scale),
        0.0,
        DVec2::new((sw - pw * scale) / 2.0, (sh - ph * scale) / 2.0),
    );
    Transform2D::new(matrix).map_err(|e| VideoRefusal::Build(e.to_string()))
}
