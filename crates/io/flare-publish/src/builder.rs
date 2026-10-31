//! Turning timelines into SWF display lists, keeping the symbols.
//!
//! The main timeline and every symbol are published the same way: each layer owns a depth, and
//! from one frame to the next only what changed is written. A symbol becomes a `DefineSprite`
//! made once however many instances there are, and an instance a placement of it, so the SWF
//! keeps the structure of the document. A tween is still written as a position for each frame.

use std::collections::{BTreeMap, HashMap, HashSet};

use flare_core::SymbolId;
use flare_document::{Document, PlayMode, Playback, SymbolKind, Timeline};
use flare_eval::{LevelContent, evaluate_level};
use glamx::DAffine2;
use swf::{CharacterId, Matrix, PlaceObjectAction, RemoveObject, Sprite, Tag};

use crate::{Characters, MAX_FRAMES, PublishError, matrix, place};

/// How deep symbols may hold symbols; the evaluator stops at the same kind of limit.
const MAX_SYMBOL_DEPTH: usize = 32;
/// Two depths per layer: the layer's own, and the one under it for the mask over it.
const MAX_LAYERS: usize = 16_000;
/// `DoAction` bytes for ActionScript 1's `stop()` and the end of the list.
const STOP: &[u8] = &[0x07, 0x00];

/// What is shown at one depth.
#[derive(Clone, PartialEq)]
struct Slot {
    character: CharacterId,
    matrix: Matrix,
    /// Makes this a clip layer for the depths above it up to this one.
    clip_depth: Option<u16>,
    /// Tells one span of an instance from the next: a new span starts its symbol again.
    token: u64,
}

/// What a timeline has on screen, kept from frame to frame.
#[derive(Default)]
pub(crate) struct Display {
    on_screen: BTreeMap<u16, Slot>,
}

/// How a sprite plays: the symbol, whether it loops (0), plays once (1) or holds a frame (2),
/// and the frame it starts at. Instances that play alike share one sprite.
type SpriteKey = (SymbolId, u8, u32);

pub(crate) struct Builder<'d> {
    document: &'d Document,
    /// Shapes and sprites, in the order they must be defined: each before what uses it.
    pub(crate) defs: Vec<Tag<'static>>,
    pub(crate) characters: Characters,
    sprites: HashMap<SpriteKey, Option<CharacterId>>,
    depth: usize,
    /// A mask that cannot be written was left out.
    pub(crate) unmasked: bool,
    /// A button was written as a sprite showing its Up state.
    pub(crate) buttons: bool,
}

impl<'d> Builder<'d> {
    pub(crate) fn new(document: &'d Document, characters: Characters) -> Self {
        Self {
            document,
            defs: Vec::new(),
            characters,
            sprites: HashMap::new(),
            depth: 0,
            unmasked: false,
            buttons: false,
        }
    }

