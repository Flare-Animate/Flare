//! Why a game description, an input script or a step was refused.

use flare_core::{SoundId, SymbolId};
use thiserror::Error;

use crate::limits::{MAX_COORD, MAX_EVENTS_PER_STEP, MAX_NAME_CHARS, MAX_RUN_FRAMES};
use crate::{ComponentKind, EntityId};

#[derive(Debug, Clone, PartialEq, Error)]
pub enum GameError {
    #[error("a game holds at most {limit} entities")]
    TooManyEntities { limit: usize },
    #[error("a game holds at most {limit} entities with a body")]
    TooManyBodies { limit: usize },
    #[error("no entity {0:?}")]
    NoSuchEntity(EntityId),
    #[error("entity {0:?} already exists")]
    DuplicateEntity(EntityId),
    #[error("an entity has no more IDs to hand out")]
    NoFreeId,
    #[error("an entity's name must be 1 to {MAX_NAME_CHARS} characters with no control characters")]
    BadName,
    #[error("{component} needs {needs} on the same entity")]
    MissingComponent {
        component: ComponentKind,
        needs: ComponentKind,
    },
    #[error("{component} cannot be removed while {by} needs it")]
    StillNeeded {
        component: ComponentKind,
        by: ComponentKind,
    },
    #[error("entity {0:?} is followed by a camera")]
    Followed(EntityId),
    #[error("unknown component {0:?}")]
    UnknownComponent(String),
    #[error("{component} has no field {field:?}")]
    UnknownField {
        component: &'static str,
        field: String,
    },
    #[error("{component} field {field}: {reason}")]
    BadField {
        component: &'static str,
        field: &'static str,
        reason: String,
    },
    #[error("unknown key {0:?}")]
    UnknownKey(String),
    #[error("no such symbol {0:?}")]
    NoSuchSymbol(SymbolId),
    #[error("no such sound {0:?}")]
    NoSuchSound(SoundId),
    #[error("gravity {0} is not a finite number")]
    NonFinite(&'static str),
    #[error("an entity's position left the range of {MAX_COORD} pixels")]
    Diverged,
    #[error("input is refused: {0}")]
    BadInput(String),
    #[error("a run may step at most {MAX_RUN_FRAMES} frames")]
    TooManyFrames,
    #[error("a step produced more than {MAX_EVENTS_PER_STEP} events")]
    TooManyEvents,
    #[error("the frame rate must be above 0 and at most {0} frames per second")]
    BadFrameRate(f64),
}
