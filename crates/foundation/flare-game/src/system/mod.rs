//! The seven systems, run in one fixed order each step. Each reads the components it needs and
//! writes the ones it owns; none reaches into the document, the window or the operating system.

pub(crate) mod audio;
pub(crate) mod camera;
pub(crate) mod particle;
pub(crate) mod physics;
pub(crate) mod script;
pub(crate) mod sprite;
pub(crate) mod ui;

pub use camera::View;

use crate::limits::MAX_EVENTS_PER_STEP;
use crate::{Event, GameError, Input, World};

/// The systems. The C++ scaffold sorted its systems by a priority number; the port fixes the
/// order here so that a camera follows the positions the physics system has just written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemKind {
    Script,
    Physics,
    Camera,
    Sprite,
    Particle,
    Audio,
    Ui,
}

impl SystemKind {
    pub const ORDER: [SystemKind; 7] = [
        SystemKind::Script,
        SystemKind::Physics,
        SystemKind::Camera,
        SystemKind::Sprite,
        SystemKind::Particle,
        SystemKind::Audio,
        SystemKind::Ui,
    ];

    pub fn name(self) -> &'static str {
        match self {
            SystemKind::Script => "script",
            SystemKind::Physics => "physics",
            SystemKind::Camera => "camera",
            SystemKind::Sprite => "sprite",
            SystemKind::Particle => "particle",
            SystemKind::Audio => "audio",
            SystemKind::Ui => "ui",
        }
    }
}

/// What a step gives every system: the seconds it advances, the input, and the events so far.
pub(crate) struct StepContext<'a> {
    pub(crate) dt: f64,
    pub(crate) input: &'a Input,
    events: Vec<Event>,
}

impl<'a> StepContext<'a> {
    pub(crate) fn new(dt: f64, input: &'a Input) -> Self {
        Self {
            dt,
            input,
            events: Vec::new(),
        }
    }

    pub(crate) fn emit(&mut self, event: Event) -> Result<(), GameError> {
        if self.events.len() >= MAX_EVENTS_PER_STEP {
            return Err(GameError::TooManyEvents);
        }
        self.events.push(event);
        Ok(())
    }

    pub(crate) fn into_events(self) -> Vec<Event> {
        self.events
    }
}

pub(crate) fn run(
    kind: SystemKind,
    world: &mut World,
    ctx: &mut StepContext,
) -> Result<(), GameError> {
    match kind {
        SystemKind::Script => script::run(world, ctx),
        SystemKind::Physics => physics::run(world, ctx),
        SystemKind::Camera => camera::run(world, ctx),
        SystemKind::Sprite => sprite::run(world, ctx),
        SystemKind::Particle => particle::run(world, ctx),
        SystemKind::Audio => audio::run(world, ctx),
        SystemKind::Ui => ui::run(world, ctx),
    }
}
