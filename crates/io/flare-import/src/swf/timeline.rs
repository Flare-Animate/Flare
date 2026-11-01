//! Timelines: the display list of a movie or a sprite read into a layer for each depth.

use crate::xfl::{ImportError, Severity};
use flare_core::{InstanceId, LayerId, SymbolId, Transform2D};
use flare_document::{Content, FrameSpan, Instance, Layer, Symbol, SymbolKind, Timeline};
use glamx::DAffine2;
use std::collections::BTreeMap;
use std::num::NonZeroU32;

use super::{MAX_SPANS, Movie};

/// The most frames one timeline may run for. A movie's header holds sixteen bits of them.
const MAX_FRAMES: u32 = 100_000;

/// A matrix as `[a, b, c, d, tx, ty]`, translation in pixels.
type Matrix = [f64; 6];

const IDENTITY: Matrix = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

/// What stands at a depth: a character and where it is put.
#[derive(Clone, Copy, PartialEq)]
struct Put {
    character: swf::CharacterId,
    matrix: Matrix,
}

fn matrix(m: &swf::Matrix) -> Matrix {
    [
        m.a.to_f64(),
        m.b.to_f64(),
        m.c.to_f64(),
        m.d.to_f64(),
        m.tx.to_pixels(),
        m.ty.to_pixels(),
    ]
}

