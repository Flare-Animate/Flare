//! Why an edit was refused.

use flare_core::{BitmapId, LayerId, SceneId, SoundId, SymbolId};
use flare_document::{Diagnostic, SoundError, TimelineError};
use thiserror::Error;

/// The longest a scene's name may be, so a pasted blob cannot become one.
pub const MAX_SCENE_NAME_CHARS: usize = 256;

#[derive(Debug, Error, PartialEq)]
pub enum EditError {
    #[error("a scene's name must be 1 to {MAX_SCENE_NAME_CHARS} characters")]
    SceneName,
    #[error("no such scene {0:?}")]
    NoSuchScene(SceneId),
    #[error("no such symbol {0:?}")]
    NoSuchSymbol(SymbolId),
    #[error("no such layer {0:?}")]
    NoSuchLayer(LayerId),
    #[error("no span starts at frame {0}")]
    NoSuchSpan(u32),
    #[error("the span at frame {0} holds no symbol instance")]
    NotAnInstance(u32),
    #[error(transparent)]
    Timeline(#[from] TimelineError),
    #[error(transparent)]
    Sound(#[from] SoundError),
    #[error("no such sound {0:?}")]
    NoSuchSound(SoundId),
    #[error("no such bitmap {0:?}")]
    NoSuchBitmap(BitmapId),
    #[error("an identical bitmap is already in the library as {0:?}")]
    DuplicateBitmap(BitmapId),
    #[error(transparent)]
    Game(#[from] flare_game::GameError),
    #[error("not a game edit")]
    NotAGameEdit,
    #[error("edit would make the document invalid: {0:?}")]
    Invalid(Vec<Diagnostic>),
}

impl From<Vec<Diagnostic>> for EditError {
    fn from(value: Vec<Diagnostic>) -> Self {
        Self::Invalid(value)
    }
}