    /// The tags that bring `display` to what `timeline` shows at `frame`.
    pub(crate) fn show(
        &mut self,
        display: &mut Display,
        timeline: &Timeline,
        frame: u32,
    ) -> Result<Vec<Tag<'static>>, PublishError> {
        let layers = timeline.layers().len();
        if layers > MAX_LAYERS {
            return Err(PublishError::TooManyLayers(layers));
        }
        let level =
            evaluate_level(self.document, timeline, frame).map_err(|e| PublishError::Evaluate {
                frame,
                message: e.to_string(),
            })?;
        let mut now: BTreeMap<u16, Slot> = BTreeMap::new();
        for item in &level.items {
            // Bottom layers get the low depths; `layers - layer` is at least 1, so even the
            // top layer's mask has a depth to sit at.
            let depth = u16::try_from(2 * (layers - item.layer)).unwrap_or(u16::MAX);
            let (character, matrix, token) = match &item.content {
                LevelContent::Shape { shape, phase } => (
                    self.characters.define(&mut self.defs, shape, *phase)?,
                    matrix(DAffine2::IDENTITY),
                    0,
                ),
                LevelContent::Instance {
                    id,
                    symbol,
                    transform,
                    span_start,
                    playback,
                } => {
                    use std::hash::{Hash, Hasher};
                    let mut hasher = std::collections::hash_map::DefaultHasher::new();
                    (id, span_start).hash(&mut hasher);
                    (
                        self.sprite(*symbol, *playback)?,
                        matrix(*transform),
                        hasher.finish(),
                    )
                }
            };
            let Some(character) = character else {
                continue;
            };
            match item.clip {
                None => {}
                // A clip layer covers the depths above it up to its `clip_depth`, so a mask of
                // one shape goes just under what it masks. Several shapes would need their
                // union, which depths cannot say.
                Some(mask) if level.clips[mask].len() == 1 => {
                    let member = &level.clips[mask][0];
                    // A mask that draws nothing hides everything under it.
                    let Some(clip) =
                        self.characters
                            .define(&mut self.defs, &member.shape, member.phase)?
                    else {
                        continue;
                    };
                    now.insert(
                        depth - 1,
                        Slot {
                            character: clip,
                            matrix: crate::matrix(member.transform),
                            clip_depth: Some(depth),
                            token: 0,
                        },
                    );
                }
                Some(_) => self.unmasked = true,
            }
            now.insert(
                depth,
                Slot {
                    character,
                    matrix,
                    clip_depth: None,
                    token,
                },
            );
        }
        let tags = difference(&display.on_screen, &now);
        display.on_screen = now;
        Ok(tags)
    }

    /// The sprite that plays `symbol` the way `playback` says, made the first time it is
    /// needed. `None` when the symbol draws nothing.
    fn sprite(
        &mut self,
        symbol: SymbolId,
        playback: Playback,
    ) -> Result<Option<CharacterId>, PublishError> {
        let Some(found) = self.document.symbol(symbol) else {
            return Ok(None);
        };
        let length = found.timeline().frame_count();
        if length == 0 {
            return Ok(None);
        }
        let length = u32::try_from(length).map_err(|_| PublishError::TooManyFrames(length))?;
        if length > MAX_FRAMES {
            return Err(PublishError::TooManyFrames(u64::from(length)));
        }
        let first = playback.first_frame.min(length - 1);
        // A Movie Clip loops from its start; a Button shows its first frame, its Up state; a
        // Graphic does what its playback says.
        let (mode, first) = match found.kind() {
            SymbolKind::MovieClip => (0, 0),
            SymbolKind::Button => {
                self.buttons = true;
                (2, 0)
            }
            SymbolKind::Graphic => match playback.mode {
                PlayMode::Loop => (0, first),
                PlayMode::PlayOnce => (1, first),
                PlayMode::SingleFrame => (2, first),
            },
        };
        let key = (symbol, mode, first);
        if let Some(known) = self.sprites.get(&key) {
            return Ok(*known);
        }
        if self.depth >= MAX_SYMBOL_DEPTH {
            return Err(PublishError::SymbolsTooDeep);
        }
        let frames: Vec<u32> = match mode {
            0 => (0..length).map(|i| (first + i) % length).collect(),
            1 => (first..length).collect(),
            _ => vec![first],
        };
        // Playing once ends on a stop, or the sprite would start over.
        let stops = mode == 1 && frames.len() > 1;
        let mut display = Display::default();
        let mut inner: Vec<Tag<'static>> = Vec::new();
        self.depth += 1;
        for (index, frame) in frames.iter().enumerate() {
            let shown = self.show(&mut display, found.timeline(), *frame);
            let Ok(shown) = shown else {
                self.depth -= 1;
                return shown.map(|_| None);
            };
            inner.extend(shown);
            if stops && index + 1 == frames.len() {
                inner.push(Tag::DoAction(STOP));
            }
            inner.push(Tag::ShowFrame);
        }
        self.depth -= 1;
        let drawn = inner.iter().any(|t| matches!(t, Tag::PlaceObject(_)));
        let made = if drawn {
            let id = self.characters.take()?;
            self.defs.push(Tag::DefineSprite(Sprite {
                id,
                num_frames: u16::try_from(frames.len()).unwrap_or(u16::MAX),
                tags: inner,
            }));
            Some(id)
        } else {
            None
        };
        self.sprites.insert(key, made);
        Ok(made)
    }
}

/// The fewest tags that turn what is on screen into `now`.
fn difference(old: &BTreeMap<u16, Slot>, now: &BTreeMap<u16, Slot>) -> Vec<Tag<'static>> {
    let mut tags = Vec::new();
    // Gone, or a new span of the same thing, which has to start again.
    let mut removed = HashSet::new();
    for (depth, before) in old {
        if now
            .get(depth)
            .is_none_or(|after| after.character == before.character && after.token != before.token)
        {
            tags.push(Tag::RemoveObject(RemoveObject {
                depth: *depth,
                character_id: None,
            }));
            removed.insert(*depth);
        }
    }
    for (depth, target) in now {
        let before = old.get(depth).filter(|_| !removed.contains(depth));
        match before {
            Some(before) if before == target => {}
            Some(before)
                if before.character == target.character
                    && before.clip_depth == target.clip_depth =>
            {
                tags.push(place(
                    *depth,
                    PlaceObjectAction::Modify,
                    Some(target.matrix),
                    None,
                ));
            }
            Some(_) => tags.push(place(
                *depth,
                PlaceObjectAction::Replace(target.character),
                Some(target.matrix),
                target.clip_depth,
            )),
            None => tags.push(place(
                *depth,
                PlaceObjectAction::Place(target.character),
                Some(target.matrix),
                target.clip_depth,
            )),
        }
    }
    tags
}
