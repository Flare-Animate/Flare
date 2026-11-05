//! Components: the data an entity holds, how each kind is checked, and how a description
//! names them.

use serde::{Deserialize, Serialize};

mod parse;
mod types;
mod validate;

use std::fmt;

pub use types::{
    Audio, Body, BodyKind, Bounds, Camera, Control, Key, Particle, Script, Sprite, Transform, Ui,
    UiKind,
};

use crate::GameError;

/// One kind of component. The C++ scaffold numbered its built-in types 1 to 8 with a bitmask;
/// the order here is the same idea, and the derived ordering keeps component maps deterministic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ComponentKind {
    Transform,
    Sprite,
    Body,
    Control,
    Camera,
    Ui,
    Audio,
    Particle,
    Script,
}

impl ComponentKind {
    pub const ALL: [ComponentKind; 9] = [
        ComponentKind::Transform,
        ComponentKind::Sprite,
        ComponentKind::Body,
        ComponentKind::Control,
        ComponentKind::Camera,
        ComponentKind::Ui,
        ComponentKind::Audio,
        ComponentKind::Particle,
        ComponentKind::Script,
    ];

    /// The name the surfaces use, as in `flare game set-component ... body`.
    pub fn name(self) -> &'static str {
        match self {
            ComponentKind::Transform => "transform",
            ComponentKind::Sprite => "sprite",
            ComponentKind::Body => "body",
            ComponentKind::Control => "control",
            ComponentKind::Camera => "camera",
            ComponentKind::Ui => "ui",
            ComponentKind::Audio => "audio",
            ComponentKind::Particle => "particle",
            ComponentKind::Script => "script",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.name() == text)
    }

    /// The kind an entity must also hold before it may hold this one. Every component needs a
    /// transform except the transform itself; a control needs the body it moves.
    pub fn requires(self) -> Option<ComponentKind> {
        match self {
            ComponentKind::Transform => None,
            ComponentKind::Control => Some(ComponentKind::Body),
            _ => Some(ComponentKind::Transform),
        }
    }
}

impl fmt::Display for ComponentKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// A component's data, one variant per kind.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Component {
    Transform(Transform),
    Sprite(Sprite),
    Body(Body),
    Control(Control),
    Camera(Camera),
    Ui(Ui),
    Audio(Audio),
    Particle(Particle),
    Script(Script),
}

impl Component {
    pub fn kind(&self) -> ComponentKind {
        match self {
            Component::Transform(_) => ComponentKind::Transform,
            Component::Sprite(_) => ComponentKind::Sprite,
            Component::Body(_) => ComponentKind::Body,
            Component::Control(_) => ComponentKind::Control,
            Component::Camera(_) => ComponentKind::Camera,
            Component::Ui(_) => ComponentKind::Ui,
            Component::Audio(_) => ComponentKind::Audio,
            Component::Particle(_) => ComponentKind::Particle,
            Component::Script(_) => ComponentKind::Script,
        }
    }

    /// Builds a component of `kind` from `name=value` fields. Missing fields take their
    /// defaults; unknown fields and values that do not check out are refused.
    pub fn parse(kind: &str, fields: &[(String, String)]) -> Result<Self, GameError> {
        parse::parse(kind, fields)
    }

    /// Checks every value against its range and its meaning.
    pub fn validate(&self) -> Result<(), GameError> {
        validate::validate(self)
    }
}
