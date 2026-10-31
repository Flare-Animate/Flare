//! Publishes a Flare document as a SWF file.
//!
//! A SWF has one timeline of characters and placements, whereas a Flare document has symbols,
//! tweens and scenes. Publishing therefore *bakes* each frame with the same evaluator the
//! editor uses: tweens become per-frame matrices, symbol instances are flattened into their
//! shapes, and scenes are played one after another. The result plays the same pictures but
//! has no symbols or runtime behaviour. It is encoded by the `swf` crate from the Ruffle
//! project and checked in tests by parsing it back with that crate's reader; it has not been
//! played in Flash Player or Ruffle.
#![forbid(unsafe_code)]

mod file;
mod html5;
mod shapes;
mod sound;

pub use file::{PublishFileError, publish_html5_to_file, publish_to_file};
pub use html5::{Html5Options, MAX_PAGE_BYTES, PublishedHtml, publish_html5};

use std::collections::HashMap;

use flare_core::ShapeId;
use flare_document::Document;
use swf::{
    Color, Compression, Fixed8, Fixed16, Header, Matrix, PlaceObject, PlaceObjectAction, Rectangle,
    RemoveObject, Tag, Twips,
};
use thiserror::Error;

/// SWF 10: the version where `DefineShape4` and the display list used here are plain.
const SWF_VERSION: u8 = 10;
/// The most frames a SWF header can declare.
const MAX_FRAMES: u32 = u16::MAX as u32;
const MAX_STAGE_PX: u32 = 8191; // SWF rectangles are 32-bit twips; keep well inside.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Info,
    /// Published with a change in meaning.
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
}

/// SWF character ids are 16 bits and 0 is reserved; the top id is kept free as well.
const MAX_SHAPES: u32 = 65_534;

#[derive(Debug, Error)]
pub enum PublishError {
    #[error("the document has {0} frames; a SWF holds at most {MAX_FRAMES}")]
    TooManyFrames(u64),
    #[error("the stage is larger than {MAX_STAGE_PX} px, which a SWF cannot hold")]
    StageTooLarge,
    #[error("the document has more distinct shapes than a SWF can number (at most {MAX_SHAPES})")]
    TooManyShapes,
    #[error("evaluating frame {frame}: {message}")]
    Evaluate { frame: u32, message: String },
    #[error("writing the SWF: {0}")]
    Write(String),
    #[error("the page would be larger than {0} bytes")]
    PageTooLarge(usize),
}

#[derive(Debug, Clone)]
pub struct PublishOptions {
    /// Compress the file with zlib.
    pub compress: bool,
}

impl Default for PublishOptions {
    fn default() -> Self {
        Self { compress: true }
    }
}

#[derive(Debug)]
pub struct Published {
    pub bytes: Vec<u8>,
    pub frames: u32,
    pub diagnostics: Vec<Diagnostic>,
}

/// Pixels to twips, rounded (the library's own conversion truncates toward zero).
fn twips(pixels: f64) -> Twips {
    Twips::new(
        (pixels * f64::from(Twips::TWIPS_PER_PIXEL))
            .round()
            .clamp(-1.0e9, 1.0e9) as i32,
    )
}

fn matrix(m: glamx::DAffine2) -> Matrix {
    let [a, b, c, d, tx, ty] = m.to_cols_array();
    Matrix {
        a: Fixed16::from_f64(a),
        b: Fixed16::from_f64(b),
        c: Fixed16::from_f64(c),
        d: Fixed16::from_f64(d),
        tx: twips(tx),
        ty: twips(ty),
    }
}

/// What is on screen at one depth.
#[derive(Clone, PartialEq)]
struct Placed {
    character: swf::CharacterId,
    matrix: Matrix,
    /// Makes this a clip layer for the depths above it up to this one.
    clip_depth: Option<u16>,
}

/// The shapes defined so far: each distinct drawing is defined once, the first time it appears.
struct Characters {
    known: HashMap<(ShapeId, u64), Option<swf::CharacterId>>,
    next: swf::CharacterId,
    approximated: bool,
}

