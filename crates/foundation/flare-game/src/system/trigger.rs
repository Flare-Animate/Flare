//! The trigger system: zones a body walks into, reported the way collisions are, as pairs that
//! begin and stop overlapping. A zone has no body and never meets another zone; the hook system
//! reads the entries that began this step. A `once` zone reports its first entry and then is
//! silent for the rest of the run, its exits included.

use std::collections::BTreeSet;

use crate::system::StepContext;
use crate::{Component, ComponentKind, EntityId, Event, GameError, World};

pub(crate) fn run(world: &mut World, ctx: &mut StepContext) -> Result<(), GameError> {
    struct Visitor {
        id: EntityId,
        min: (f64, f64),
        max: (f64, f64),
    }
    let visitors: Vec<Visitor> = world
        .entities
        .iter()
        .filter_map(|(id, live)| {
            let Component::Body(body) = live.components.get(&ComponentKind::Body)? else {
                return None;
            };
            let Component::Transform(t) = live.components.get(&ComponentKind::Transform)? else {
                return None;
            };
            Some(Visitor {
                id: *id,
                min: (t.x, t.y),
                max: (t.x + body.width, t.y + body.height),
            })
        })
        .collect();

    let mut now: BTreeSet<(EntityId, EntityId)> = BTreeSet::new();
    let mut begins: Vec<(EntityId, EntityId)> = Vec::new();
    for (zone_id, live) in &world.entities {
        let Some(Component::Trigger(trigger)) = live.components.get(&ComponentKind::Trigger) else {
            continue;
        };
        let Some(Component::Transform(t)) = live.components.get(&ComponentKind::Transform) else {
            continue;
        };
        if trigger.once && live.once_fired {
            continue;
        }
        let zone = (t.x, t.y, t.x + trigger.width, t.y + trigger.height);
        for v in &visitors {
            if zone.0 < v.max.0 && v.min.0 < zone.2 && zone.1 < v.max.1 && v.min.1 < zone.3 {
                let pair = (*zone_id, v.id);
                if !world.zone.contains(&pair) {
                    begins.push(pair);
                }
                now.insert(pair);
            }
        }
    }

    for (zone, visitor) in &begins {
        ctx.emit(Event::Trigger {
            trigger: *zone,
            by: *visitor,
            begin: true,
        })?;
        ctx.triggers.push((*zone, *visitor));
    }
    for pair in world.zone.difference(&now) {
        ctx.emit(Event::Trigger {
            trigger: pair.0,
            by: pair.1,
            begin: false,
        })?;
    }
    world.zone = now
        .iter()
        .copied()
        .filter(|(zone, _)| !once(world, *zone))
        .collect();
    for (zone, _) in &begins {
        let is_once = once(world, *zone);
        if let Some(live) = world.entities.get_mut(zone) {
            live.once_fired = is_once || live.once_fired;
        }
    }
    Ok(())
}

/// Whether the trigger on `entity` reports only once.
fn once(world: &World, entity: EntityId) -> bool {
    world
        .entities
        .get(&entity)
        .and_then(|live| live.components.get(&ComponentKind::Trigger))
        .is_some_and(|component| match component {
            Component::Trigger(trigger) => trigger.once,
            _ => false,
        })
}
