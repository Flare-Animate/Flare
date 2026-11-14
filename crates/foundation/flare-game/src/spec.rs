//! The game description: the entities a project declares, their components and the world's
//! gravity. It is plain data; the document stores it and validates the references into its own
//! library (symbols and sounds) on top of what is checked here.

use serde::{Deserialize, Serialize};

use std::collections::BTreeMap;

use flare_core::SymbolId;

use crate::limits::{MAX_BODIES, MAX_COORD, MAX_ENTITIES, MAX_NAME_CHARS, MAX_TRIGGERS};
use crate::{Component, ComponentKind, EntityId, GameError, HookWhen};

/// One entity: an ID, a name (its tag), the library symbol it draws if any, and at most one
/// component of each kind.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "EntityWire")]
pub struct EntitySpec {
    id: EntityId,
    name: String,
    symbol: Option<SymbolId>,
    components: BTreeMap<ComponentKind, Component>,
}

impl EntitySpec {
    pub fn new(id: EntityId, name: impl Into<String>) -> Result<Self, GameError> {
        let name = name.into();
        check_name(&name)?;
        Ok(Self {
            id,
            name,
            symbol: None,
            components: BTreeMap::new(),
        })
    }

    /// The same entity drawing `symbol`.
    #[must_use]
    pub fn with_symbol(mut self, symbol: Option<SymbolId>) -> Self {
        self.symbol = symbol;
        self
    }

    /// The entity with `component` set (replacing one of its kind). The entity's other
    /// components must already satisfy what this one needs.
    pub fn with_component(mut self, component: Component) -> Result<Self, GameError> {
        component.validate()?;
        self.check_requirement(&component)?;
        self.components.insert(component.kind(), component);
        Ok(self)
    }

    pub fn id(&self) -> EntityId {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn symbol(&self) -> Option<SymbolId> {
        self.symbol
    }

    pub fn component(&self, kind: ComponentKind) -> Option<&Component> {
        self.components.get(&kind)
    }

    pub fn components(&self) -> impl Iterator<Item = &Component> {
        self.components.values()
    }

    pub(crate) fn check_requirement(&self, component: &Component) -> Result<(), GameError> {
        let kind = component.kind();
        match kind.requires() {
            Some(needs) if !self.components.contains_key(&needs) => {
                return Err(GameError::MissingComponent {
                    component: kind,
                    needs,
                });
            }
            _ => {}
        }
        // A hook that listens for its zone needs the zone: an action `when` does not.
        if let Component::Hook(hook) = component
            && matches!(hook.when, HookWhen::Trigger)
            && !self.components.contains_key(&ComponentKind::Trigger)
        {
            return Err(GameError::MissingComponent {
                component: kind,
                needs: ComponentKind::Trigger,
            });
        }
        Ok(())
    }

    pub(crate) fn set_component(&mut self, component: Component) -> Result<(), GameError> {
        component.validate()?;
        self.check_requirement(&component)?;
        self.components.insert(component.kind(), component);
        Ok(())
    }

    /// Removes a component. Refused while another component on the entity needs it, or while a
    /// trigger-waiting hook would lose the zone it listens for.
    pub(crate) fn remove_component(&mut self, kind: ComponentKind) -> Result<(), GameError> {
        if let Some(by) = self
            .components
            .keys()
            .find(|other| other.requires() == Some(kind))
        {
            return Err(GameError::StillNeeded {
                component: kind,
                by: *by,
            });
        }
        if kind == ComponentKind::Trigger
            && self.components.values().any(|component| {
                matches!(component, Component::Hook(hook) if matches!(hook.when, HookWhen::Trigger))
            })
        {
            return Err(GameError::StillNeeded {
                component: kind,
                by: ComponentKind::Hook,
            });
        }
        self.components.remove(&kind);
        Ok(())
    }

    /// Checks the name, every component and every requirement.
    pub(crate) fn check(&self) -> Result<(), GameError> {
        check_name(&self.name)?;
        for (kind, component) in &self.components {
            component.validate()?;
            debug_assert_eq!(*kind, component.kind());
            self.check_requirement(component)?;
        }
        Ok(())
    }
}

/// An entity as stored. Converting it goes through the checks a command makes, so a file cannot
/// hold a component the editor would refuse.
#[derive(Deserialize)]
struct EntityWire {
    id: EntityId,
    name: String,
    symbol: Option<SymbolId>,
    components: BTreeMap<ComponentKind, Component>,
}

impl TryFrom<EntityWire> for EntitySpec {
    type Error = GameError;