impl Characters {
    /// The character that draws `shape`, or `None` when it draws nothing. A step of a shape
    /// tween has its shape's identity, so `phase` tells the steps apart.
    fn define<'a>(
        &mut self,
        tags: &mut Vec<Tag<'a>>,
        shape: &flare_document::Shape,
        phase: u64,
    ) -> Result<Option<swf::CharacterId>, PublishError> {
        let id = (shape.id(), phase);
        if let Some(known) = self.known.get(&id) {
            return Ok(*known);
        }
        // Character ids are 16 bits; running out must not wrap or repeat.
        if self.next == swf::CharacterId::MAX {
            return Err(PublishError::TooManyShapes);
        }
        let defined = self.next;
        let made = shapes::define_shape(defined, shape, &mut self.approximated);
        let assigned = made.is_some().then_some(defined);
        if let Some(shape) = made {
            self.next = self.next.saturating_add(1);
            tags.push(Tag::DefineShape(Box::new(shape)));
        }
        self.known.insert(id, assigned);
        Ok(assigned)
    }
}

fn place(
    depth: u16,
    action: PlaceObjectAction,
    matrix: Option<Matrix>,
    clip_depth: Option<u16>,
) -> Tag<'static> {
    Tag::PlaceObject(Box::new(PlaceObject {
        version: 2,
        action,
        depth,
        matrix,
        color_transform: None,
        ratio: None,
        name: None,
        clip_depth,
        class_name: None,
        filters: None,
        background_color: None,
        blend_mode: None,
        clip_actions: None,
        has_image: false,
        is_bitmap_cached: None,
        is_visible: None,
        amf_data: None,
    }))
}

