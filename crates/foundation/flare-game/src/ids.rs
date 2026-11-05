//! Entity identities. Small integers, handed out in order and never reused within a game, so
//! an ID named in a command or a script always means the same entity.

use serde::{Deserialize, Serialize};

use std::fmt;

/// An entity's ID. Zero is never an entity (the C++ scaffold's `INVALID_ENTITY`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(into = "u32", try_from = "u32")]
pub struct EntityId(u32);

impl TryFrom<u32> for EntityId {
    type Error = &'static str;

    /// Zero is refused, so a stored file cannot name an entity the world could not hold.
    fn try_from(raw: u32) -> Result<Self, Self::Error> {
        Self::new(raw).ok_or("zero is not an entity ID")
    }
}

impl From<EntityId> for u32 {
    fn from(id: EntityId) -> Self {
        id.0
    }
}

impl EntityId {
    /// The ID with this number, or `None` for zero.
    pub const fn new(raw: u32) -> Option<Self> {
        if raw == 0 { None } else { Some(Self(raw)) }
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

impl fmt::Display for EntityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
