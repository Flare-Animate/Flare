//! Lip sync from start to end: decode the sound, read the mouth shape of each frame (the built-in
//! analysis, or Rhubarb Lip Sync when it is installed and asked for), and turn the mouth keyframe
//! into keys of its drawings (`flare_tools::lipsync`). The desktop app runs this on the sound
//! worker ([`crate::AudioJob::LipSync`]); the command line and the MCP server call [`lip_sync`].

mod rhubarb;

pub use rhubarb::{RHUBARB_ENV, RHUBARB_TIME_LIMIT, locate_rhubarb};

use std::path::Path;

use flare_audio::lipsync::{self, Viseme};
use flare_core::{SceneId, SoundId};
use flare_document::Document;
use flare_tools::lipsync::{LipSyncError, MouthMap, lip_sync as place, sound_start};
use flare_tools::ops::SpanRef;
use thiserror::Error;

/// How the mouth shapes are found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Analysis {
    /// Rhubarb Lip Sync when it is installed, the built-in analysis when it is not or fails.
    #[default]
    Auto,
    /// The built-in analysis only.
    BuiltIn,
    /// Rhubarb Lip Sync only; an error when it is not installed or fails.
    Rhubarb,
}

impl Analysis {
    pub const ALL: [Self; 3] = [Self::Auto, Self::BuiltIn, Self::Rhubarb];

    pub fn name(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::BuiltIn => "built-in",
            Self::Rhubarb => "rhubarb",
        }
    }

    /// `auto`, `built-in` (or `builtin`, `built_in`) and `rhubarb`, in any case.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim().to_ascii_lowercase().replace(['_', ' '], "-");
        match text.as_str() {
            "builtin" => Some(Self::BuiltIn),
            other => Self::ALL.into_iter().find(|a| a.name() == other),
        }
    }
}

/// What to lip sync: the mouth keyframe, the voice it follows and the frame each shape shows.
#[derive(Debug, Clone, PartialEq)]
pub struct LipSyncRequest {
    pub scene: SceneId,
    pub at: SpanRef,
    pub sound: SoundId,
    pub map: MouthMap,
    pub analysis: Analysis,
}

/// A lip sync made: the commands to apply as one step, and what went into them.
#[derive(Debug, Clone, PartialEq)]
pub struct LipSyncMade {
    pub commands: Vec<flare_commands::Command>,
    pub keyframes: usize,
    /// The frames covered, from the keyframe's first.
    pub start: u32,
    pub frames: u32,
    /// Which analysis gave the shapes: [`Analysis::BuiltIn`] or [`Analysis::Rhubarb`].
    pub used: Analysis,
    /// The shape of each frame of the sound, from its cue.
    pub shapes: Vec<Viseme>,
    /// Why Rhubarb was not used when it was wanted and the built-in analysis stood in.
    pub fell_back: Option<String>,
}

#[derive(Debug, Error, PartialEq)]
pub enum LipSyncFailure {
    #[error("there is no sound {0} in the library")]
    UnknownSound(String),
    #[error("the sound cannot be read: {0}")]
    Decode(#[from] flare_audio::AudioError),
    #[error("{0}")]
    Analysis(#[from] lipsync::LipSyncError),
    #[error("{0}")]
    Place(#[from] LipSyncError),
    #[error(
        "Rhubarb Lip Sync is not installed: put `rhubarb` on the PATH or name it in {RHUBARB_ENV}, or use the built-in analysis"
    )]
    RhubarbMissing,
    #[error("Rhubarb Lip Sync failed: {0}")]
    Rhubarb(String),
    #[error("no sound is on the timeline to lip sync to")]
    NoSound,
    #[error("more than one sound is on the timeline; name the one to lip sync to")]
    WhichSound,
}

/// The sound to lip sync to: the one `named` (by name or id), or, when no name is given, the one
/// sound on the scene's timeline. The command line and the MCP server both pick with this.
pub fn pick_sound(
    document: &Document,
    scene: SceneId,
    named: Option<&str>,
) -> Result<SoundId, LipSyncFailure> {
    if let Some(text) = named {
        let text = text.trim();
        return document
            .sounds()
            .iter()
            .find(|s| s.name() == text || s.id().as_uuid().to_string() == text)
            .map(flare_document::Sound::id)
            .ok_or_else(|| LipSyncFailure::UnknownSound(text.to_owned()));
    }
    let mut cued: Vec<SoundId> = document
        .scenes()
        .iter()
        .filter(|s| s.id() == scene)
        .flat_map(|s| s.timeline().cues().map(|(_, cue)| cue.sound))
        .collect();
    cued.sort_unstable();
    cued.dedup();
    match cued.as_slice() {
        [] => Err(LipSyncFailure::NoSound),
        [one] => Ok(*one),
        _ => Err(LipSyncFailure::WhichSound),
    }
}

/// Lip syncs `request` in `document`. `rhubarb` is the program to run for
/// [`Analysis::Auto`] and [`Analysis::Rhubarb`] ([`locate_rhubarb`] finds the installed one);
/// `None` means it is not there. Nothing here changes the document: apply the commands.
pub fn lip_sync(
    document: &Document,
    request: &LipSyncRequest,
    rhubarb: Option<&Path>,
) -> Result<LipSyncMade, LipSyncFailure> {
    let sound = document
        .sound(request.sound)
        .ok_or_else(|| LipSyncFailure::UnknownSound(request.sound.as_uuid().to_string()))?;
    let cue =
        sound_start(document, request.scene, request.sound).ok_or(LipSyncError::SoundNotCued)?;
    let audio = flare_audio::decode(sound.data())?;
    let per_second = document.frame_rate().per_second();
    let (shapes, used, fell_back) = match (request.analysis, rhubarb) {
        (Analysis::BuiltIn, _) => (
            lipsync::analyze(&audio, per_second)?,
            Analysis::BuiltIn,
            None,
        ),
        (Analysis::Rhubarb, None) => return Err(LipSyncFailure::RhubarbMissing),
        (Analysis::Rhubarb, Some(program)) => (
            rhubarb_shapes(program, &audio, per_second)?,
            Analysis::Rhubarb,
            None,
        ),
        (Analysis::Auto, None) => (
            lipsync::analyze(&audio, per_second)?,
            Analysis::BuiltIn,
            None,
        ),
        (Analysis::Auto, Some(program)) => match rhubarb_shapes(program, &audio, per_second) {
            Ok(shapes) => (shapes, Analysis::Rhubarb, None),
            Err(error) => (
                lipsync::analyze(&audio, per_second)?,
                Analysis::BuiltIn,
                Some(error.to_string()),
            ),
        },
    };
    let made = place(
        document,
        request.scene,
        request.at,
        cue,
        &shapes,
        &request.map,
    )?;
    Ok(LipSyncMade {
        commands: made.commands,
        keyframes: made.keyframes,
        start: made.start,
        frames: made.frames,
        used,
        shapes,
        fell_back,
    })
}

fn rhubarb_shapes(
    program: &Path,
    audio: &flare_audio::Audio,
    per_second: f64,
) -> Result<Vec<Viseme>, LipSyncFailure> {
    let frames = lipsync::frames_for(audio.seconds(), per_second)?;
    let output = rhubarb::run(program, audio).map_err(LipSyncFailure::Rhubarb)?;
    let cues = lipsync::parse_rhubarb(&output)?;
    Ok(lipsync::frames_from_cues(&cues, per_second, frames)?)
}

#[cfg(test)]
mod tests;
