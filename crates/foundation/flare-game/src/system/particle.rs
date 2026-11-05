//! The particle system. The C++ emitter was a stub; the port counts particles at the emitter's
//! rate and reports them as events. The particles themselves are not simulated.

use crate::system::StepContext;
use crate::{Component, ComponentKind, Event, GameError, World};

pub(crate) fn run(world: &mut World, ctx: &mut StepContext) -> Result<(), GameError> {
    let dt = ctx.dt;
    let mut emitted = Vec::new();
    for (id, live) in world.entities.iter_mut() {
        let particle = match live.components.get(&ComponentKind::Particle) {
            Some(Component::Particle(particle)) => *particle,
            _ => continue,
        };
        if !particle.auto_play {
            continue;
        }
        live.emit_carry += particle.rate * dt;
        let count = live.emit_carry.floor();
        live.emit_carry -= count;
        if count >= 1.0 {
            emitted.push(Event::Emit {
                entity: *id,
                count: count as u32,
            });
        }
    }
    for event in emitted {
        ctx.emit(event)?;
    }
    Ok(())
}