/// Publishes `document`, scene after scene, as a SWF.
pub fn publish_swf(
    document: &Document,
    options: &PublishOptions,
) -> Result<Published, PublishError> {
    let mut diagnostics = Vec::new();
    let stage = document.stage();
    if stage.width() > MAX_STAGE_PX || stage.height() > MAX_STAGE_PX {
        return Err(PublishError::StageTooLarge);
    }
    let total: u64 = document
        .scenes()
        .iter()
        .map(|s| s.timeline().frame_count().max(1))
        .sum();
    let total = u32::try_from(total)
        .ok()
        .filter(|t| *t <= MAX_FRAMES)
        .ok_or(PublishError::TooManyFrames(total))?;
    if document.scenes().len() > 1 {
        diagnostics.push(Diagnostic {
            severity: Severity::Warning,
            message: "a SWF has one timeline, so the scenes were played one after another".into(),
        });
    }
    diagnostics.push(Diagnostic {
        severity: Severity::Info,
        message: "frames were baked: tweens became per-frame positions and symbols were flattened"
            .into(),
    });
    if stage.background().a != 255 {
        diagnostics.push(Diagnostic {
            severity: Severity::Warning,
            message: "a SWF background has no transparency; it was written opaque".into(),
        });
    }

    let mut tags: Vec<Tag<'_>> = vec![
        Tag::FileAttributes(swf::FileAttributes::empty()),
        Tag::SetBackgroundColor(Color {
            r: stage.background().r,
            g: stage.background().g,
            b: stage.background().b,
            a: 255,
        }),
    ];

    // Sounds are defined first, so shapes take the ids after them.
    let sound::Sounds {
        define: defined_sounds,
        starts: mut sound_starts,
        diagnostics: mut sound_diagnostics,
        next_character,
    } = sound::define(document, 1, sound::MAX_SAMPLES);
    diagnostics.append(&mut sound_diagnostics);
    tags.extend(defined_sounds.iter().map(sound::Defined::tag));
    let mut characters = Characters {
        known: HashMap::new(),
        next: next_character,
        approximated: false,
    };
    let mut unmasked = false;
    let mut on_screen: Vec<Placed> = Vec::new();
    let mut frames = 0_u32;

    for scene in document.scenes() {
        let length = u32::try_from(scene.timeline().frame_count().max(1)).unwrap_or(u32::MAX);
        for frame in 0..length {
            let evaluated =
                flare_eval::evaluate_frame(document, scene.id(), frame).map_err(|e| {
                    PublishError::Evaluate {
                        frame: frames,
                        message: e.to_string(),
                    }
                })?;

            let mut now: Vec<Placed> = Vec::new();
            let drawn = &evaluated.shapes;
            let mut start = 0;
            while start < drawn.len() {
                // A run of shapes under the same masks shares them.
                let masks = &drawn[start].clips;
                let end = start
                    + drawn[start..]
                        .iter()
                        .take_while(|d| d.clips == *masks)
                        .count();
                let mut run: Vec<Placed> = Vec::new();
                for shape in &drawn[start..end] {
                    if let Some(character) =
                        characters.define(&mut tags, &shape.shape, shape.phase)?
                    {
                        run.push(Placed {
                            character,
                            matrix: matrix(shape.transform),
                            clip_depth: None,
                        });
                    }
                }
                match masks.as_slice() {
                    [] => now.append(&mut run),
                    // A clip layer covers the depths above it up to its `clip_depth`, so a mask
                    // of one shape goes just under its run. Several shapes or several masks
                    // would need their union or intersection, which depths cannot say.
                    [mask] if evaluated.clips[*mask].len() == 1 => {
                        let member = &evaluated.clips[*mask][0];
                        // A mask that draws nothing hides everything under it.
                        if let Some(character) =
                            characters.define(&mut tags, &member.shape, member.phase)?
                            && !run.is_empty()
                        {
                            let last = now.len() + run.len() + 1;
                            now.push(Placed {
                                character,
                                matrix: matrix(member.transform),
                                clip_depth: Some(u16::try_from(last).unwrap_or(u16::MAX)),
                            });
                            now.append(&mut run);
                        }
                    }
                    _ => {
                        unmasked = true;
                        now.append(&mut run);
                    }
                }
                start = end;
            }

            // Bring the display list from `on_screen` to `now` with the fewest tags.
            for (index, target) in now.iter().enumerate() {
                let depth = u16::try_from(index + 1).unwrap_or(u16::MAX);
                match on_screen.get(index) {
                    Some(old) if old == target => {}
                    Some(old)
                        if old.character == target.character
                            && old.clip_depth == target.clip_depth =>
                    {
                        tags.push(place(
                            depth,
                            PlaceObjectAction::Modify,
                            Some(target.matrix),
                            None,
                        ));
                    }
                    Some(_) => tags.push(place(
                        depth,
                        PlaceObjectAction::Replace(target.character),
                        Some(target.matrix),
                        target.clip_depth,
                    )),
                    None => tags.push(place(
                        depth,
                        PlaceObjectAction::Place(target.character),
                        Some(target.matrix),
                        target.clip_depth,
                    )),
                }
            }
            for index in now.len()..on_screen.len() {
                let depth = u16::try_from(index + 1).unwrap_or(u16::MAX);
                tags.push(Tag::RemoveObject(RemoveObject {
                    depth,
                    character_id: None,
                }));
            }
            on_screen = now;
            for start in sound_starts.remove(&frames).unwrap_or_default() {
                tags.push(Tag::StartSound(start));
            }
            tags.push(Tag::ShowFrame);
            frames += 1;
        }
    }
    if unmasked {
        diagnostics.push(Diagnostic {
            severity: Severity::Warning,
            message: "SWF publishing writes masks of one shape; masks of several shapes, and masks inside masks, were left out, so those layers appear unmasked".into(),
        });
    }
    if characters.approximated {
        diagnostics.push(Diagnostic {
            severity: Severity::Info,
            message: "SWF shapes hold only lines and quadratic curves, so cubic curves were approximated within 0.05 px".into(),
        });
    }

    let fps = f64::from(document.frame_rate().numerator())
        / f64::from(document.frame_rate().denominator());
    if fps > 255.0 {
        diagnostics.push(Diagnostic {
            severity: Severity::Warning,
            message: "a SWF frame rate is at most 255 fps; it was capped".into(),
        });
    }
    let header = Header {
        compression: if options.compress {
            Compression::Zlib
        } else {
            Compression::None
        },
        version: SWF_VERSION,
        stage_size: Rectangle {
            x_min: Twips::ZERO,
            x_max: twips(f64::from(stage.width())),
            y_min: Twips::ZERO,
            y_max: twips(f64::from(stage.height())),
        },
        frame_rate: Fixed8::from_f64(fps.min(255.0)),
        num_frames: u16::try_from(total).unwrap_or(u16::MAX),
    };
    let mut bytes = Vec::new();
    swf::write_swf(&header, &tags, &mut bytes).map_err(|e| PublishError::Write(e.to_string()))?;
    Ok(Published {
        bytes,
        frames,
        diagnostics,
    })
}
