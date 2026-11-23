//! File > Import > Import to Stage and Import to Library, as Animate has them for a Flash movie:
//! another document (a `.swf`, or anything else Flare imports) is brought into this one instead
//! of being opened on its own. Its library comes with it, its pictures and sounds too, and its
//! main timeline becomes a Graphic symbol named after the file, so it plays frame for frame with
//! the timeline it is placed on. Import to Stage also puts that symbol on a new layer at the top,
//! from the playhead on; its origin is the stage's, so the artwork sits where it did in the movie.
//!
//! Everything is one batch of commands: one undo step, and nothing changes if it is refused.

use std::collections::{HashMap, HashSet};
use std::num::NonZeroU32;

use flare_commands::Command;
use flare_core::{BitmapId, InstanceId, LayerId, SceneId, SymbolId, Transform2D};
use flare_document::{
    BitmapInstance, Content, Document, FrameSpan, Instance, Layer, SoundCue, Symbol, SymbolKind,
    Timeline,
};
use thiserror::Error;

use crate::layers_to_symbol::MAX_NAME_CHARS;

/// Where an import goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Destination {
    /// Into the library only.
    Library,
    /// Into the library, and on a new layer of `scene` from `frame` on.
    Stage { scene: SceneId, frame: u32 },
}

/// Why a document could not be brought in.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ImportRefusal {
    #[error("the file has no frames to import")]
    NoFrames,
    #[error("there is no such scene")]
    NoSuchScene,
    #[error("a layer of bones cannot go into a symbol yet")]
    Armature,
    #[error("the file's main timeline has more sounds than one layer can hold")]
    TooManySounds,
    #[error(
        "this document already has the file's {0}; import the file again to bring in a fresh copy"
    )]
    AlreadyHere(&'static str),
    #[error("the frame is past the longest timeline Flare keeps")]
    FrameTooFar,
    #[error("the file could not be brought in: {0}")]
    Invalid(String),
}

/// Something the import left out or changed, for the person to be told.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportNote {
    /// Scenes after the first were not brought in (how many).
    ScenesLeftOut(usize),
    /// The file's camera layer belongs to its scene and was left out.
    CameraLeftOut,
    /// The file ran at another rate (frames a second, rounded); its frames play at this one's.
    FrameRate(String),
    /// Sounds on the file's main timeline were not placed, as the library has no timeline.
    SoundsNotPlaced(usize),
    /// Shader graphs the file carried were not brought in.
    ShadersLeftOut(usize),
    /// A symbol was renamed, as the library already had its name: from, to.
    Renamed(String, String),
}

impl std::fmt::Display for ImportNote {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ScenesLeftOut(n) => write!(f, "{n} more scene(s) in the file were not imported"),
            Self::CameraLeftOut => write!(f, "the file's camera layer was not imported"),
            Self::FrameRate(rate) => write!(
                f,
                "the file ran at {rate} frames a second; it plays at this document's rate"
            ),
            Self::SoundsNotPlaced(n) => write!(
                f,
                "{n} sound(s) on the file's main timeline are in the library but not placed"
            ),
            Self::ShadersLeftOut(n) => {
                write!(f, "{n} shader graph(s) in the file were not imported")
            }
            Self::Renamed(from, to) => write!(f, "symbol \"{from}\" was imported as \"{to}\""),
        }
    }
}

/// What an import does: the commands for one undo step, the symbol holding the file's main
/// timeline, the layer it was placed on (Import to Stage) and what to tell the person.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportedInto {
    pub commands: Vec<Command>,
    pub symbol: SymbolId,
    pub layer: Option<LayerId>,
    pub notes: Vec<ImportNote>,
}

