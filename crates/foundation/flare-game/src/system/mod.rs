//! The seven systems, run in one fixed order each step. Each reads the components it needs and
//! writes the ones it owns; none reaches into the document, the window or the operating system.

pub(crate) mod audio;
pub(crate) mod camera;
pub(crate) mod hook;
pub(crate) mod particle;
pub(crate) mod physics;
pub(crate) mod script;
pub(crate) mod sprite;
pub(crate) mod trigger;
pub(crate) mod ui;

pub use camera::View;

use crate::limits::MAX_EVENTS_PER_STEP;
use crate::{EntityId, Event, GameError, Input, World};

/// The systems. The C++ scaffold sorted its systems by a priority number; the port fixes the
/// order here so that a camera follows the positions the physics system has just written, and a
/// hook fires on the entries the trigger system has just reported, before the camera moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemKind {
    Script,
    Physics,
    Trigger,
    Hook,
    Camera,
    Sprite,
    Particle,
    Audio,
    Ui,
}

impl SystemKind {
    pub const ORDER: [SystemKind; 9] = [
        SystemKind::Script,
        SystemKind::Physics,
        SystemKind::Trigger,
        SystemKind::Hook,
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
            SystemKind::Trigger => "trigger",
            SystemKind::Hook => "hook",
            SystemKind::Camera => "camera",
            SystemKind::Sprite => "sprite",
            SystemKind::Particle => "particle",
            SystemKind::Audio => "audio",
            SystemKind::Ui => "ui",
        }
    }
}

/// What a step gives every system: the seconds it advances, the input, the events so far, and
/// the trigger entries this step, which the trigger system writes and the hook system reads.
pub(crate) struct StepContext<'a> {
    pub(crate) dt: f64,
    pub(crate) input: &'a Input,
    /// The `(zone, visitor)` pairs whose overlap began this step, in report order.
    pub(crate) triggers: Vec<(EntityId, EntityId)>,
    events: Vec<Event>,
}

impl<'a> StepContext<'a> {
    pub(crate) fn new(dt: f64, input: &'a Input) -> Self {
        Self {
            dt,
            input,
            triggers: Vec::new(),
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
        SystemKind::Trigger => trigger::run(world, ctx),
        SystemKind::Hook => hook::run(world, ctx),
        SystemKind::Camera => camera::run(world, ctx),
        SystemKind::Sprite => sprite::run(world, ctx),
        SystemKind::Particle => particle::run(world, ctx),
        SystemKind::Audio => audio::run(world, ctx),
        SystemKind::Ui => ui::run(world, ctx),
    }
}