    fn try_from(wire: EntityWire) -> Result<Self, GameError> {
        let mut entity = EntitySpec::new(wire.id, wire.name)?.with_symbol(wire.symbol);
        for (kind, component) in wire.components {
            if component.kind() != kind {
                return Err(GameError::BadInput(format!(
                    "a {kind} is stored under the wrong kind"
                )));
            }
            entity = entity.with_component(component)?;
        }
        Ok(entity)
    }
}

/// A game as stored: its gravity and its entities by ID.
#[derive(Deserialize)]
struct GameWire {
    gravity: (f64, f64),
    entities: BTreeMap<EntityId, EntitySpec>,
}

impl TryFrom<GameWire> for GameSpec {
    type Error = GameError;

    fn try_from(wire: GameWire) -> Result<Self, GameError> {
        let mut spec = GameSpec::new().with_gravity(wire.gravity.0, wire.gravity.1)?;
        for (id, entity) in wire.entities {
            if entity.id() != id {
                return Err(GameError::BadInput(format!(
                    "entity {id} is stored under another ID"
                )));
            }
            spec.add_entity(entity)?;
        }
        Ok(spec)
    }
}

/// The game: its entities, in ID order, and the world's gravity in pixels per second squared.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "GameWire")]
pub struct GameSpec {
    gravity: (f64, f64),
    entities: BTreeMap<EntityId, EntitySpec>,
}

impl Default for GameSpec {
    fn default() -> Self {
        Self {
            gravity: (0.0, 0.0),
            entities: BTreeMap::new(),
        }
    }
}

impl GameSpec {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn gravity(&self) -> (f64, f64) {
        self.gravity
    }

    /// The world with gravity set to `(x, y)`. Refused when either part is not finite or is too
    /// large to keep a body inside the coordinate limit in a few frames.
    pub fn with_gravity(mut self, x: f64, y: f64) -> Result<Self, GameError> {
        for (name, value) in [("x", x), ("y", y)] {
            if !value.is_finite() {
                return Err(GameError::NonFinite(name));
            }
            if value.abs() > MAX_COORD {
                return Err(GameError::BadField {
                    component: "gravity",
                    field: "x",
                    reason: format!("is larger than {MAX_COORD}"),
                });
            }
        }
        self.gravity = (x, y);
        Ok(self)
    }

    pub fn entities(&self) -> impl Iterator<Item = &EntitySpec> {
        self.entities.values()
    }

    pub fn entity(&self, id: EntityId) -> Option<&EntitySpec> {
        self.entities.get(&id)
    }