/// Brings `source` into `target` as `name` (the file's name without its extension), at
/// `destination`.
pub fn import_into(
    target: &Document,
    source: &Document,
    name: &str,
    destination: Destination,
) -> Result<ImportedInto, ImportRefusal> {
    let mut notes = Vec::new();
    let scene = source.scenes().first().ok_or(ImportRefusal::NoFrames)?;
    if source.scenes().len() > 1 {
        notes.push(ImportNote::ScenesLeftOut(source.scenes().len() - 1));
    }
    if source.frame_rate() != target.frame_rate() {
        notes.push(ImportNote::FrameRate(format!(
            "{:.2}",
            source.frame_rate().per_second()
        )));
    }
    if !source.shader_assets().is_empty() {
        notes.push(ImportNote::ShadersLeftOut(source.shader_assets().len()));
    }
    let place = match destination {
        Destination::Library => None,
        Destination::Stage { scene, frame } => {
            if !target.scenes().iter().any(|s| s.id() == scene) {
                return Err(ImportRefusal::NoSuchScene);
            }
            Some((scene, frame))
        }
    };

    // A picture this document already has is used as it is: the library holds one of each.
    let mut commands = Vec::new();
    let mut same_picture: HashMap<BitmapId, BitmapId> = HashMap::new();
    for bitmap in source.bitmaps() {
        if let Some(here) = target
            .bitmaps()
            .iter()
            .find(|b| b.content_hash() == bitmap.content_hash())
        {
            same_picture.insert(bitmap.id(), here.id());
        } else if target.bitmaps().iter().any(|b| b.id() == bitmap.id()) {
            return Err(ImportRefusal::AlreadyHere("pictures"));
        } else {
            commands.push(Command::AddBitmap(bitmap.clone()));
        }
    }
    for sound in source.sounds() {
        if target.sounds().iter().any(|s| s.id() == sound.id()) {
            return Err(ImportRefusal::AlreadyHere("sounds"));
        }
        commands.push(Command::AddSound(sound.clone()));
    }

    let mut taken: HashSet<String> = target
        .symbols()
        .iter()
        .map(|s| s.name().to_owned())
        .collect();
    for symbol in source.symbols() {
        if target.symbols().iter().any(|s| s.id() == symbol.id()) {
            return Err(ImportRefusal::AlreadyHere("symbols"));
        }
        let free = free_in(&taken, symbol.name());
        if free != symbol.name() {
            notes.push(ImportNote::Renamed(symbol.name().to_owned(), free.clone()));
        }
        taken.insert(free.clone());
        let timeline = Timeline::new(rebind_layers(symbol.timeline().layers(), &same_picture)?);
        commands.push(Command::AddSymbol(
            symbol.clone().with_name(free).with_timeline(timeline),
        ));
    }

    // The main timeline, without its camera and with its sounds kept back for the stage.
    let mut cues: Vec<SoundCue> = Vec::new();
    let mut inside = Vec::new();
    let mut camera = false;
    for layer in scene.timeline().layers() {
        if layer.is_camera() {
            camera = true;
            continue;
        }
        cues.extend(layer.cues().iter().cloned());
        inside.push(
            layer
                .clone()
                .with_cues(Vec::new())
                .map_err(|e| ImportRefusal::Invalid(e.to_string()))?,
        );
    }
    if camera {
        notes.push(ImportNote::CameraLeftOut);
        // A layer whose folder was the camera's would point at nothing.
        let kept: HashSet<LayerId> = inside.iter().map(Layer::id).collect();
        inside = inside
            .into_iter()
            .map(|l| {
                let parent = l.parent().filter(|p| kept.contains(p));
                l.with_parent(parent)
            })
            .collect();
    }
    let inside = rebind_layers(&inside, &same_picture)?;
    let frames = inside
        .iter()
        .flat_map(Layer::spans)
        .map(FrameSpan::end)
        .max()
        .and_then(|end| u32::try_from(end).ok())
        .and_then(NonZeroU32::new)
        .ok_or(ImportRefusal::NoFrames)?;
    let symbol = SymbolId::new();
    let main_name = free_in(&taken, name);
    commands.push(Command::AddSymbol(
        Symbol::new(symbol, main_name.clone(), Timeline::new(inside))
            .with_kind(SymbolKind::Graphic),
    ));

    let Some((scene, frame)) = place else {
        if !cues.is_empty() {
            notes.push(ImportNote::SoundsNotPlaced(cues.len()));
        }
        return Ok(ImportedInto {
            commands,
            symbol,
            layer: None,
            notes,
        });
    };
    frame
        .checked_add(frames.get())
        .ok_or(ImportRefusal::FrameTooFar)?;
    let instance = Instance::new(InstanceId::new(), symbol, Transform2D::IDENTITY);
    let mut spans = Vec::new();
    if let Some(before) = NonZeroU32::new(frame) {
        spans.push(FrameSpan::new(0, before, Content::Empty));
    }
    spans.push(FrameSpan::new(frame, frames, Content::Instance(instance)));
    let cues = cues
        .into_iter()
        .map(|c| {
            c.start
                .checked_add(frame)
                .map(|start| SoundCue {
                    start,
                    sound: c.sound,
                })
                .ok_or(ImportRefusal::FrameTooFar)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let layer = LayerId::new();
    let placed = Layer::new(layer, main_name, spans)
        .map_err(|e| ImportRefusal::Invalid(e.to_string()))?
        .with_cues(cues)
        .map_err(|_| ImportRefusal::TooManySounds)?;
    commands.push(Command::AddLayer {
        scene,
        layer: placed,
    });
    commands.push(Command::MoveLayer {
        scene,
        layer,
        index: 0,
    });
    Ok(ImportedInto {
        commands,
        symbol,
        layer: Some(layer),
        notes,
    })
}

/// `layers` with each picture `same` names swapped for the copy this document already has.
fn rebind_layers(
    layers: &[Layer],
    same: &HashMap<BitmapId, BitmapId>,
) -> Result<Vec<Layer>, ImportRefusal> {
    layers
        .iter()
        .map(|layer| {
            if layer
                .spans()
                .iter()
                .flat_map(FrameSpan::elements)
                .any(|e| matches!(e, Content::Armature(_)))
            {
                return Err(ImportRefusal::Armature);
            }
            if same.is_empty() {
                return Ok(layer.clone());
            }
            let spans = layer
                .spans()
                .iter()
                .map(|span| rebind_span(span, same))
                .collect();
            layer
                .with_spans(spans)
                .map_err(|e| ImportRefusal::Invalid(e.to_string()))
        })
        .collect()
}

fn rebind_span(span: &FrameSpan, same: &HashMap<BitmapId, BitmapId>) -> FrameSpan {
    let mut span = span.clone();
    for index in 0..span.element_count() {
        if let Some(Content::Bitmap(picture)) = span.element(index)
            && let Some(here) = same.get(&picture.bitmap())
        {
            let swapped = BitmapInstance::new(picture.id(), *here, picture.transform())
                .with_opacity(picture.opacity());
            if let Some(next) = span.with_element(index, Content::Bitmap(swapped)) {
                span = next;
            }
        }
    }
    span
}

/// `base` (trimmed and cut to a name's length) if `taken` lacks it, else `base 2`, `base 3` and
/// on.
fn free_in(taken: &HashSet<String>, base: &str) -> String {
    let base: String = base.trim().chars().take(MAX_NAME_CHARS - 8).collect();
    let base = if base.is_empty() {
        "Imported".to_owned()
    } else {
        base
    };
    if !taken.contains(&base) {
        return base;
    }
    (2..=taken.len() + 2)
        .map(|n| format!("{base} {n}"))
        .find(|candidate| !taken.contains(candidate))
        .unwrap_or(base)
}

#[cfg(test)]
#[path = "import_into_tests.rs"]
mod tests;
