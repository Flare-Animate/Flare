//! Lip sync, as Animate does it: a Graphic symbol holds one mouth drawing on each of its frames,
//! each mouth shape is mapped to one of those frames, and syncing to a sound turns the span that
//! shows the symbol into keyframes of it on Single Frame, one where the mouth changes. The result
//! is ordinary keyframes, so every one can be moved, retimed or given another frame afterwards.
//!
//! What shape each frame of the sound shows comes from `flare_audio::lipsync` (the built-in
//! reading, or Rhubarb Lip Sync's); this module only places it.

mod map;

pub use flare_audio::lipsync::Viseme;
pub use map::{MapError, MouthMap};

use std::num::NonZeroU32;

use flare_commands::Command;
use flare_core::{InstanceId, SceneId, SoundId};
use flare_document::{Content, Document, FrameSpan, Instance, PlayMode, Playback, SymbolKind};
use thiserror::Error;

use crate::common::{SpanRef, find_span, scene_timeline};

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum LipSyncError {
    #[error("there is no keyframe there to lip sync")]
    NoSpan,
    #[error("lip sync needs a keyframe holding one instance of a Graphic symbol")]
    NotAGraphic,
    #[error("the layer is locked")]
    Locked,
    #[error("the sound is not on the timeline of this scene")]
    SoundNotCued,
    #[error("the sound ends before the keyframe starts, so there is nothing to sync")]
    NothingToSync,
}

/// What a lip sync made: the commands (one undo step) and what they do.
#[derive(Debug, Clone, PartialEq)]
pub struct LipSynced {
    pub commands: Vec<Command>,
    /// How many keyframes the span became.
    pub keyframes: usize,
    /// The frames covered, from the span's first.
    pub start: u32,
    pub frames: u32,
}

/// The frame of the scene where `sound` first starts: lip sync times the mouth from there.
pub fn sound_start(document: &Document, scene: SceneId, sound: SoundId) -> Option<u32> {
    scene_timeline(document, scene)?
        .cues()
        .filter(|(_, cue)| cue.sound == sound)
        .map(|(_, cue)| cue.start)
        .min()
}

/// Turns the span `at` names, which shows a Graphic, into keyframes of it on Single Frame: frame
/// `f` of the scene shows the drawing `map` gives for `shapes[f - sound_start]`, and the resting
/// mouth before the sound starts and after it ends. The span grows to the end of the sound when
/// the layer has room, and stops at the next keyframe when it does not. The first keyframe keeps
/// the instance's identity; all keep its place. A tween on the span is not kept, since a mouth
/// that changes drawing every few frames is held between them.
pub fn lip_sync(
    document: &Document,
    scene: SceneId,
    at: SpanRef,
    sound_start: u32,
    shapes: &[Viseme],
    map: &MouthMap,
) -> Result<LipSynced, LipSyncError> {
    let layer = scene_timeline(document, scene)
        .and_then(|t| t.layers().iter().find(|l| l.id() == at.layer))
        .ok_or(LipSyncError::NoSpan)?;
    if layer.is_locked() {
        return Err(LipSyncError::Locked);
    }
    let span = find_span(document, scene, at).ok_or(LipSyncError::NoSpan)?;
    let instance = graphic_instance(document, span)?;
    let last_drawing = document
        .symbol(instance.symbol())
        .map(|s| u32::try_from(s.timeline().frame_count().saturating_sub(1)).unwrap_or(u32::MAX))
        .unwrap_or(0);

    let start = span.start();
    let sound_end = u64::from(sound_start) + shapes.len() as u64;
    if sound_end <= u64::from(start) {
        return Err(LipSyncError::NothingToSync);
    }
    // Up to the end of the sound, or of the span if that is later, but never into the next key.
    let next_key = layer
        .spans()
        .iter()
        .map(FrameSpan::start)
        .filter(|s| *s > start)
        .min()
        .map_or(u64::from(u32::MAX), u64::from);
    let end = sound_end.max(span.end()).min(next_key);
    let frames = u32::try_from(end - u64::from(start)).unwrap_or(u32::MAX);

    let drawing_at = |frame: u32| {
        let shape = frame
            .checked_sub(sound_start)
            .and_then(|i| shapes.get(i as usize))
            .copied()
            .unwrap_or(Viseme::Neutral);
        map.frame(shape).min(last_drawing)
    };
    let mut commands = vec![Command::RemoveSpan {
        scene,
        layer: at.layer,
        start,
    }];
    let mut run_start = start;
    let mut run_drawing = drawing_at(start);
    let mut keyframes = 0;
    let mut place = |from: u32, to: u32, drawing: u32, commands: &mut Vec<Command>| {
        let id = if keyframes == 0 {
            instance.id()
        } else {
            InstanceId::new()
        };
        let key =
            Instance::new(id, instance.symbol(), instance.transform()).with_playback(Playback {
                mode: PlayMode::SingleFrame,
                first_frame: drawing,
            });
        if let Some(duration) = NonZeroU32::new(to - from) {
            keyframes += 1;
            commands.push(Command::InsertSpan {
                scene,
                layer: at.layer,
                span: FrameSpan::new(from, duration, Content::Instance(key)),
            });
        }
    };
    // Before the sound starts and after it ends the mouth rests, so only the frames between can
    // change it: a long span is not walked frame by frame.
    let end = start + frames;
    let changes_until = u32::try_from(sound_end + 1).map_or(end, |e| e.min(end));
    for frame in sound_start.max(start + 1)..changes_until {
        let drawing = drawing_at(frame);
        if drawing != run_drawing {
            place(run_start, frame, run_drawing, &mut commands);
            (run_start, run_drawing) = (frame, drawing);
        }
    }
    place(run_start, end, run_drawing, &mut commands);
    Ok(LipSynced {
        commands,
        keyframes,
        start,
        frames,
    })
}

/// The one Graphic instance a span shows, or why it cannot be lip synced.
fn graphic_instance<'a>(
    document: &Document,
    span: &'a FrameSpan,
) -> Result<&'a Instance, LipSyncError> {
    let Content::Instance(instance) = span.content() else {
        return Err(LipSyncError::NotAGraphic);
    };
    let graphic = document
        .symbol(instance.symbol())
        .is_some_and(|s| s.kind() == SymbolKind::Graphic);
    if !graphic || span.element_count() != 1 {
        return Err(LipSyncError::NotAGraphic);
    }
    Ok(instance)
}

#[cfg(test)]
mod tests;
