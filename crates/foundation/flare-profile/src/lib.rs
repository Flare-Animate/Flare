//! Workspace profiles: the rooms, dock layouts, toolbars and defaults an interface starts from.
//!
//! A profile is data, not code. The four built-in profiles ship as JSON in `data/` and go through
//! the same reader (`load::parse`) as a file a person might add, so a built-in one is held to the
//! same rules. Every profile must place every panel somewhere, so no profile hides a feature.

pub mod builtin;
pub mod load;
pub mod model;

#[cfg(test)]
mod tests;

pub use load::{SHORTCUT_SETS, parse, validate};
pub use model::{Axis, DockNode, PanelId, Profile, Room, Toolbar};

/// Why a profile was refused. A refused profile is never used in part.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ProfileError {
    /// The file is larger than the reader accepts; it is not parsed at all.
    #[error("the profile is {bytes} bytes, more than the reader accepts")]
    TooLarge { bytes: usize },
    #[error("the profile is not valid: {0}")]
    Parse(String),
    #[error("profile schema {0} is not supported")]
    Schema(u32),
    #[error("{what} {value:?} is not valid")]
    BadText { what: &'static str, value: String },
    #[error("{what} {value:?} is not one of the known ones")]
    Unknown { what: &'static str, value: String },
    #[error("a profile needs at least one room")]
    NoRooms,
    #[error("a profile may have more than {max} {what}")]
    TooMany { what: &'static str, max: usize },
    #[error("default room {0} does not exist")]
    DefaultRoom(usize),
    #[error("a dock layout is nested deeper than the reader accepts")]
    TooDeep,
    #[error("a split gives its first part {0}, which is outside 0.05 to 0.95")]
    Fraction(f32),
    #[error("a tab stack holds no panels")]
    EmptyStack,
    #[error("tab {active} is not in a stack of {len} panels")]
    ActiveTab { active: usize, len: usize },
    #[error("{what} {value} appears more than once")]
    Duplicate { what: &'static str, value: String },
    #[error("the profile leaves out panels: {0:?}")]
    MissingPanels(Vec<PanelId>),
}