    pub fn len(&self) -> usize {
        self.entities.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    /// The next free ID: one more than the largest in use.
    pub fn next_id(&self) -> Result<EntityId, GameError> {
        let largest = self.entities.keys().next_back().map_or(0, |id| id.get());
        largest
            .checked_add(1)
            .and_then(EntityId::new)
            .ok_or(GameError::NoFreeId)
    }

    /// Adds an entity. Its ID must be new, and the game must stay within its limits.
    pub fn add_entity(&mut self, entity: EntitySpec) -> Result<(), GameError> {
        entity.check()?;
        if self.entities.contains_key(&entity.id) {
            return Err(GameError::DuplicateEntity(entity.id));
        }
        if self.entities.len() >= MAX_ENTITIES {
            return Err(GameError::TooManyEntities {
                limit: MAX_ENTITIES,
            });
        }
        if let Some(Component::Camera(camera)) = entity.components.get(&ComponentKind::Camera)
            && let Some(target) = camera.follow
            && target != entity.id
            && !self.has(target, ComponentKind::Transform)
        {
            return Err(GameError::NoSuchEntity(target));
        }
        for component in entity.components.values() {
            if let Component::Hook(hook) = component
                && let Some(target) = hook.spawn
                && target != entity.id
                && !self.has(target, ComponentKind::Transform)
            {
                return Err(GameError::NoSuchEntity(target));
            }
        }
        let bodies =
            self.body_count() + usize::from(entity.components.contains_key(&ComponentKind::Body));
        if bodies > MAX_BODIES {
            return Err(GameError::TooManyBodies { limit: MAX_BODIES });
        }
        let triggers = self.trigger_count()
            + usize::from(entity.components.contains_key(&ComponentKind::Trigger));
        if triggers > MAX_TRIGGERS {
            return Err(GameError::TooManyTriggers {
                limit: MAX_TRIGGERS,
            });
        }
        self.entities.insert(entity.id, entity);
        Ok(())
    }

    /// Removes an entity. Refused while a camera follows it or a hook spawns it.
    pub fn remove_entity(&mut self, id: EntityId) -> Result<EntitySpec, GameError> {
        if !self.entities.contains_key(&id) {
            return Err(GameError::NoSuchEntity(id));
        }
        if self.followers(id).next().is_some() {
            return Err(GameError::Followed(id));
        }
        if self.spawners(id).next().is_some() {
            return Err(GameError::Spawned(id));
        }
        self.entities.remove(&id).ok_or(GameError::NoSuchEntity(id))
    }

    /// Sets a component on an entity, replacing one of its kind. Refused when the entity does
    /// not satisfy what the component needs, when a camera would follow or a hook would spawn
    /// an entity that does not exist, or when the body or trigger count would pass its limit. A
    /// refused edit changes nothing.
    pub fn set_component(&mut self, id: EntityId, component: Component) -> Result<(), GameError> {
        let entity = self.entities.get(&id).ok_or(GameError::NoSuchEntity(id))?;
        if let Component::Camera(camera) = &component
            && let Some(target) = camera.follow
            && !self.has(target, ComponentKind::Transform)
        {
            return Err(GameError::NoSuchEntity(target));
        }
        if let Component::Hook(hook) = &component
            && let Some(target) = hook.spawn
            && !self.has(target, ComponentKind::Transform)
        {
            return Err(GameError::NoSuchEntity(target));
        }
        if matches!(component, Component::Body(_))
            && !entity.components.contains_key(&ComponentKind::Body)
            && self.body_count() >= MAX_BODIES
        {
            return Err(GameError::TooManyBodies { limit: MAX_BODIES });
        }
        if matches!(component, Component::Trigger(_))
            && !entity.components.contains_key(&ComponentKind::Trigger)
            && self.trigger_count() >= MAX_TRIGGERS
        {
            return Err(GameError::TooManyTriggers {
                limit: MAX_TRIGGERS,
            });
        }
        let entity = self
            .entities
            .get_mut(&id)
            .ok_or(GameError::NoSuchEntity(id))?;
        entity.set_component(component)
    }

    /// Removes a component. Refused while another component needs it, or while a camera
    /// follows the entity and the component is its transform.
    pub fn remove_component(&mut self, id: EntityId, kind: ComponentKind) -> Result<(), GameError> {
        if kind == ComponentKind::Transform && self.followers(id).next().is_some() {
            return Err(GameError::Followed(id));
        }
        self.entities
            .get_mut(&id)
            .ok_or(GameError::NoSuchEntity(id))?
            .remove_component(kind)
    }

    /// Every problem in the game, not just the first. Empty when it is valid.
    pub fn problems(&self) -> Vec<GameError> {
        let mut out = Vec::new();
        if self.entities.len() > MAX_ENTITIES {
            out.push(GameError::TooManyEntities {
                limit: MAX_ENTITIES,
            });
        }
        if self.body_count() > MAX_BODIES {
            out.push(GameError::TooManyBodies { limit: MAX_BODIES });
        }
        if self.trigger_count() > MAX_TRIGGERS {
            out.push(GameError::TooManyTriggers {
                limit: MAX_TRIGGERS,
            });
        }
        for entity in self.entities.values() {
            if let Err(error) = entity.check() {
                out.push(error);
            }
            for component in entity.components.values() {
                match component {
                    Component::Camera(c) => {
                        if let Some(target) = c.follow
                            && !self.has(target, ComponentKind::Transform)
                        {
                            out.push(GameError::NoSuchEntity(target));
                        }
                    }
                    Component::Hook(hook) => {
                        if let Some(target) = hook.spawn
                            && target != entity.id
                            && !self.has(target, ComponentKind::Transform)
                        {
                            out.push(GameError::NoSuchEntity(target));
                        }
                    }
                    _ => {}
                }
            }
        }
        out
    }

    /// The first problem in the game, or `Ok` when there is none.
    pub fn validate(&self) -> Result<(), GameError> {
        match self.problems().into_iter().next() {
            None => Ok(()),
            Some(error) => Err(error),
        }
    }

    fn has(&self, id: EntityId, kind: ComponentKind) -> bool {
        self.entities
            .get(&id)
            .is_some_and(|entity| entity.components.contains_key(&kind))
    }

    fn body_count(&self) -> usize {
        self.entities
            .values()
            .filter(|entity| entity.components.contains_key(&ComponentKind::Body))
            .count()
    }

    fn trigger_count(&self) -> usize {
        self.entities
            .values()
            .filter(|entity| entity.components.contains_key(&ComponentKind::Trigger))
            .count()
    }

    /// Every entity whose hooks bring `target` to life.
    fn spawners(&self, target: EntityId) -> impl Iterator<Item = EntityId> + '_ {
        self.entities.values().filter_map(move |entity| {
            entity
                .components
                .values()
                .find_map(|component| match component {
                    Component::Hook(hook) if hook.spawn == Some(target) => Some(entity.id),
                    _ => None,
                })
        })
    }

    fn followers(&self, target: EntityId) -> impl Iterator<Item = EntityId> + '_ {
        self.entities.values().filter_map(move |entity| {
            match entity.components.get(&ComponentKind::Camera) {
                Some(Component::Camera(c)) if c.follow == Some(target) => Some(entity.id),
                _ => None,
            }
        })
    }
}

fn check_name(name: &str) -> Result<(), GameError> {
    let len = name.chars().count();
    if len == 0 || len > MAX_NAME_CHARS || name.chars().any(char::is_control) {
        return Err(GameError::BadName);
    }
    Ok(())
}
