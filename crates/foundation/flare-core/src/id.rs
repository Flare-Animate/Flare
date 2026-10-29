//! Persistent identities.
//!
//! Each entity kind has its own newtype so IDs cannot be mixed up. UUID ordering is
//! not an animation clock, document revision, or authorization.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

macro_rules! define_id {
    ($($(#[$meta:meta])* $name:ident),+ $(,)?) => {$(
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            pub fn new() -> Self {
                Self(Uuid::now_v7())
            }

            pub const fn from_uuid(value: Uuid) -> Self {
                Self(value)
            }

            pub const fn as_uuid(&self) -> &Uuid {
                &self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }
    )+};
}

define_id! {
    /// Identity of a whole document.
    DocumentId,
    SceneId,
    SymbolId,
    InstanceId,
    LayerId,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_and_round_trip_through_uuid() {
        let (a, b) = (SymbolId::new(), SymbolId::new());
        assert_ne!(a, b);
        assert_eq!(SymbolId::from_uuid(*a.as_uuid()), a);
    }
}