impl Movie {
    /// Reads `tags` as one timeline. Definitions in them are made into symbols as they come.
    pub(super) fn timeline(
        &mut self,
        tags: &[swf::Tag<'_>],
        declared_frames: u32,
        place: &str,
    ) -> Result<Timeline, ImportError> {
        let _ = declared_frames;
        // What stands at each depth now, with the frame it began on.
        let mut current: BTreeMap<swf::Depth, (Put, u32)> = BTreeMap::new();
        // Changes made since the last frame was shown, which take effect on the next one.
        let mut pending: BTreeMap<swf::Depth, Option<Put>> = BTreeMap::new();
        let mut spans: BTreeMap<swf::Depth, Vec<FrameSpan>> = BTreeMap::new();
        let mut frame = 0_u32;
        for tag in tags {
            match tag {
                swf::Tag::DefineShape(shape) => self.define_shape(shape)?,
                swf::Tag::DefineSprite(sprite) => self.define_sprite(sprite)?,
                swf::Tag::PlaceObject(placement) => {
                    self.place(placement, &current, &mut pending, place);
                }
                swf::Tag::RemoveObject(remove) => {
                    pending.insert(remove.depth, None);
                }
                swf::Tag::ShowFrame => {
                    self.commit(&mut current, &mut pending, &mut spans, frame, place)?;
                    frame += 1;
                    if frame > MAX_FRAMES {
                        return Err(ImportError::TooLarge(format!("the timeline of {place}")));
                    }
                }
                other => self.unsupported(other, place),
            }
        }
        // Whatever was changed after the last frame was shown still counts as a frame.
        if !pending.is_empty() {
            self.commit(&mut current, &mut pending, &mut spans, frame, place)?;
            frame += 1;
        }
        let end = frame.max(1);
        for (depth, (put, since)) in std::mem::take(&mut current) {
            self.emit(&mut spans, depth, put, since, end, place)?;
        }
        let mut layers = Vec::new();
        // The highest depth is on top, which is first in a document.
        for (depth, depth_spans) in spans.into_iter().rev() {
            match Layer::new(LayerId::new(), format!("Depth {depth}"), depth_spans) {
                Ok(layer) => layers.push(layer),
                Err(_) => self.note(
                    Severity::Unsupported,
                    place,
                    format!("depth {depth} was skipped: its frames overlap"),
                ),
            }
        }
        Ok(Timeline::new(layers))
    }

    /// Notes a placement as a change to make on the next frame.
    fn place(
        &mut self,
        placement: &swf::PlaceObject<'_>,
        current: &BTreeMap<swf::Depth, (Put, u32)>,
        pending: &mut BTreeMap<swf::Depth, Option<Put>>,
        place: &str,
    ) {
        let depth = placement.depth;
        if placement.clip_depth.is_some() {
            self.note(
                Severity::Unsupported,
                place,
                "masks are not imported; the mask was left out",
            );
            pending.insert(depth, None);
            return;
        }
        if placement.color_transform.is_some() {
            self.note(
                Severity::Unsupported,
                place,
                "color effects are not imported",
            );
        }
        if placement.filters.as_ref().is_some_and(|f| !f.is_empty()) {
            self.note(Severity::Unsupported, place, "filters are not imported");
        }
        if placement
            .blend_mode
            .is_some_and(|b| b != swf::BlendMode::Normal)
        {
            self.note(Severity::Unsupported, place, "blend modes are not imported");
        }
        let there = pending
            .get(&depth)
            .copied()
            .unwrap_or_else(|| current.get(&depth).map(|(put, _)| *put));
        let moved = placement.matrix.as_ref().map(matrix);
        let put = match placement.action {
            swf::PlaceObjectAction::Place(id) | swf::PlaceObjectAction::Replace(id) => {
                let kept = if matches!(placement.action, swf::PlaceObjectAction::Replace(_)) {
                    there.map(|t| t.matrix)
                } else {
                    None
                };
                if !self.defined.contains_key(&id) {
                    self.note(
                        Severity::Unsupported,
                        place,
                        "an object that is not imported (text, a button, a picture or video) was left out",
                    );
                    pending.insert(depth, None);
                    return;
                }
                Put {
                    character: id,
                    matrix: moved.or(kept).unwrap_or(IDENTITY),
                }
            }
            swf::PlaceObjectAction::Modify => {
                let Some(there) = there else { return };
                Put {
                    character: there.character,
                    matrix: moved.unwrap_or(there.matrix),
                }
            }
            swf::PlaceObjectAction::None => return,
        };
        pending.insert(depth, Some(put));
    }

    /// Makes the changes of the frame about to be shown, closing the span of each thing that
    /// changed or went.
    fn commit(
        &mut self,
        current: &mut BTreeMap<swf::Depth, (Put, u32)>,
        pending: &mut BTreeMap<swf::Depth, Option<Put>>,
        spans: &mut BTreeMap<swf::Depth, Vec<FrameSpan>>,
        frame: u32,
        place: &str,
    ) -> Result<(), ImportError> {
        for (depth, change) in std::mem::take(pending) {
            let old = current.remove(&depth);
            if let Some((put, since)) = old {
                if change == Some(put) {
                    current.insert(depth, (put, since));
                    continue;
                }
                self.emit(spans, depth, put, since, frame, place)?;
            }
            if let Some(put) = change {
                current.insert(depth, (put, frame));
            }
        }
        Ok(())
    }

    /// Adds the span from frame `since` up to (not including) `until` for `put` at `depth`.
    fn emit(
        &mut self,
        spans: &mut BTreeMap<swf::Depth, Vec<FrameSpan>>,
        depth: swf::Depth,
        put: Put,
        since: u32,
        until: u32,
        place: &str,
    ) -> Result<(), ImportError> {
        let Some(duration) = NonZeroU32::new(until.saturating_sub(since)) else {
            return Ok(());
        };
        self.spans += 1;
        if self.spans > MAX_SPANS {
            return Err(ImportError::TooLarge("the movie's timelines".into()));
        }
        let Some(&symbol) = self.defined.get(&put.character) else {
            return Ok(());
        };
        let transform = Transform2D::new(DAffine2::from_cols_array(&put.matrix));
        let Ok(transform) = transform else {
            self.note(
                Severity::Unsupported,
                place,
                "an object with an invalid matrix was left out",
            );
            return Ok(());
        };
        let content = Content::Instance(Instance::new(InstanceId::new(), symbol, transform));
        spans
            .entry(depth)
            .or_default()
            .push(FrameSpan::new(since, duration, content));
        Ok(())
    }

    fn define_shape(&mut self, shape: &swf::Shape) -> Result<(), ImportError> {
        self.symbol_count_ok()?;
        let place = format!("shape {}", shape.id);
        let contents = self.shape_contents(shape, &place);
        // A span holds one shape, so a character that draws several has a layer for each.
        let layers: Vec<Layer> = contents
            .into_iter()
            .enumerate()
            .filter_map(|(i, content)| {
                let span = FrameSpan::new(0, NonZeroU32::MIN, content);
                Layer::new(LayerId::new(), format!("Shape {}", i + 1), vec![span]).ok()
            })
            .rev()
            .collect();
        let name = self
            .names
            .get(&shape.id)
            .cloned()
            .unwrap_or_else(|| format!("Shape {}", shape.id));
        self.add_symbol(shape.id, name, Timeline::new(layers), SymbolKind::Graphic);
        Ok(())
    }

    fn define_sprite(&mut self, sprite: &swf::Sprite<'_>) -> Result<(), ImportError> {
        self.symbol_count_ok()?;
        let place = format!("sprite {}", sprite.id);
        let timeline = self.timeline(&sprite.tags, u32::from(sprite.num_frames), &place)?;
        let name = self
            .names
            .get(&sprite.id)
            .cloned()
            .unwrap_or_else(|| format!("Sprite {}", sprite.id));
        self.add_symbol(sprite.id, name, timeline, SymbolKind::MovieClip);
        Ok(())
    }

    fn add_symbol(
        &mut self,
        character: swf::CharacterId,
        name: String,
        timeline: Timeline,
        kind: SymbolKind,
    ) {
        let id = SymbolId::new();
        self.defined.insert(character, id);
        self.symbols
            .push(Symbol::new(id, name, timeline).with_kind(kind));
    }

    /// Says what the movie has that this import does not carry over.
    fn unsupported(&mut self, tag: &swf::Tag<'_>, place: &str) {
        let what = match tag {
            swf::Tag::DefineText(_) | swf::Tag::DefineText2(_) | swf::Tag::DefineEditText(_) => {
                "text"
            }
            swf::Tag::DefineButton(_) | swf::Tag::DefineButton2(_) => "buttons",
            swf::Tag::DefineMorphShape(_) => "shape morphs",
            swf::Tag::DefineBits { .. }
            | swf::Tag::DefineBitsJpeg2 { .. }
            | swf::Tag::DefineBitsJpeg3(_)
            | swf::Tag::DefineBitsLossless(_) => "pictures",
            swf::Tag::DefineVideoStream(_) | swf::Tag::VideoFrame(_) => "video",
            swf::Tag::DefineSound(_)
            | swf::Tag::StartSound(_)
            | swf::Tag::StartSound2 { .. }
            | swf::Tag::SoundStreamHead(_)
            | swf::Tag::SoundStreamHead2(_)
            | swf::Tag::SoundStreamBlock(_) => "sound",
            swf::Tag::DoAction(_)
            | swf::Tag::DoAbc(_)
            | swf::Tag::DoAbc2(_)
            | swf::Tag::DoInitAction { .. } => "scripts",
            swf::Tag::FrameLabel(_) | swf::Tag::DefineSceneAndFrameLabelData(_) => {
                "frame labels and scenes"
            }
            _ => return,
        };
        self.note(
            Severity::Unsupported,
            place,
            format!("{what} are not imported"),
        );
    }
}
