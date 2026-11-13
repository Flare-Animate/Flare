//! The hook system: a declarative reaction, the Flash engines' keydown and trigger handlers
//! were. When `when` first happens, the hook plays its sound, brings its spawn to the hook's
//! place and in sight, hands the camera to the visitor, and removes who `destroy` names. No
//! scripting: what a hook does is exactly these four things.
//!
//! It runs straight after the trigger system, whose entries this step it reads, and before the
//! camera, which follows the positions it has just written. Removals happen at the end, so a
//! hook's own reaction is not cut short by another hook's destroy.

use crate::system::StepContext;
use crate::{
    Component, ComponentKind, DestroyWho, EntityId, Event, GameError, Hook, HookWhen, World,
};

pub(crate) fn run(world: &mut World, ctx: &mut StepContext) -> Result<(), GameError> {
    let hooks: Vec<EntityId> = world
        .entities
        .iter()
        .filter(|(_, live)| live.components.contains_key(&ComponentKind::Hook))
        .map(|(id, _)| *id)
        .collect();

    let mut removals: Vec<EntityId> = Vec::new();
    for id in hooks {
        let Some(Component::Hook(hook)) = world
            .entities
            .get(&id)
            .and_then(|live| live.components.get(&ComponentKind::Hook))
        else {
            continue;
        };
        // Cloned so the reaction can use the world while it reads the hook.
        let hook = hook.clone();
        let fired: Option<Option<EntityId>> = match &hook.when {
            HookWhen::Trigger => ctx
                .triggers
                .iter()
                .find(|(trigger, _)| *trigger == id)
                .map(|(_, visitor)| Some(*visitor)),
            HookWhen::Action(name) => {
                let held = ctx.input.action(name);
                let rising = held && !world.entities.get(&id).is_some_and(|live| live.hook_held);
                if let Some(live) = world.entities.get_mut(&id) {
                    live.hook_held = held;
                }
                rising.then_some(None)
            }
        };
        let Some(visitor) = fired else {
            continue;
        };
        fire(world, ctx, id, &hook, visitor)?;
        match hook.destroy {
            DestroyWho::Never => {}
            DestroyWho::Visitor => {
                if let Some(visitor) = visitor {
                    removals.push(visitor);
                }
            }
            DestroyWho::This => removals.push(id),
        }
    }
    for id in removals {
        world.entities.remove(&id);
    }
    Ok(())
}

/// One hook's reaction: the sound, the spawn, the camera and nothing else.
fn fire(
    world: &mut World,
    ctx: &mut StepContext,
    id: EntityId,
    hook: &Hook,
    visitor: Option<EntityId>,
) -> Result<(), GameError> {
    if let Some(sound) = hook.sound {
        ctx.emit(Event::Sound {
            entity: id,
            sound,
            volume: 1.0,
            looping: false,
        })?;
    }
    ctx.emit(Event::Hook { entity: id })?;
    if let Some(spawn) = hook.spawn {
        let place = world
            .entities
            .get(&id)
            .and_then(|live| live.components.get(&ComponentKind::Transform))
            .and_then(|component| match component {
                Component::Transform(t) => Some((t.x, t.y)),
                _ => None,
            });
        if let (Some((x, y)), Some(live)) = (place, world.entities.get_mut(&spawn)) {
            if let Some(Component::Transform(t)) =
                live.components.get_mut(&ComponentKind::Transform)
            {
                t.x = x;
                t.y = y;
            }
            if let Some(Component::Sprite(sprite)) = live.components.get_mut(&ComponentKind::Sprite)
            {
                sprite.visible = true;
            }
        }
    }
    if hook.follow {
        let Some(visitor) = visitor else {
            return Ok(());
        };
        for live in world.entities.values_mut() {
            if let Some(Component::Camera(camera)) = live.components.get_mut(&ComponentKind::Camera)
            {
                camera.follow = Some(visitor);
            }
        }
    }
    Ok(())
}
