//! A small entity-component game world for Flare's game description: entities with typed
//! components, seven systems run in a fixed order, and a fixed-timestep step that is a pure
//! function of the description and the scripted input. No window, sound card, scripting
//! engine or file access lives here; the surfaces (CLI, MCP, desktop) drive it.
//!
//! The model is the one the archived C++ `tgame` scaffold describes: entities are IDs, components
//! are data, systems are logic over the components they require. See `docs/GAME.md` for what
//! each system does here and what the C++ left empty.

mod component;
mod digest;
mod error;
mod ids;
mod input;
mod limits;
mod run;
mod spec;
mod system;
mod world;

pub use component::{
    Audio, Body, BodyKind, Bounds, Camera, Component, ComponentKind, Control, Key, Particle,
    Script, Sprite, Transform, Ui, UiKind,
};
pub use error::GameError;
pub use ids::EntityId;

pub use input::{Input, parse_script};
pub use limits::*;
pub use run::{Playback, play};
pub use spec::{EntitySpec, GameSpec};
pub use system::{SystemKind, View};
pub use world::{Event, SpriteFrame, World};
