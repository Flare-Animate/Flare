//! The running world: the entities with their components, the step that runs the systems in
//! order, and the read-only views the surfaces draw from.

use std::collections::{BTreeMap, BTreeSet};

use flare_core::SoundId;

use crate::system::{self, StepContext, SystemKind, View};
use crate::{Component, ComponentKind, EntityId, GameError, GameSpec, Input, MAX_FPS, Transform};

/// Something a step reports. Playing a sound, drawing a click's feedback and spawning particles
/// are the surface's job; the world only says they happened.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// Two bodies began (`begin`) or stopped (`!begin`) overlapping. `a` is the smaller ID.
    Collision {
        a: EntityId,
        b: EntityId,
        begin: bool,
    },
    /// The pointer was pressed inside a UI element of `entity`.
    Click { entity: EntityId, x: f64, y: f64 },
    /// A sound starts: an audio component with play-on-start, on the step after the game starts.
    Sound {
        entity: EntityId,
        sound: SoundId,
        volume: f64,
        looping: bool,
    },
    /// An emitter produced `count` particles this step.
    Emit { entity: EntityId, count: u32 },
}

/// A visible sprite and the frame it shows now.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpriteFrame {
    pub entity: EntityId,
    pub frame: u32,
    pub order: i32,
    pub alpha: f64,
}

/// One entity's state at run time. Its components are the spec's, changed by the systems;
/// the rest is what the systems remember between steps.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Live {
    pub(crate) components: BTreeMap<ComponentKind, Component>,
    /// The sprite frame now shown.
    pub(crate) frame: u32,
    /// Fractional frames of animation not yet shown.
    pub(crate) clock: f64,
    /// Fractional particles not yet emitted.
    pub(crate) emit_carry: f64,
    /// Whether the audio system has already looked at this entity.
    pub(crate) sounded: bool,
}

/// A game being run. Built from a [`GameSpec`] at a frame rate, then stepped one frame at a time.
#[derive(Debug, Clone, PartialEq)]
pub struct World {
    pub(crate) dt: f64,
    pub(crate) gravity: (f64, f64),
    pub(crate) entities: BTreeMap<EntityId, Live>,
    pub(crate) overlaps: BTreeSet<(EntityId, EntityId)>,
    steps: u64,
}

impl World {
    /// The world the description starts in. The step is `1 / frames_per_second` seconds, so a
    /// run is the same however fast the machine is.
    pub fn from_spec(spec: &GameSpec, frames_per_second: f64) -> Result<Self, GameError> {
        if !frames_per_second.is_finite() || frames_per_second <= 0.0 || frames_per_second > MAX_FPS
        {
            return Err(GameError::BadFrameRate);
        }
        spec.validate()?;
        let entities = spec
            .entities()
            .map(|entity| {
                let components: BTreeMap<ComponentKind, Component> = entity
                    .components()
                    .map(|component| (component.kind(), component.clone()))
                    .collect();
                let frame = match components.get(&ComponentKind::Sprite) {
                    Some(Component::Sprite(sprite)) => sprite.frame,
                    _ => 0,
                };
                (
                    entity.id(),
                    Live {
                        components,
                        frame,
                        clock: 0.0,
                        emit_carry: 0.0,
                        sounded: false,
                    },
                )
            })
            .collect();
        Ok(Self {
            dt: 1.0 / frames_per_second,
            gravity: spec.gravity(),
            entities,
            overlaps: BTreeSet::new(),
            steps: 0,
        })
    }

    /// Runs one frame: every system in [`SystemKind::ORDER`], once each, with `input`. The step
    /// runs on a copy and is kept only when every system finishes, so a refused input or a step
    /// that produces too many events leaves the world as it was.
    pub fn step(&mut self, input: &Input) -> Result<Vec<Event>, GameError> {
        input.validate()?;
        let mut next = self.clone();
        let mut ctx = StepContext::new(next.dt, input);
        for kind in SystemKind::ORDER {
            system::run(kind, &mut next, &mut ctx)?;
        }
        next.steps += 1;
        *self = next;
        Ok(ctx.into_events())
    }

    /// Frames stepped so far.
    pub fn steps(&self) -> u64 {
        self.steps
    }

    /// The seconds one step advances.
    pub fn step_seconds(&self) -> f64 {
        self.dt
    }

    pub fn transform(&self, id: EntityId) -> Result<Transform, GameError> {
        match self.component(id, ComponentKind::Transform)? {
            Component::Transform(t) => Ok(*t),
            _ => Err(GameError::NoSuchEntity(id)),
        }
    }

    /// The entity's `(x, y)`.
    pub fn position(&self, id: EntityId) -> Result<(f64, f64), GameError> {
        let t = self.transform(id)?;
        Ok((t.x, t.y))
    }

    /// The velocity of an entity with a body.
    pub fn velocity(&self, id: EntityId) -> Result<(f64, f64), GameError> {
        match self.component(id, ComponentKind::Body)? {
            Component::Body(body) => Ok((body.vx, body.vy)),
            _ => Err(GameError::NoSuchEntity(id)),
        }
    }

    /// The sprite frame an entity shows now.
    pub fn sprite_frame(&self, id: EntityId) -> Result<u32, GameError> {
        let live = self.live(id)?;
        match live.components.get(&ComponentKind::Sprite) {
            Some(_) => Ok(live.frame),
            None => Err(GameError::NoSuchEntity(id)),
        }
    }

    /// The visible sprites, lowest draw order first and by ID within an order.
    pub fn sprites_in_order(&self) -> Vec<SpriteFrame> {
        let mut out: Vec<SpriteFrame> = self
            .entities
            .iter()
            .filter_map(
                |(id, live)| match live.components.get(&ComponentKind::Sprite) {
                    Some(Component::Sprite(s)) if s.visible => Some(SpriteFrame {
                        entity: *id,
                        frame: live.frame,
                        order: s.order,
                        alpha: s.alpha,
                    }),
                    _ => None,
                },
            )
            .collect();
        out.sort_by_key(|sprite| (sprite.order, sprite.entity));
        out
    }

    /// The view a camera gives now.
    pub fn view(&self, camera: EntityId) -> Result<View, GameError> {
        system::camera::view(self, camera)
    }

    pub fn len(&self) -> usize {
        self.entities.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    pub(crate) fn live(&self, id: EntityId) -> Result<&Live, GameError> {
        self.entities.get(&id).ok_or(GameError::NoSuchEntity(id))
    }

    fn component(&self, id: EntityId, kind: ComponentKind) -> Result<&Component, GameError> {
        self.live(id)?
            .components
            .get(&kind)
            .ok_or(GameError::NoSuchEntity(id))
    }
}
